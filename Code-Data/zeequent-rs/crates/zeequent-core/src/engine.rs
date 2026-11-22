//! The proof engine: one prover and one verifier for any constraint system expressed as a [`System`].

use crate::check::Violation;
use crate::public_polys::{eval_at, on_domain, PublicPoly};
use ark_bn254::{Fr, G1Affine};
use ark_ec::CurveGroup;
use ark_ff::{FftField, Field, One, UniformRand, Zero};
use ark_poly::{DenseUVPolynomial, EvaluationDomain, Radix2EvaluationDomain};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ark_std::rand::RngCore;
use ark_std::{cfg_chunks, cfg_into_iter, cfg_iter};
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use zeequent_kzg::{
    batch_open, batch_verify, commit, commit_blinding, commit_mask, evaluate_all, Blinding,
    Commitment, DomainKey, KzgError, Opening, OpeningClaim, Poly, Srs, Transcript, VerifierKey,
};

/// Slack in the quotient's degree bound.
pub(crate) const QUOTIENT_EXTRA: usize = 8;

/// A constraint system over columns on a domain of size `n`.
pub(crate) trait System {
    fn protocol(&self) -> &'static [u8];
    /// Absorbs the public statement (parameters, receipt).
    fn absorb_statement(&self, t: &mut Transcript);
    fn n(&self) -> usize;
    /// Number of first-round columns; they come first in the layout.
    fn phase1_columns(&self) -> usize;
    /// Number of columns in both rounds.
    fn num_columns(&self) -> usize;
    /// Columns also opened at `omega * zeta`, in opening order.
    fn shifted(&self) -> Vec<usize>;
    /// The second-round columns, from the first-round columns and `beta`.
    fn phase2(&self, beta: Fr, phase1: &[Vec<Fr>]) -> Vec<Vec<Fr>>;
    /// The public polynomials.
    fn public_polys(&self) -> Vec<PublicPoly>;
    /// `sum_j alpha^j V_j` at one point.
    fn combine(
        &self,
        cols: &[Fr],
        shifted: &[Fr],
        pubs: &[Fr],
        gate: Fr,
        beta: Fr,
        alpha: Fr,
    ) -> Fr;
}

/// A column whose commitment already exists (a book column).
pub(crate) struct FixedColumn<'a> {
    pub column: usize,
    pub commitment: Commitment,
    pub blinding: &'a Blinding,
    /// Coefficients of `rho` in the mask `rho(X) * Z_H(X)`.
    pub mask: &'a [Fr],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proof {
    /// One commitment per column, in layout order (first round, then second).
    pub commitments: Vec<Commitment>,
    /// Commitment to the quotient.
    pub quotient: Commitment,
    /// Every column, then the quotient, at `zeta`.
    pub at_zeta: Opening,
    /// The shifted columns at `omega * zeta`.
    pub at_zeta_omega: Opening,
}

/// Largest column count `Proof::from_bytes` accepts.
const MAX_COLUMNS: usize = 1 << 16;

fn put<T: CanonicalSerialize>(out: &mut Vec<u8>, value: &T) {
    value
        .serialize_compressed(out)
        .expect("writing to a Vec cannot fail");
}

fn take<T: CanonicalDeserialize>(input: &mut &[u8]) -> Option<T> {
    T::deserialize_compressed(input).ok()
}

fn take_len(input: &mut &[u8]) -> Option<usize> {
    let (head, rest) = input.split_first_chunk::<4>()?;
    *input = rest;
    let len = u32::from_le_bytes(*head) as usize;
    (len <= MAX_COLUMNS + 1).then_some(len)
}

fn put_opening(out: &mut Vec<u8>, opening: &Opening) {
    put(out, &opening.point);
    put(out, &opening.blinding_eval);
    put(out, &opening.witness);
    out.extend_from_slice(&(opening.evals.len() as u32).to_le_bytes());
    for eval in &opening.evals {
        put(out, eval);
    }
}

fn take_opening(input: &mut &[u8]) -> Option<Opening> {
    let point = take(input)?;
    let blinding_eval = take(input)?;
    let witness = take(input)?;
    let len = take_len(input)?;
    let evals = (0..len).map(|_| take(input)).collect::<Option<_>>()?;
    Some(Opening {
        point,
        evals,
        blinding_eval,
        witness,
    })
}

impl Proof {
    /// Serializes the proof.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.commitments.len() as u32).to_le_bytes());
        for commitment in &self.commitments {
            put(&mut out, &commitment.0);
        }
        put(&mut out, &self.quotient.0);
        put_opening(&mut out, &self.at_zeta);
        put_opening(&mut out, &self.at_zeta_omega);
        out
    }

    /// Parses a proof.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let mut input = bytes;
        let len = take_len(&mut input)?;
        // Commitments are decoded in parallel when the `parallel` feature is on.
        let size = len.checked_mul(32).filter(|size| *size <= input.len())?;
        let (block, rest) = input.split_at(size);
        input = rest;
        let commitments = cfg_chunks!(block, 32)
            .map(|mut chunk| G1Affine::deserialize_compressed(&mut chunk).ok().map(Commitment))
            .collect::<Option<Vec<_>>>()?;
        let quotient = Commitment(take(&mut input)?);
        let at_zeta = take_opening(&mut input)?;
        let at_zeta_omega = take_opening(&mut input)?;
        input.is_empty().then_some(Self {
            commitments,
            quotient,
            at_zeta,
            at_zeta_omega,
        })
    }

    /// Serialized size in bytes.
    pub fn size_bytes(&self) -> usize {
        self.to_bytes().len()
    }

    /// Number of commitments, including the quotient.
    pub fn num_commitments(&self) -> usize {
        self.commitments.len() + 1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProveError {
    /// The witness does not satisfy the constraints (pre-flight check).
    Violation(Violation),
    /// The combined constraint polynomial is not divisible by `Z_H`.
    NotDivisible,
    Kzg(KzgError),
    /// The field has no subgroup of the required size, or the domain key is for another size.
    Domain,
}

impl From<KzgError> for ProveError {
    fn from(e: KzgError) -> Self {
        ProveError::Kzg(e)
    }
}

/// Why a proof was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerifyError {
    /// The receipt fails a verifier-side scalar check.
    Receipt,
    /// The proof has the wrong number of commitments or evaluations.
    Shape,
    /// The proof's book commitments are not the published ones.
    Book,
    /// An opening is claimed at a point other than the challenge.
    Point,
    /// The constraint identity fails at `zeta`.
    Identity,
    /// A KZG opening is invalid.
    Opening,
    Domain,
}

pub(crate) fn domain_of(n: usize) -> Option<Radix2EvaluationDomain<Fr>> {
    Radix2EvaluationDomain::<Fr>::new(n).filter(|domain| domain.size() == n)
}

/// Absorbs the statement and both rounds of commitments, and returns the transcript with the challenges `(beta, alpha)`.
fn start_transcript<S: System>(
    system: &S,
    first_round: &[Commitment],
    second_round: impl FnOnce(Fr) -> Result<Vec<Commitment>, ProveError>,
) -> Result<(Transcript, Fr, Fr, Vec<Commitment>), ProveError> {
    let mut t = Transcript::new(system.protocol());
    system.absorb_statement(&mut t);
    for commitment in first_round {
        t.absorb_g1(b"column", &commitment.0);
    }
    let beta = t.challenge(b"beta");
    let helpers = second_round(beta)?;
    for commitment in &helpers {
        t.absorb_g1(b"helper", &commitment.0);
    }
    let alpha = t.challenge(b"alpha");
    Ok((t, beta, alpha, helpers))
}

/// Reduces a polynomial modulo `X^n - s`.
fn fold(coeffs: &[Fr], n: usize, s: Fr) -> Vec<Fr> {
    let mut reduced = vec![Fr::zero(); n];
    let mut power = Fr::one();
    for block in coeffs.chunks(n) {
        for (slot, coefficient) in reduced.iter_mut().zip(block) {
            *slot += *coefficient * power;
        }
        power *= s;
    }
    reduced
}

/// Adds `rho(X) * (X^n - 1)`: the values on the domain are unchanged.
fn apply_mask(coeffs: &mut Vec<Fr>, n: usize, rho: &[Fr]) {
    if coeffs.len() < n + rho.len() {
        coeffs.resize(n + rho.len(), Fr::zero());
    }
    for (i, r) in rho.iter().enumerate() {
        coeffs[i] -= r;
        coeffs[n + i] += r;
    }
}

/// Runs the prover for `system` on its first-round `columns` (evaluation form, layout order).
pub(crate) fn prove_system<S: System + Sync, R: RngCore>(
    system: &S,
    columns: &[Vec<Fr>],
    fixed: &[FixedColumn],
    srs: &Srs,
    key: &DomainKey,
    rng: &mut R,
    skip_checks: bool,
) -> Result<Proof, ProveError> {
    let n = system.n();
    let total = system.num_columns();
    let phase1 = system.phase1_columns();
    let shifted_columns = system.shifted();
    let shifted_columns = &shifted_columns[..];
    let domain = domain_of(n).ok_or(ProveError::Domain)?;
    if key.n != n || columns.len() != phase1 || phase1 > total {
        return Err(ProveError::Domain);
    }

    // Columns -> masked polynomials and commitments.
    let fresh: Vec<Option<(Vec<Fr>, Blinding)>> = (0..total)
        .map(|index| {
            if fixed.iter().any(|f| f.column == index) {
                return None;
            }
            let points = if shifted_columns.contains(&index) { 2 } else { 1 };
            let rho = (0..=points).map(|_| Fr::rand(rng)).collect();
            Some((rho, Blinding::random(points, rng)))
        })
        .collect();

    type Built = (Poly, Commitment, Blinding);
    let build = |index: usize, evals: &[Fr]| -> Result<Built, KzgError> {
        let mut coeffs = domain.ifft(evals);
        let (commitment, blinding) = match &fresh[index] {
            None => {
                let fixed_column = fixed
                    .iter()
                    .find(|f| f.column == index)
                    .expect("a column without fresh randomness is fixed");
                apply_mask(&mut coeffs, n, fixed_column.mask);
                (fixed_column.commitment, fixed_column.blinding.clone())
            }
            Some((rho, blinding)) => {
                apply_mask(&mut coeffs, n, rho);
                let point = key.commit_evals(evals)
                    + commit_mask(srs, n, rho)?
                    + commit_blinding(srs, blinding)?;
                (Commitment(point.into_affine()), blinding.clone())
            }
        };
        Ok((Poly::from_coefficients_vec(coeffs), commitment, blinding))
    };

    let mut polys = Vec::with_capacity(total);
    let mut commitments = Vec::with_capacity(total);
    let mut blindings = Vec::with_capacity(total);

    // First round.
    let built: Vec<Built> = cfg_iter!(columns)
        .enumerate()
        .map(|(index, column)| build(index, column))
        .collect::<Result<_, _>>()?;
    for (poly, commitment, blinding) in built {
        polys.push(poly);
        commitments.push(commitment);
        blindings.push(blinding);
    }

    // Second round: the lookup helpers, which depend on `beta`.
    let (mut t, beta, alpha, helper_commitments) =
        start_transcript(system, &commitments, |beta| {
            let helpers = system.phase2(beta, columns);
            if helpers.len() != total - phase1 {
                return Err(ProveError::Domain);
            }
            let built: Vec<Built> = cfg_iter!(helpers)
                .enumerate()
                .map(|(index, evals)| build(phase1 + index, evals))
                .collect::<Result<_, _>>()?;
            let mut helper_commitments = Vec::with_capacity(built.len());
            for (poly, commitment, blinding) in built {
                polys.push(poly);
                helper_commitments.push(commitment);
                blindings.push(blinding);
            }
            Ok(helper_commitments)
        })?;
    commitments.extend(helper_commitments);

    // Quotient.
    let big = Radix2EvaluationDomain::<Fr>::new(4 * n).ok_or(ProveError::Domain)?;
    let big_coset = big.get_coset(Fr::GENERATOR).ok_or(ProveError::Domain)?;
    debug_assert_eq!(big.group_gen.pow([4]), domain.group_gen);

    let public_polys: Vec<Vec<Fr>> = on_domain(&system.public_polys(), n)
        .iter()
        .map(|evals| domain.ifft(evals))
        .collect();
    let last_root = domain.element(n - 1);

    let mut quotient_evals = vec![Fr::zero(); 4 * n];
    for j in 0..4 {
        let offset = big_coset.element(j);
        let coset = domain.get_coset(offset).ok_or(ProveError::Domain)?;
        let offset_n = offset.pow([n as u64]);
        let vanishing_inverse = (offset_n - Fr::one())
            .inverse()
            .expect("the coset is disjoint from the domain");

        let column_evals: Vec<Vec<Fr>> = cfg_iter!(polys)
            .map(|p| coset.fft(&fold(&p.coeffs, n, offset_n)))
            .collect();
        let public_evals: Vec<Vec<Fr>> = public_polys.iter().map(|p| coset.fft(p)).collect();
        let points: Vec<Fr> = coset.elements().collect();

        let chunk: Vec<Fr> = cfg_into_iter!(0..n)
            .map(|t| {
                let next = (t + 1) % n;
                let cols: Vec<Fr> = column_evals.iter().map(|evals| evals[t]).collect();
                let shifted: Vec<Fr> =
                    shifted_columns.iter().map(|c| column_evals[*c][next]).collect();
                let pubs: Vec<Fr> = public_evals.iter().map(|evals| evals[t]).collect();
                system.combine(&cols, &shifted, &pubs, points[t] - last_root, beta, alpha)
                    * vanishing_inverse
            })
            .collect();
        for (t, value) in chunk.into_iter().enumerate() {
            quotient_evals[4 * t + j] = value;
        }
    }
    let mut quotient_coeffs = big_coset.ifft(&quotient_evals);
    let bound = (2 * n + QUOTIENT_EXTRA + 1).min(4 * n);
    if !skip_checks && quotient_coeffs[bound..].iter().any(|c| !c.is_zero()) {
        return Err(ProveError::NotDivisible);
    }
    quotient_coeffs.truncate(bound);
    let quotient_poly = Poly::from_coefficients_vec(quotient_coeffs);
    let (quotient, quotient_blinding) = commit(srs, &quotient_poly, 1, rng)?;

    t.absorb_g1(b"quotient", &quotient.0);
    let zeta = t.challenge(b"zeta");
    let zeta_omega = zeta * domain.group_gen;

    // Openings.
    let mut at_zeta_polys: Vec<&Poly> = polys.iter().collect();
    at_zeta_polys.push(&quotient_poly);
    let mut at_zeta_blindings: Vec<&Blinding> = blindings.iter().collect();
    at_zeta_blindings.push(&quotient_blinding);
    t.absorb_frs(b"evals_zeta", &evaluate_all(&at_zeta_polys, zeta));
    let gamma_1 = t.challenge(b"gamma_1");

    let shifted_polys: Vec<&Poly> = shifted_columns.iter().map(|c| &polys[*c]).collect();
    let shifted_blindings: Vec<&Blinding> =
        shifted_columns.iter().map(|c| &blindings[*c]).collect();
    t.absorb_frs(b"evals_zeta_omega", &evaluate_all(&shifted_polys, zeta_omega));
    let gamma_2 = t.challenge(b"gamma_2");

    let at_zeta = batch_open(srs, &at_zeta_polys, &at_zeta_blindings, zeta, gamma_1)?;
    let at_zeta_omega =
        batch_open(srs, &shifted_polys, &shifted_blindings, zeta_omega, gamma_2)?;

    Ok(Proof {
        commitments,
        quotient,
        at_zeta,
        at_zeta_omega,
    })
}

/// Runs the verifier for `system`.
pub(crate) fn verify_system<S: System>(
    system: &S,
    proof: &Proof,
    vk: &VerifierKey,
) -> Result<(), VerifyError> {
    let n = system.n();
    let total = system.num_columns();
    let phase1 = system.phase1_columns();
    let shifted_columns = system.shifted();
    let domain = domain_of(n).ok_or(VerifyError::Domain)?;

    if phase1 > total
        || proof.commitments.len() != total
        || proof.at_zeta.evals.len() != total + 1
        || proof.at_zeta_omega.evals.len() != shifted_columns.len()
    {
        return Err(VerifyError::Shape);
    }

    // Recompute every challenge.
    let (first_round, second_round) = proof.commitments.split_at(phase1);
    let (mut t, beta, alpha, _) =
        start_transcript(system, first_round, |_| Ok(second_round.to_vec()))
            .map_err(|_| VerifyError::Shape)?;
    t.absorb_g1(b"quotient", &proof.quotient.0);
    let zeta = t.challenge(b"zeta");
    let zeta_omega = zeta * domain.group_gen;
    t.absorb_frs(b"evals_zeta", &proof.at_zeta.evals);
    let gamma_1 = t.challenge(b"gamma_1");
    t.absorb_frs(b"evals_zeta_omega", &proof.at_zeta_omega.evals);
    let gamma_2 = t.challenge(b"gamma_2");
    for opening in [&proof.at_zeta, &proof.at_zeta_omega] {
        t.absorb_g1(b"witness", &opening.witness);
        t.absorb_fr(b"blinding_eval", &opening.blinding_eval);
    }
    let r = t.challenge(b"r");

    if proof.at_zeta.point != zeta || proof.at_zeta_omega.point != zeta_omega {
        return Err(VerifyError::Point);
    }
    let vanishing = zeta.pow([n as u64]) - Fr::one();
    if vanishing.is_zero() {
        return Err(VerifyError::Point);
    }

    // The constraint identity at zeta.
    let pubs = eval_at(&domain, &system.public_polys(), zeta);
    let combined = system.combine(
        &proof.at_zeta.evals[..total],
        &proof.at_zeta_omega.evals,
        &pubs,
        zeta - domain.element(n - 1),
        beta,
        alpha,
    );
    if combined != proof.at_zeta.evals[total] * vanishing {
        return Err(VerifyError::Identity);
    }

    // The openings.
    let mut at_zeta_commitments = proof.commitments.clone();
    at_zeta_commitments.push(proof.quotient);
    let shifted_commitments: Vec<Commitment> =
        shifted_columns.iter().map(|c| proof.commitments[*c]).collect();
    let claims = [
        OpeningClaim {
            commitments: &at_zeta_commitments,
            opening: &proof.at_zeta,
            gamma: gamma_1,
        },
        OpeningClaim {
            commitments: &shifted_commitments,
            opening: &proof.at_zeta_omega,
            gamma: gamma_2,
        },
    ];
    if !batch_verify(vk, &claims, r) {
        return Err(VerifyError::Opening);
    }
    Ok(())
}
