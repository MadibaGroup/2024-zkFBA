//! Hiding KZG10 polynomial commitments over BN254, with batched openings.

pub mod codec;
pub mod ptau;
pub mod transcript;

use ark_bn254::{Bn254, Fr, G1Affine, G1Projective, G2Affine, G2Projective};
use ark_ec::{pairing::Pairing, AffineRepr, CurveGroup, Group, VariableBaseMSM};
use ark_ff::{Field, One, UniformRand, Zero};
use ark_poly::{
    univariate::DensePolynomial, DenseUVPolynomial, EvaluationDomain, Polynomial,
    Radix2EvaluationDomain,
};
use ark_std::rand::RngCore;

pub use transcript::Transcript;

pub type Poly = DensePolynomial<Fr>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KzgError {
    /// A polynomial has more coefficients than the SRS has powers.
    SrsTooSmall { needed: usize, available: usize },
    /// `batch_open` was given different numbers of polynomials and blindings.
    LengthMismatch,
}

/// Structured reference string: powers of `tau` in G1 on two bases, and `[1]_2`, `[tau]_2`.
#[derive(Clone, Debug)]
pub struct Srs {
    /// `[tau^i]_1`
    pub g: Vec<G1Affine>,
    /// `[gamma * tau^i]_1`, the hiding basis.
    pub gamma_g: Vec<G1Affine>,
    /// `[1]_2`
    pub h: G2Affine,
    /// `[tau]_2`
    pub tau_h: G2Affine,
}

/// The part of the SRS a verifier needs.
#[derive(Clone, Copy, Debug)]
pub struct VerifierKey {
    pub g: G1Affine,
    pub gamma_g: G1Affine,
    pub h: G2Affine,
    pub tau_h: G2Affine,
}

/// Degree of the blinding polynomial of a book-column commitment.
pub const BOOK_BLINDING_DEGREE: usize = 3;

/// Public data tied to one evaluation domain of size `n`, derived from the same `tau` as the SRS.
#[derive(Clone, Debug)]
pub struct DomainKey {
    pub n: usize,
    /// `[L_i(tau)]_1`: the SRS in the Lagrange basis.
    pub lagrange_g: Vec<G1Affine>,
    /// `[tau^n - 1]_2 = [Z_H(tau)]_2`, for the empty-book check.
    pub vanishing_h: G2Affine,
    /// `[gamma * tau^i]_1` for `i <= BOOK_BLINDING_DEGREE`.
    pub blinding_basis: Vec<G1Affine>,
}

const SETUP_MAGIC: &[u8; 8] = b"ZQKZGSRS";

/// Serializes an SRS and its domain key (uncompressed points).
pub fn setup_to_bytes(srs: &Srs, key: &DomainKey) -> Vec<u8> {
    let mut w = codec::Writer::new(SETUP_MAGIC);
    w.u64(key.n as u64);
    w.g1s(&srs.g);
    w.g1s(&srs.gamma_g);
    w.g2(&srs.h);
    w.g2(&srs.tau_h);
    w.g1s(&key.lagrange_g);
    w.g2(&key.vanishing_h);
    w.finish()
}

/// Reads a setup written by [`setup_to_bytes`].
pub fn setup_from_bytes(bytes: &[u8]) -> Option<(Srs, DomainKey)> {
    let mut r = codec::Reader::new(bytes, SETUP_MAGIC)?;
    let n = usize::try_from(r.u64()?).ok()?;
    let g = r.g1s_unchecked()?;
    let gamma_g = r.g1s_unchecked()?;
    let h = r.g2()?;
    let tau_h = r.g2()?;
    let lagrange_g = r.g1s_unchecked()?;
    let vanishing_h = r.g2()?;
    r.finish()?;
    if g.is_empty() || gamma_g.len() <= BOOK_BLINDING_DEGREE || lagrange_g.len() != n {
        return None;
    }
    let blinding_basis = gamma_g[..=BOOK_BLINDING_DEGREE].to_vec();
    let srs = Srs { g, gamma_g, h, tau_h };
    Some((srs, DomainKey { n, lagrange_g, vanishing_h, blinding_basis }))
}

impl DomainKey {
    /// Derives the domain key from the SRS points alone.
    pub fn from_powers(n: usize, srs: &Srs, tau_n_h: G2Affine) -> Option<Self> {
        let domain = Radix2EvaluationDomain::<Fr>::new(n).filter(|d| d.size() == n)?;
        if srs.g.len() < n || srs.gamma_g.len() <= BOOK_BLINDING_DEGREE {
            return None;
        }
        let monomial: Vec<G1Projective> = srs.g[..n].iter().map(|p| p.into_group()).collect();
        let lagrange = domain.ifft(&monomial);
        Some(Self {
            n,
            lagrange_g: G1Projective::normalize_batch(&lagrange),
            vanishing_h: (tau_n_h.into_group() - srs.h.into_group()).into_affine(),
            blinding_basis: srs.gamma_g[..=BOOK_BLINDING_DEGREE].to_vec(),
        })
    }

    /// A key for committing in the Lagrange basis of a second domain of size `n`, derived from the SRS.
    pub fn lagrange_only(n: usize, srs: &Srs) -> Option<Self> {
        let mut key = Self::from_powers(n, srs, srs.h)?;
        key.vanishing_h = G2Affine::identity();
        Some(key)
    }

    /// Unblinded commitment to the polynomial with these values on the domain.
    pub fn commit_evals(&self, evals: &[Fr]) -> G1Projective {
        let len = evals.len().min(self.lagrange_g.len());
        G1Projective::msm_unchecked(&self.lagrange_g[..len], &evals[..len])
    }

    /// The same for a 0/1 column: additions only.
    pub fn commit_bits(&self, bits: &[u64]) -> G1Projective {
        let mut acc = G1Projective::zero();
        for (point, bit) in self.lagrange_g.iter().zip(bits) {
            if *bit == 1 {
                acc += point;
            }
        }
        acc
    }
}

/// Commitment to `rho(X) * (X^n - 1)`, the off-domain mask of a column.
pub fn commit_mask(srs: &Srs, n: usize, rho: &[Fr]) -> Result<G1Projective, KzgError> {
    if n + rho.len() > srs.g.len() {
        return Err(KzgError::SrsTooSmall {
            needed: n + rho.len(),
            available: srs.g.len(),
        });
    }
    Ok(msm(&srs.g[n..], rho)? - msm(&srs.g, rho)?)
}

/// The blinding part of a commitment, `[gamma * r(tau)]_1`.
pub fn commit_blinding(srs: &Srs, blinding: &Blinding) -> Result<G1Projective, KzgError> {
    msm(&srs.gamma_g, &blinding.0.coeffs)
}

impl Srs {
    /// Generates an SRS from a locally sampled `tau`.
    pub fn insecure_setup<R: RngCore>(max_degree: usize, rng: &mut R) -> Self {
        Self::from_secrets(max_degree, Fr::rand(rng), Fr::rand(rng))
    }

    /// As [`Srs::insecure_setup`], and also the [`DomainKey`] for a domain of size `n`.
    pub fn insecure_setup_for_domain<R: RngCore>(
        max_degree: usize,
        n: usize,
        rng: &mut R,
    ) -> Option<(Self, DomainKey)> {
        let tau = Fr::rand(rng);
        let srs = Self::from_secrets(max_degree.max(BOOK_BLINDING_DEGREE), tau, Fr::rand(rng));
        let domain = Radix2EvaluationDomain::<Fr>::new(n).filter(|d| d.size() == n)?;
        let g = G1Projective::generator();
        let lagrange: Vec<G1Projective> = domain
            .evaluate_all_lagrange_coefficients(tau)
            .into_iter()
            .map(|coefficient| g * coefficient)
            .collect();
        let vanishing = tau.pow([n as u64]) - Fr::one();
        let key = DomainKey {
            n,
            lagrange_g: G1Projective::normalize_batch(&lagrange),
            vanishing_h: (G2Projective::generator() * vanishing).into_affine(),
            blinding_basis: srs.gamma_g[..=BOOK_BLINDING_DEGREE].to_vec(),
        };
        Some((srs, key))
    }

    fn from_secrets(max_degree: usize, tau: Fr, gamma: Fr) -> Self {
        let g = G1Projective::generator();
        let gamma_g = g * gamma;

        let mut g_powers = Vec::with_capacity(max_degree + 1);
        let mut gamma_g_powers = Vec::with_capacity(max_degree + 1);
        let mut tau_power = Fr::one();
        for _ in 0..=max_degree {
            g_powers.push(g * tau_power);
            gamma_g_powers.push(gamma_g * tau_power);
            tau_power *= tau;
        }

        let h = G2Projective::generator();
        Self {
            g: G1Projective::normalize_batch(&g_powers),
            gamma_g: G1Projective::normalize_batch(&gamma_g_powers),
            h: h.into_affine(),
            tau_h: (h * tau).into_affine(),
        }
    }

    /// Checks that the SRS and the domain key are internally consistent.
    pub fn is_consistent<R: RngCore>(&self, key: &DomainKey, rng: &mut R) -> bool {
        let n = key.n;
        let Some(domain) = Radix2EvaluationDomain::<Fr>::new(n).filter(|d| d.size() == n) else {
            return false;
        };
        if self.g.len() <= n
            || self.gamma_g.len() <= BOOK_BLINDING_DEGREE
            || key.lagrange_g.len() != n
            || self.g[0] != G1Affine::generator()
            || self.h != G2Affine::generator()
            || key.blinding_basis[..] != self.gamma_g[..=BOOK_BLINDING_DEGREE]
        {
            return false;
        }
        // Each list is a geometric sequence in tau.
        let geometric = |points: &[G1Affine], rng: &mut R| {
            let weights: Vec<Fr> = (1..points.len()).map(|_| Fr::rand(rng)).collect();
            let low = G1Projective::msm_unchecked(&points[..points.len() - 1], &weights);
            let high = G1Projective::msm_unchecked(&points[1..], &weights);
            Bn254::pairing(high.into_affine(), self.h) == Bn254::pairing(low.into_affine(), self.tau_h)
        };
        if !geometric(&self.g, rng) || !geometric(&self.gamma_g, rng) {
            return false;
        }
        // [Z_H(tau)]_2 matches [tau^n - 1]_1.
        let vanishing_g = (self.g[n].into_group() - self.g[0]).into_affine();
        if Bn254::pairing(vanishing_g, self.h) != Bn254::pairing(self.g[0], key.vanishing_h) {
            return false;
        }
        // The Lagrange basis matches the powers.
        let evals: Vec<Fr> = (0..n).map(|_| Fr::rand(rng)).collect();
        let coeffs = domain.ifft(&evals);
        key.commit_evals(&evals) == G1Projective::msm_unchecked(&self.g[..n], &coeffs)
    }

    pub fn max_degree(&self) -> usize {
        self.g.len() - 1
    }

    pub fn verifier_key(&self) -> VerifierKey {
        VerifierKey {
            g: self.g[0],
            gamma_g: self.gamma_g[0],
            h: self.h,
            tau_h: self.tau_h,
        }
    }
}

/// A KZG commitment (one G1 point).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Commitment(pub G1Affine);

/// The blinding polynomial `r` of a commitment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Blinding(pub Poly);

impl Blinding {
    /// No blinding.
    pub fn none() -> Self {
        Self(Poly::from_coefficients_vec(vec![]))
    }

    /// A uniformly random blinding polynomial of the given degree.
    pub fn random<R: RngCore>(degree: usize, rng: &mut R) -> Self {
        Self(Poly::from_coefficients_vec(
            (0..=degree).map(|_| Fr::rand(rng)).collect(),
        ))
    }
}

fn msm(bases: &[G1Affine], scalars: &[Fr]) -> Result<G1Projective, KzgError> {
    if scalars.len() > bases.len() {
        return Err(KzgError::SrsTooSmall {
            needed: scalars.len(),
            available: bases.len(),
        });
    }
    Ok(G1Projective::msm_unchecked(&bases[..scalars.len()], scalars))
}

/// Commits to `poly` with an explicit blinding polynomial.
pub fn commit_with(srs: &Srs, poly: &Poly, blinding: &Blinding) -> Result<Commitment, KzgError> {
    let point = msm(&srs.g, &poly.coeffs)? + msm(&srs.gamma_g, &blinding.0.coeffs)?;
    Ok(Commitment(point.into_affine()))
}

/// Commits to `poly` with a fresh random blinding polynomial of degree `hiding_degree`.
pub fn commit<R: RngCore>(
    srs: &Srs,
    poly: &Poly,
    hiding_degree: usize,
    rng: &mut R,
) -> Result<(Commitment, Blinding), KzgError> {
    let blinding = Blinding::random(hiding_degree, rng);
    let commitment = commit_with(srs, poly, &blinding)?;
    Ok((commitment, blinding))
}

/// `acc += scale * coeffs`, growing `acc` if needed.
fn add_scaled(acc: &mut Vec<Fr>, coeffs: &[Fr], scale: Fr) {
    if acc.len() < coeffs.len() {
        acc.resize(coeffs.len(), Fr::zero());
    }
    for (a, c) in acc.iter_mut().zip(coeffs) {
        *a += *c * scale;
    }
}

/// Divides by `(X - z)` and returns the quotient, discarding the remainder.
fn divide_by_linear(coeffs: &[Fr], z: Fr) -> Vec<Fr> {
    if coeffs.len() < 2 {
        return vec![];
    }
    let mut quotient = vec![Fr::zero(); coeffs.len() - 1];
    let mut carry = Fr::zero();
    for i in (1..coeffs.len()).rev() {
        carry = coeffs[i] + z * carry;
        quotient[i - 1] = carry;
    }
    quotient
}

/// A batched opening of several polynomials at one point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Opening {
    pub point: Fr,
    /// `p_i(point)`, in the order the polynomials were given.
    pub evals: Vec<Fr>,
    /// `sum_i gamma^i * r_i(point)`.
    pub blinding_eval: Fr,
    /// Commitment to the quotient of the combined polynomial.
    pub witness: G1Affine,
}

/// Opens `polys` at `point`, combined with powers of `gamma`.
pub fn batch_open(
    srs: &Srs,
    polys: &[&Poly],
    blindings: &[&Blinding],
    point: Fr,
    gamma: Fr,
) -> Result<Opening, KzgError> {
    if polys.len() != blindings.len() {
        return Err(KzgError::LengthMismatch);
    }
    let mut combined = Vec::new();
    let mut combined_blinding = Vec::new();
    let mut power = Fr::one();
    for (poly, blinding) in polys.iter().zip(blindings) {
        add_scaled(&mut combined, &poly.coeffs, power);
        add_scaled(&mut combined_blinding, &blinding.0.coeffs, power);
        power *= gamma;
    }

    let witness = msm(&srs.g, &divide_by_linear(&combined, point))?
        + msm(&srs.gamma_g, &divide_by_linear(&combined_blinding, point))?;

    Ok(Opening {
        point,
        evals: evaluate_all(polys, point),
        blinding_eval: Poly::from_coefficients_vec(combined_blinding).evaluate(&point),
        witness: witness.into_affine(),
    })
}

/// Evaluates every polynomial at `point`.
pub fn evaluate_all(polys: &[&Poly], point: Fr) -> Vec<Fr> {
    polys.iter().map(|poly| poly.evaluate(&point)).collect()
}

/// One opening to verify.
#[derive(Clone, Copy, Debug)]
pub struct OpeningClaim<'a> {
    pub commitments: &'a [Commitment],
    pub opening: &'a Opening,
    pub gamma: Fr,
}

/// Verifies openings at any number of points with two pairings.
pub fn batch_verify(vk: &VerifierKey, claims: &[OpeningClaim], r: Fr) -> bool {
    // The left side is a single MSM.
    let mut bases = Vec::new();
    let mut scalars = Vec::new();
    let mut value_total = Fr::zero();
    let mut blinding_total = Fr::zero();
    let mut witness_sum = G1Projective::zero();
    let mut r_power = Fr::one();

    for claim in claims {
        let opening = claim.opening;
        if claim.commitments.len() != opening.evals.len() {
            return false;
        }
        let mut weight = r_power;
        for (commitment, eval) in claim.commitments.iter().zip(&opening.evals) {
            bases.push(commitment.0);
            scalars.push(weight);
            value_total += *eval * weight;
            weight *= claim.gamma;
        }
        blinding_total += opening.blinding_eval * r_power;
        bases.push(opening.witness);
        scalars.push(opening.point * r_power);
        witness_sum += opening.witness * r_power;
        r_power *= r;
    }
    bases.push(vk.g);
    scalars.push(-value_total);
    bases.push(vk.gamma_g);
    scalars.push(-blinding_total);
    let left = G1Projective::msm_unchecked(&bases, &scalars);

    Bn254::pairing(left.into_affine(), vk.h) == Bn254::pairing(witness_sum.into_affine(), vk.tau_h)
}
