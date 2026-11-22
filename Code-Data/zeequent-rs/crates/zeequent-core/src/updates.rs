//! The update-validity proof: each logged difference encodes a bounded non-negative quantity at one position of the padded domain. Zero is allowed.

use crate::book::{open_column, verify_empty_column, EmptyColumnProof, UpdateStep};
use crate::check::Violation;
use crate::columns::to_field;
use crate::engine::{
    domain_of, prove_system, verify_system, FixedColumn, Proof, ProveError, System, VerifyError,
};
use crate::lookup::{range_entries, Lookup};
use crate::proof::ProverOptions;
use crate::public_polys::{indicator, PublicPoly};
use ark_bn254::{Fr, G1Affine, G1Projective};
use ark_ec::{AffineRepr, CurveGroup, VariableBaseMSM};
use ark_ff::{batch_inversion, One, UniformRand, Zero};
use ark_poly::EvaluationDomain;
use ark_std::rand::RngCore;
use zeequent_kzg::codec::{Reader, Writer};
use zeequent_kzg::{
    commit_blinding, commit_mask, Blinding, Commitment, DomainKey, KzgError, Srs, Transcript,
    VerifierKey,
};
use zeequent_types::Params;

const U_Q: usize = 0;
const U_T: usize = 1;
const U_W: usize = 2;
const U_LIMBS: usize = 3;

// Public polynomials.
const P_POWERS: usize = 0;
const P_TICKS: usize = 1;
const P_IN_H: usize = 2;
const P_TABLE: usize = 3;

/// The SRS degree the update-validity prover needs for a step domain of this size.
pub fn required_srs_degree_updates(size: usize) -> usize {
    2 * size + crate::engine::QUOTIENT_EXTRA
}

/// The smallest admissible step domain for `steps` updates.
pub fn update_domain_size(n: usize, steps: usize) -> usize {
    if steps <= n {
        n
    } else {
        steps.next_power_of_two().max(2 * n)
    }
}

fn admissible(n: usize, size: usize) -> bool {
    size.is_power_of_two() && (size == n || size >= 2 * n)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateProof {
    /// `Delta_mask`: a masked commitment to the empty column.
    pub mask: Commitment,
    pub mask_proof: EmptyColumnProof,
    pub proof: Proof,
}

const UPDATE_PROOF_MAGIC: &[u8; 8] = b"ZQUPDPRF";

impl UpdateProof {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::new(UPDATE_PROOF_MAGIC);
        w.g1(&self.mask.0);
        w.g1(&self.mask_proof.d);
        w.g1(&self.mask_proof.e);
        w.g1(&self.mask_proof.schnorr_commitment);
        w.frs(&self.mask_proof.schnorr_response);
        w.bytes(&self.proof.to_bytes());
        w.finish()
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let mut r = Reader::new(bytes, UPDATE_PROOF_MAGIC)?;
        let mask = Commitment(r.g1()?);
        let mask_proof = EmptyColumnProof {
            d: r.g1()?,
            e: r.g1()?,
            schnorr_commitment: r.g1()?,
            schnorr_response: r.frs()?,
        };
        let proof = Proof::from_bytes(r.bytes()?)?;
        r.finish()?;
        Some(Self { mask, mask_proof, proof })
    }

    pub fn size_bytes(&self) -> usize {
        self.to_bytes().len()
    }
}

/// The statement digest and the challenge `c`.
fn challenges(
    params: &Params,
    size: usize,
    deltas: &[G1Affine],
    mask: &Commitment,
    q: &Commitment,
    t: &Commitment,
) -> (Fr, Fr) {
    let mut transcript = Transcript::new(b"zeequent-updates-v1");
    transcript.absorb_u64(b"n", params.n as u64);
    transcript.absorb_u64(b"n_ticks", params.n_ticks as u64);
    transcript.absorb_u64(b"k_in", params.k_in as u64);
    transcript.absorb_u64(b"size", size as u64);
    transcript.absorb_u64(b"steps", deltas.len() as u64);
    for delta in deltas {
        transcript.absorb_g1(b"step", delta);
    }
    transcript.absorb_g1(b"mask", &mask.0);
    transcript.absorb_g1(b"q", &q.0);
    transcript.absorb_g1(b"t", &t.0);
    let statement = transcript.challenge(b"statement");
    let c = transcript.challenge(b"c");
    (statement, c)
}

fn powers(c: Fr, count: usize) -> Vec<Fr> {
    let mut out = Vec::with_capacity(count);
    let mut power = Fr::one();
    for _ in 0..count {
        out.push(power);
        power *= c;
    }
    out
}

struct UpdateSystem {
    /// Size of the book's domain `H`.
    book_n: usize,
    /// Size of the step domain `K`.
    size: usize,
    /// Number of limbs of a quantity.
    limbs: usize,
    lookup: Lookup,
    statement: Fr,
    /// `c^j` for each logged step.
    powers: Vec<Fr>,
}

impl UpdateSystem {
    fn new(params: &Params, size: usize, steps: usize, statement: Fr, c: Fr) -> Self {
        let limb_bits = Lookup::limb_bits_for(size);
        let bits = params.k_in as usize;
        let limbs = bits.div_ceil(limb_bits);
        Self {
            book_n: params.n,
            size,
            limbs,
            lookup: Lookup {
                limb_bits,
                entries: range_entries(limb_bits, U_LIMBS, bits),
                mult: U_LIMBS + limbs,
            },
            statement,
            powers: powers(c, steps),
        }
    }

    /// Rows of `K` that are points of `H`: row `i * stride` is `omega^i`.
    fn stride(&self) -> usize {
        self.size / self.book_n
    }

    fn histogram_helper(&self) -> usize {
        self.lookup.total()
    }

    fn histogram_table_helper(&self) -> usize {
        self.lookup.total() + 1
    }

    fn histogram_sum(&self) -> usize {
        self.lookup.total() + 2
    }

    /// The elements of `H`, in order.
    fn ticks(&self) -> Vec<Fr> {
        domain_of(self.book_n).map(|d| d.elements().collect()).unwrap_or_default()
    }
}

impl System for UpdateSystem {
    fn protocol(&self) -> &'static [u8] {
        b"zeequent-update-validity-v1"
    }

    /// The statement digest binds everything public.
    fn absorb_statement(&self, t: &mut Transcript) {
        t.absorb_fr(b"statement", &self.statement);
        t.absorb_u64(b"size", self.size as u64);
        t.absorb_u64(b"steps", self.powers.len() as u64);
    }

    fn n(&self) -> usize {
        self.size
    }

    fn phase1_columns(&self) -> usize {
        self.lookup.phase1()
    }

    fn num_columns(&self) -> usize {
        self.lookup.total() + 3
    }

    fn shifted(&self) -> Vec<usize> {
        vec![self.lookup.running_sum(), self.histogram_sum()]
    }

    fn phase2(&self, beta: Fr, phase1: &[Vec<Fr>]) -> Vec<Vec<Fr>> {
        let mut columns = self.lookup.phase2(beta, phase1);

        // g_j = c^j Q_j / (beta - T_j) on the rows that hold a step.
        let mut inverse: Vec<Fr> = phase1[U_T].iter().map(|t| beta - t).collect();
        batch_inversion(&mut inverse);
        let mut g = vec![Fr::zero(); self.size];
        for (j, power) in self.powers.iter().enumerate() {
            g[j] = *power * phase1[U_Q][j] * inverse[j];
        }

        // u_i = w_i / (beta - omega^i) on the rows that are points of H.
        let mut inverse: Vec<Fr> = self.ticks().iter().map(|tick| beta - tick).collect();
        batch_inversion(&mut inverse);
        let stride = self.stride();
        let mut u = vec![Fr::zero(); self.size];
        for (i, inverse) in inverse.iter().enumerate() {
            u[i * stride] = phase1[U_W][i * stride] * inverse;
        }

        let mut sum = Vec::with_capacity(self.size);
        let mut running = Fr::zero();
        for row in 0..self.size {
            sum.push(running);
            running += g[row] - u[row];
        }
        columns.extend([g, u, sum]);
        columns
    }

    fn public_polys(&self) -> Vec<PublicPoly> {
        let stride = self.stride();
        vec![
            self.powers.iter().copied().enumerate().collect(),
            self.ticks().into_iter().enumerate().map(|(i, tick)| (i * stride, tick)).collect(),
            indicator((0..self.book_n).map(|i| i * stride)),
            self.lookup.table(),
        ]
    }

    fn combine(
        &self,
        cols: &[Fr],
        shifted: &[Fr],
        p: &[Fr],
        _gate: Fr,
        beta: Fr,
        alpha: Fr,
    ) -> Fr {
        let (g, u) = (cols[self.histogram_helper()], cols[self.histogram_table_helper()]);
        let mut terms = vec![
            // Q is the value of its limbs; with the lookup, Q is in range.
            cols[U_Q] - self.lookup.recompose(&cols[U_LIMBS..U_LIMBS + self.limbs]),
            // The two sides of the histogram identity.
            g * (beta - cols[U_T]) - p[P_POWERS] * cols[U_Q],
            u * (beta - p[P_TICKS]) - p[P_IN_H] * cols[U_W],
            // Their difference sums to zero around the domain.
            shifted[1] - cols[self.histogram_sum()] - (g - u),
        ];
        self.lookup.terms(cols, shifted[0], p[P_TABLE], beta, &mut terms);
        terms.iter().rev().fold(Fr::zero(), |acc, term| acc * alpha + term)
    }
}

/// A fresh masked, blinded commitment to a column opened at one point.
fn commit_column<R: RngCore>(
    evals: &[Fr],
    srs: &Srs,
    key: &DomainKey,
    rng: &mut R,
) -> Result<(Commitment, Blinding, Vec<Fr>), KzgError> {
    let mask: Vec<Fr> = (0..2).map(|_| Fr::rand(rng)).collect();
    let blinding = Blinding::random(1, rng);
    let point =
        key.commit_evals(evals) + commit_mask(srs, key.n, &mask)? + commit_blinding(srs, &blinding)?;
    Ok((Commitment(point.into_affine()), blinding, mask))
}

/// Proves that every step of the batch adds a non-negative quantity at one tick.
pub fn prove_updates<R: RngCore>(
    params: &Params,
    steps: &[UpdateStep],
    srs: &Srs,
    key: &DomainKey,
    update_key: &DomainKey,
    rng: &mut R,
) -> Result<UpdateProof, ProveError> {
    prove_updates_with(params, steps, srs, key, update_key, rng, ProverOptions::default())
}

pub fn prove_updates_with<R: RngCore>(
    params: &Params,
    steps: &[UpdateStep],
    srs: &Srs,
    key: &DomainKey,
    update_key: &DomainKey,
    rng: &mut R,
    options: ProverOptions,
) -> Result<UpdateProof, ProveError> {
    let n = params.n;
    let size = update_key.n;
    if key.n != n || !admissible(n, size) || steps.len() > size {
        return Err(ProveError::Domain);
    }
    let domain_h = domain_of(n).ok_or(ProveError::Domain)?;
    let domain_k = domain_of(size).ok_or(ProveError::Domain)?;
    if !options.skip_checks {
        for (j, step) in steps.iter().enumerate() {
            if step.qty >= params.input_bound() || step.tick >= params.n_ticks {
                return Err(ProveError::Violation(Violation {
                    constraint: "update",
                    tick: Some(j),
                }));
            }
        }
    }
    if steps.iter().any(|step| step.tick >= n) {
        return Err(ProveError::Domain);
    }

    // Q and T, committed before the challenge c.
    let ticks: Vec<Fr> = domain_h.elements().collect();
    let mut quantities = vec![0u64; size];
    let mut t = vec![Fr::one(); size];
    for (j, step) in steps.iter().enumerate() {
        quantities[j] = step.qty;
        t[j] = ticks[step.tick];
    }
    let q = to_field(&quantities);
    let (q_commitment, q_blinding, q_mask) = commit_column(&q, srs, update_key, rng)?;
    let (t_commitment, t_blinding, t_mask) = commit_column(&t, srs, update_key, rng)?;

    // The masked empty commitment that hides W's opening.
    let (mask_secret, mask_proof) = open_column(n, srs, rng)?;
    let mask = mask_secret.commitment;

    let deltas: Vec<G1Affine> = steps.iter().map(|step| step.delta).collect();
    let (statement, c) = challenges(params, size, &deltas, &mask, &q_commitment, &t_commitment);
    let system = UpdateSystem::new(params, size, steps.len(), statement, c);

    // W = Delta_mask + sum_j c^j Delta_j, with its polynomial and blinding.
    let mut w = vec![Fr::zero(); n];
    let mut blinding_shift = Fr::zero();
    for (step, power) in steps.iter().zip(&system.powers) {
        w[step.tick] += *power * Fr::from(step.qty);
        blinding_shift += *power * step.rho;
    }
    // Commitment from the public steps; polynomial and blinding from the history.
    let w_commitment = Commitment(
        (G1Projective::msm_unchecked(&deltas, &system.powers) + mask.0.into_group()).into_affine(),
    );
    let from_history =
        (key.commit_evals(&w) + srs.gamma_g[0] * blinding_shift + mask.0).into_affine();
    if !options.skip_checks && from_history != w_commitment.0 {
        return Err(ProveError::Violation(Violation {
            constraint: "update-log",
            tick: None,
        }));
    }
    let mut w_blinding = mask_secret.blinding.clone();
    w_blinding.0.coeffs[0] += blinding_shift;
    // On the step domain.
    let (w_evals, w_mask) = if size == n {
        (w, mask_secret.mask.clone())
    } else {
        let mut coeffs = domain_h.ifft(&w);
        coeffs.resize(n + mask_secret.mask.len(), Fr::zero());
        for (i, r) in mask_secret.mask.iter().enumerate() {
            coeffs[i] -= r;
            coeffs[n + i] += r;
        }
        (domain_k.fft(&coeffs), Vec::new())
    };

    // First-round columns: Q, T, W, limbs, multiplicities.
    let mut integers: Vec<Vec<u64>> = vec![Vec::new(); U_LIMBS];
    integers.extend(system.lookup.limbs(&quantities, system.limbs));
    let multiplicities = system.lookup.multiplicities(&integers, size);
    let mut columns = vec![q, t, w_evals];
    columns.extend(integers[U_LIMBS..].iter().map(|limb| to_field(limb)));
    columns.push(to_field(&multiplicities));

    let fixed = [
        FixedColumn {
            column: U_Q,
            commitment: q_commitment,
            blinding: &q_blinding,
            mask: &q_mask,
        },
        FixedColumn {
            column: U_T,
            commitment: t_commitment,
            blinding: &t_blinding,
            mask: &t_mask,
        },
        FixedColumn {
            column: U_W,
            commitment: w_commitment,
            blinding: &w_blinding,
            mask: &w_mask,
        },
    ];
    let proof =
        prove_system(&system, &columns, &fixed, srs, update_key, rng, options.skip_checks)?;
    Ok(UpdateProof { mask, mask_proof, proof })
}

/// Verifies an update-validity proof against the logged steps (`deltas`, from `Board::deltas`).
pub fn verify_updates(
    params: &Params,
    deltas: &[G1Affine],
    size: usize,
    proof: &UpdateProof,
    vk: &VerifierKey,
    key: &DomainKey,
) -> Result<(), VerifyError> {
    if key.n != params.n || !admissible(params.n, size) {
        return Err(VerifyError::Domain);
    }
    if deltas.len() > size || proof.proof.commitments.len() <= U_W {
        return Err(VerifyError::Shape);
    }
    if !verify_empty_column(&proof.mask, &proof.mask_proof, vk, key) {
        return Err(VerifyError::Book);
    }
    let commitments = &proof.proof.commitments;
    let (statement, c) =
        challenges(params, size, deltas, &proof.mask, &commitments[U_Q], &commitments[U_T]);
    let system = UpdateSystem::new(params, size, deltas.len(), statement, c);

    let combined = G1Projective::msm_unchecked(deltas, &system.powers) + proof.mask.0.into_group();
    if combined.into_affine() != commitments[U_W].0 {
        return Err(VerifyError::Book);
    }
    verify_system(&system, &proof.proof, vk)
}
