//! # ZK-FBA Library
//!
//! Five-layer pipeline for proving Frequent Batch Auction arithmetic
//! using KZG polynomial commitments over BN254:
//!
//! ```text
//! Layer 1  Array computation       -- bid depth, ask depth, min array
//! Layer 2  Polynomial interpolation-- 5 witness polynomials over Fr / H
//! Layer 3a KZG trusted setup       -- simulated Powers-of-Tau SRS
//! Layer 3b KZG commit              -- commit to 5 witness polynomials
//! Layer 3c Quotient polynomials    -- V(X) / divisor for each constraint
//! Layer 3d KZG commit quotients    -- commit to 5 quotient polynomials
//! Layer 4  Constraint verification -- algebraic check on the witness
//! ```
//!
//! ## Vanishing constraints (corrected forms, verified against spreadsheet data)
//!
//! | Label        | Polynomial identity over H |
//! |--------------|----------------------------|
//! | V_AccA_init  | Acc_A(w^0)    = Ask(w^0) |
//! | V_AccA_rec   | Acc_A(w*X) - Acc_A(X) - Ask(w*X) = 0  for allX != w^{N-1} |
//! | V_AccB_init  | Acc_B(w^{N-1}) = Bid(w^{N-1}) |
//! | V_AccB_rec   | Acc_B(X) - Acc_B(w*X) - Bid(X) = 0    for allX != w^{N-1} |
//! | V_KL         | (Acc_A(X)-Min(X))*(Acc_B(X)-Min(X)) = 0  for allX in H |
//!
//! ## Quotient divisors
//!
//! | Constraint  | Divisor D(X)   | Degree of Q = V/D |
//! |-------------|----------------|-------------------|
//! | V_AccA_init | (X - w^0)       | <= 30              |
//! | V_AccA_rec  | Z_H(X)=X^32-1  | <= 0 (constant)    |
//! | V_AccB_init | (X - w^{N-1}) | <= 30              |
//! | V_AccB_rec  | Z_H(X)=X^32-1  | <= 0 (constant)    |
//! | V_KL        | Z_H(X)=X^32-1  | <= 30              |
//!
//! ## Document corrections
//! The *Vanishing Summary* PDF has two sign errors and swapped initialisation
//! points. The *Kimia's Draft* PDF also has the wrong index in V_AccA_rec
//! (writes Ask(X) instead of Ask(w*X)). The forms above are correct.

use ark_bn254::{Bn254, Fr as F, G1Affine, G1Projective, G2Affine, G2Projective};
use ark_ec::{pairing::Pairing, CurveGroup, Group, VariableBaseMSM};
use ark_ff::{Field, One, PrimeField, UniformRand, Zero};
use ark_poly::{
    evaluations::univariate::Evaluations, univariate::DensePolynomial,
    EvaluationDomain, GeneralEvaluationDomain, Polynomial,
};
use ark_serialize::CanonicalSerialize;
use ark_std::rand::Rng;
use sha2::{Digest, Sha256};

// ===========================================================================
// #0  Constants -- spreadsheet data (21 price ticks: 0, 10, 20 ... 110)
// ===========================================================================

pub const N: usize = 21;           // number of price ticks
pub const DOMAIN_SIZE: usize = 32; // next power of 2 >= N  (for NTT)

pub const PRICES: [u64; N] = [
    0, 10, 20, 30, 40, 50, 60, 70, 80, 90,
    100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110,
];
pub const BIDS_RAW: [u64; N] = [
    100, 100, 100, 200, 200, 500, 1000, 1500, 2000, 700,
    100, 100, 100, 0, 0, 0, 500, 500, 1000, 1000, 2000,
];
pub const ASKS_RAW: [u64; N] = [
    0, 20, 30, 50, 100, 200, 400, 700, 1000, 1500,
    1000, 0, 0, 0, 0, 0, 100, 100, 200, 200, 200,
];

// ===========================================================================
// #1  Layer 1 -- Array Computation
// ===========================================================================

/// Return the first 64-bit limb of a field element. Safe here because all
/// auction volumes are far below 2^6^4.
#[inline]
pub fn to_u64(f: F) -> u64 {
    f.into_bigint().as_ref()[0]
}

/// **Demand accumulator** Acc_B (bid depth): backward cumulative sum.
/// `Acc_B[n-1] = Bid[n-1]`,  `Acc_B[i] = Acc_B[i+1] + Bid[i]`  i = n-2...0
pub fn compute_bid_depth(bids: &[F; N]) -> [F; N] {
    let mut d = [F::zero(); N];
    d[N - 1] = bids[N - 1];
    for i in (0..N - 1).rev() {
        d[i] = d[i + 1] + bids[i];
    }
    d
}

/// **Supply accumulator** Acc_A (ask depth): forward cumulative sum.
/// `Acc_A[0] = Ask[0]`,  `Acc_A[i] = Acc_A[i-1] + Ask[i]`  i = 1...n-1
pub fn compute_ask_depth(asks: &[F; N]) -> [F; N] {
    let mut d = [F::zero(); N];
    d[0] = asks[0];
    for i in 1..N {
        d[i] = d[i - 1] + asks[i];
    }
    d
}

/// **Min array** (MCV candidates): `Min[i] = min(Acc_B[i], Acc_A[i])`.
/// The global maximum of this array is the Market Clearing Volume.
pub fn compute_min(bid_depth: &[F; N], ask_depth: &[F; N]) -> [F; N] {
    let mut m = [F::zero(); N];
    for i in 0..N {
        m[i] = if to_u64(bid_depth[i]) <= to_u64(ask_depth[i]) {
            bid_depth[i]
        } else {
            ask_depth[i]
        };
    }
    m
}

/// Compute all five arrays from the hard-coded constants.
pub fn compute_all_arrays() -> ([F; N], [F; N], [F; N], [F; N], [F; N]) {
    let bids: [F; N] = BIDS_RAW.map(F::from);
    let asks: [F; N] = ASKS_RAW.map(F::from);
    let bid_depth = compute_bid_depth(&bids);
    let ask_depth = compute_ask_depth(&asks);
    let min_arr   = compute_min(&bid_depth, &ask_depth);
    (bids, asks, bid_depth, ask_depth, min_arr)
}

// ===========================================================================
// #2  Layer 2 -- Polynomial Interpolation
// ===========================================================================

/// Multiplicative subgroup H of size `DOMAIN_SIZE = 32` in the BN254 scalar
/// field. The generator w satisfies w^{32} = 1.
pub fn build_domain() -> GeneralEvaluationDomain<F> {
    GeneralEvaluationDomain::new(DOMAIN_SIZE).expect("domain creation failed")
}

/// Interpolate an N-element array -> polynomial over `domain`.
/// Padded with zeros to DOMAIN_SIZE before IFFT so that
/// `p(w^i) = vals[i]` for i < N and `p(w^i) = 0` for N <= i < DOMAIN_SIZE.
pub fn interpolate(vals: &[F; N], domain: &GeneralEvaluationDomain<F>) -> DensePolynomial<F> {
    let mut evals = vals.to_vec();
    evals.resize(DOMAIN_SIZE, F::zero());
    Evaluations::from_vec_and_domain(evals, *domain).interpolate()
}

/// All five witness polynomials.
pub struct Polynomials {
    pub bid:   DensePolynomial<F>, // P_Bid(X)
    pub ask:   DensePolynomial<F>, // P_Ask(X)
    pub acc_b: DensePolynomial<F>, // Acc_B(X) -- demand accumulator
    pub acc_a: DensePolynomial<F>, // Acc_A(X) -- supply accumulator
    pub min:   DensePolynomial<F>, // Min(X)
}

pub fn build_all_polys(
    bids: &[F; N], asks: &[F; N],
    bid_depth: &[F; N], ask_depth: &[F; N], min_arr: &[F; N],
    domain: &GeneralEvaluationDomain<F>,
) -> Polynomials {
    Polynomials {
        bid:   interpolate(bids,      domain),
        ask:   interpolate(asks,      domain),
        acc_b: interpolate(bid_depth, domain),
        acc_a: interpolate(ask_depth, domain),
        min:   interpolate(min_arr,   domain),
    }
}

// ===========================================================================
// #3a/b  Layer 3 -- KZG Polynomial Commitments
// ===========================================================================

/// KZG Structured Reference String.
/// In production this comes from a multi-party computation ceremony.
/// Here we simulate with a random local tau (the toxic waste).
pub struct KZGSetup {
    /// `srs_g1[i]` = [tau^i * G1]  for i = 0 ... degree
    pub srs_g1: Vec<G1Affine>,
    /// `srs_g2` = [tau * G2]  -- used in the opening-proof pairing check
    pub srs_g2: G2Affine,
    pub degree: usize,
}

impl KZGSetup {
    /// Build the SRS: `degree + 1` G1 scalar multiplications + one G2 multiplication.
    pub fn new(degree: usize, tau: F) -> Self {
        let g1 = G1Projective::generator();
        let g2 = G2Projective::generator();
        let mut srs_g1  = Vec::with_capacity(degree + 1);
        let mut tau_pow = F::one();
        for _ in 0..=degree {
            srs_g1.push((g1 * tau_pow).into_affine());
            tau_pow *= tau;
        }
        let srs_g2 = (g2 * tau).into_affine();
        KZGSetup { srs_g1, srs_g2, degree }
    }

    /// Commit to `poly` via MSM: `C = sum_i p_i * [tau^i*G1] = [p(tau)]_G1`.
    pub fn commit(&self, poly: &DensePolynomial<F>) -> G1Affine {
        let n = poly.coeffs.len();
        assert!(n <= self.srs_g1.len(),
            "poly degree {} > SRS degree {}", n - 1, self.degree);
        G1Projective::msm(&self.srs_g1[..n], &poly.coeffs[..n])
            .expect("MSM failed")
            .into_affine()
    }
}

pub struct Commitments {
    pub c_bid:   G1Affine,
    pub c_ask:   G1Affine,
    pub c_acc_b: G1Affine,
    pub c_acc_a: G1Affine,
    pub c_min:   G1Affine,
}

pub fn commit_all(setup: &KZGSetup, polys: &Polynomials) -> Commitments {
    Commitments {
        c_bid:   setup.commit(&polys.bid),
        c_ask:   setup.commit(&polys.ask),
        c_acc_b: setup.commit(&polys.acc_b),
        c_acc_a: setup.commit(&polys.acc_a),
        c_min:   setup.commit(&polys.min),
    }
}

// ===========================================================================
// #3c  Quotient Polynomial Computation
// ===========================================================================
//
// For each vanishing constraint V(X) = 0 on H, the prover must show V is
// divisible by the appropriate divisor D(X):
//
//   V(X) = Q(X) * D(X)   (zero remainder)
//
// The prover commits to Q(X). The verifier will later use the quotient
// commitment and a random evaluation point to confirm the relation holds
// without seeing the witness polynomials -- that is the next step (Fiat-Shamir).
//
// Two kinds of divisor are used here:
//
//  (a) Z_H(X) = X^{DOMAIN_SIZE} - 1  -- the vanishing polynomial of the full
//      evaluation domain H.  Used for "full-domain" constraints (recurrences
//      and the min exclusivity check) that must hold at every point of H.
//      If V(w^i) = 0 for all i in {0 ... DOMAIN_SIZE-1} then Z_H | V exactly.
//
//  (b) (X - w^j)  -- a single linear factor.  Used for "point" constraints
//      (initialisation conditions) that only need to hold at one specific
//      domain point w^j.  If V(w^j) = 0 then (X - w^j) | V exactly.
//
// In a full batched PLONK proof all five V_i are combined via random powers
// of alpha into one quotient before commitment. We keep them separate here for
// clarity, and each gets its own KZG commitment.

//Low-level polynomial helpers

/// Construct a `DensePolynomial` from a coefficient vector, trimming trailing zeros.
fn make_poly(mut coeffs: Vec<F>) -> DensePolynomial<F> {
    while coeffs.last().map(|x: &F| x.is_zero()).unwrap_or(false) {
        coeffs.pop();
    }
    DensePolynomial { coeffs }
}

/// Compute `p(w*X)` by multiplying coefficient `i` by `w^i`.
///
/// If `p(X) = sum p_i*X^i` then `p(w*X) = sum p_i*w^i*X^i`.
/// This is an O(d) field-multiplication pass, no polynomial multiplication needed.
pub fn shift_omega(poly: &DensePolynomial<F>, omega: F) -> DensePolynomial<F> {
    let mut omega_pow = F::one();
    let coeffs = poly.coeffs.iter().map(|&c| {
        let val = c * omega_pow;
        omega_pow *= omega;
        val
    }).collect();
    DensePolynomial { coeffs }
}

/// Compute `(X - a) * p(X)`.
fn mul_x_minus_a(poly: &DensePolynomial<F>, a: F) -> DensePolynomial<F> {
    if poly.coeffs.is_empty() {
        return DensePolynomial { coeffs: vec![] };
    }
    let n = poly.coeffs.len();
    let mut result = vec![F::zero(); n + 1];
    for (i, &c) in poly.coeffs.iter().enumerate() {
        result[i + 1] += c;       //  X * p
        result[i]     -= c * a;   // -a * p
    }
    make_poly(result)
}

/// Build `Z_H(X) = X^{DOMAIN_SIZE} - 1` (the vanishing polynomial of H).
pub fn build_zh_poly() -> DensePolynomial<F> {
    let mut coeffs = vec![F::zero(); DOMAIN_SIZE + 1];
    coeffs[0]           = -F::one(); // constant term = -1
    coeffs[DOMAIN_SIZE] =  F::one(); // leading term  = +1
    DensePolynomial { coeffs }
}

/// General polynomial long division.
///
/// Returns `(Q, R)` such that `num = Q * den + R`  and  `deg(R) < deg(den)`.
/// When `num` is divisible by `den`, `R` is the zero polynomial.
///
/// # Algorithm
/// Standard coefficient-space long division processed from the highest degree
/// downward, dividing the current leading term of the remainder by the leading
/// term of `den` at each step.
pub fn poly_div_rem(
    num: &DensePolynomial<F>,
    den: &DensePolynomial<F>,
) -> (DensePolynomial<F>, DensePolynomial<F>) {
    if num.coeffs.is_empty() {
        return (DensePolynomial { coeffs: vec![] },
                DensePolynomial { coeffs: vec![] });
    }
    let m = num.coeffs.len() - 1; // degree of numerator
    let n = den.coeffs.len() - 1; // degree of denominator
    if m < n {
        return (DensePolynomial { coeffs: vec![] }, num.clone());
    }

    let den_lead_inv = den.coeffs[n]
        .inverse()
        .expect("denominator leading coefficient is zero");

    let mut rem = num.coeffs.clone();
    let mut quot = vec![F::zero(); m - n + 1];

    // Process degree m down to degree n.
    for i in (n..=m).rev() {
        let q_coeff = rem[i] * den_lead_inv;
        quot[i - n] = q_coeff;
        for j in 0..=n {
            rem[i - n + j] -= q_coeff * den.coeffs[j];
        }
    }

    (make_poly(quot), make_poly(rem))
}

/// Synthetic division of `poly` by the linear factor `(X - a)`.
///
/// Returns `(Q, remainder)` where `poly = Q*(X-a) + remainder`.
/// If `poly(a) = 0`, then `remainder = 0` and the division is exact.
///
/// Synthetic division processes coefficients little-endian (constant first):
/// ```text
///   q[n-2] = p[n-1]
///   q[k]   = p[k+1] + a*q[k+1]   for k = n-3 ... 0
///   rem    = p[0]   + a*q[0]
/// ```
pub fn div_by_linear(poly: &DensePolynomial<F>, a: F) -> (DensePolynomial<F>, F) {
    let c = &poly.coeffs;
    match c.len() {
        0 => (DensePolynomial { coeffs: vec![] }, F::zero()),
        1 => (DensePolynomial { coeffs: vec![] }, c[0]),
        n => {
            let mut q = vec![F::zero(); n - 1];
            q[n - 2] = c[n - 1];
            for k in (0..n - 2).rev() {
                q[k] = c[k + 1] + a * q[k + 1];
            }
            let rem = c[0] + a * q[0];
            (make_poly(q), rem)
        }
    }
}

//Quotient structs

/// The five quotient polynomials, one per constraint.
pub struct QuotientPolynomials {
    /// Q_AccA_init = (Acc_A - Ask) / (X - w^0)
    /// Point constraint at w^0 (init of ask depth accumulator)
    pub q_acc_a_init: DensePolynomial<F>,

    /// Q_AccA_rec = [(X-w^{N-1})*(Acc_A(wX)-Acc_A(X)-Ask(wX))] / Z_H
    /// Full-domain transition constraint for ask depth recurrence
    pub q_acc_a_rec: DensePolynomial<F>,

    /// Q_AccB_init = (Acc_B - Bid) / (X - w^{N-1})
    /// Point constraint at w^{N-1} (init of bid depth accumulator)
    pub q_acc_b_init: DensePolynomial<F>,

    /// Q_AccB_rec = [(X-w^{N-1})*(Acc_B(X)-Acc_B(wX)-Bid(X))] / Z_H
    /// Full-domain transition constraint for bid depth recurrence
    pub q_acc_b_rec: DensePolynomial<F>,

    /// Q_KL = (Acc_A-Min)*(Acc_B-Min) / Z_H
    /// Full-domain mutual exclusivity (min proof)
    pub q_kl: DensePolynomial<F>,
}

impl QuotientPolynomials {
    pub fn max_degree(&self) -> usize {
        [
            self.q_acc_a_init.degree(),
            self.q_acc_a_rec.degree(),
            self.q_acc_b_init.degree(),
            self.q_acc_b_rec.degree(),
            self.q_kl.degree(),
        ]
        .into_iter()
        .max()
        .unwrap_or(0)
    }
}

/// Whether each division produced a zero remainder.
/// A `true` value is a **proof** that the corresponding vanishing constraint
/// holds over the entire evaluation domain.
pub struct QuotientCheck {
    pub r_acc_a_init: bool, // (Acc_A-Ask)(w^0)    = 0 ?
    pub r_acc_a_rec:  bool, // V_AccA_rec  in Z_H * F[X] ?
    pub r_acc_b_init: bool, // (Acc_B-Bid)(w^{N-1}) = 0 ?
    pub r_acc_b_rec:  bool, // V_AccB_rec  in Z_H * F[X] ?
    pub r_kl:         bool, // V_KL        in Z_H * F[X] ?
}

impl QuotientCheck {
    pub fn all_zero(&self) -> bool {
        self.r_acc_a_init && self.r_acc_a_rec
            && self.r_acc_b_init && self.r_acc_b_rec
            && self.r_kl
    }
}

//Main quotient computation

/// Compute all five quotient polynomials and check their remainders.
///
/// # How it works for each constraint
///
/// **V_AccA_init** (point at w^0):
///   `f = Acc_A - Ask`  is a degree-31 poly that must vanish at w^0.
///   `Q = f / (X - w^0)` via synthetic division.  `R in F` (a scalar).
///   If `R = 0` then `f(w^0) = 0` i.e. `Acc_A(w^0) = Ask(w^0)`. 
///
/// **V_AccA_rec** (full domain, all 32 points):
///   `g = Acc_A(wX) - Acc_A(X) - Ask(wX)` -- inner constraint polynomial.
///   `V = (X - w^{N-1}) * g` -- zero at w^{N-1} by the linear factor;
///       zero at all other domain points because the constraint holds there.
///   `Q = V / Z_H`  via long division.  `R` is a polynomial (must be zero).
///
/// **V_AccB_init** (point at w^{N-1}): mirror of AccA init.
///
/// **V_AccB_rec** (full domain): mirror of AccA rec.
///
/// **V_KL** (full domain, quadratic):
///   `V = (Acc_A - Min) * (Acc_B - Min)` -- degree-62 poly.
///   `Q = V / Z_H` -- degree <= 30.
pub fn compute_quotients(
    polys:  &Polynomials,
    domain: &GeneralEvaluationDomain<F>,
) -> (QuotientPolynomials, QuotientCheck) {
    let omega      = domain.group_gen();              // generator of H (32nd root of unity)
    let elems: Vec<F> = domain.elements().collect();
    let omega_0         = elems[0];              // w^0  -- lowest price  (index 0)
    let omega_last_data = elems[N - 1];          // w^20 -- highest price (index N-1=20)
    let omega_last_dom  = elems[DOMAIN_SIZE - 1];// w^31 -- actual last domain point (index 31)
    let zh         = build_zh_poly();             // Z_H(X) = X^32 - 1

    //V_AccA_init: (Acc_A - Ask) / (X - w^0)
    let f_a_init  = &polys.acc_a - &polys.ask;
    let (q1, r1)  = div_by_linear(&f_a_init, omega_0);

    //V_AccA_rec: (X-w^{N-1})*(Acc_A(wX)-Acc_A(X)-Ask(wX)) / Z_H
    //
    // The ask-depth accumulator goes FORWARD (index 0 -> N-1).  The inner
    // polynomial g_2 fails at w^{N-1} = w^20 (last data -> first padding
    // transition, where Acc_A(w^2^1)=0 but Acc_A(w^20)=5800!=0).
    // Multiplying by (X - w^{N-1}) zeroes that point out.
    // The cyclic wrap-around at w^31 is harmless here because
    // Acc_A(w^0) = ask_depth[0] = 0 = Ask(w^0), so g_2(w^31) = 0 - 0 - 0 = 0.
    let acc_a_next = shift_omega(&polys.acc_a, omega); // Acc_A(wX)
    let ask_next   = shift_omega(&polys.ask,   omega); // Ask(wX)
    let g2  = &(&acc_a_next - &polys.acc_a) - &ask_next;
    let v2  = mul_x_minus_a(&g2, omega_last_data);     // skip at w^20
    let (q2, r2) = poly_div_rem(&v2, &zh);

    //V_AccB_init: (Acc_B - Bid) / (X - w^{N-1})
    let f_b_init  = &polys.acc_b - &polys.bid;
    let (q3, r3)  = div_by_linear(&f_b_init, omega_last_data);

    //V_AccB_rec: (X-w^31)*(Acc_B(X)-Acc_B(wX)-Bid(X)) / Z_H
    //
    // The bid-depth accumulator goes BACKWARD (index N-1 -> 0).  The inner
    // polynomial g_4 actually holds at w^20 (bid_depth[20]=bids[20]=2000) and
    // at all padded points w^2^1...w^3^0.  However the cyclic wrap-around at w^31
    // breaks it: g_4(w^31) = Acc_B(w^0) - Acc_B(w^32) - Bid(w^31)
    //                     = bid_depth[0] - 0 - 0 = 11700 != 0.
    // Fix: multiply by (X - w^31) to skip the actual last domain point,
    // which is the standard PLONK convention.
    let acc_b_next = shift_omega(&polys.acc_b, omega); // Acc_B(wX)
    let g4  = &(&polys.acc_b - &acc_b_next) - &polys.bid;
    let v4  = mul_x_minus_a(&g4, omega_last_dom);      // skip at w^31
    let (q4, r4) = poly_div_rem(&v4, &zh);

    //V_KL: (Acc_A-Min)*(Acc_B-Min) / Z_H
    //
    // This is the min exclusivity constraint. The product vanishes at every
    // point of H because at each tick Min equals exactly one of the two
    // accumulators, making one factor zero. This gives a degree-62 V, and
    // Q has degree <= 30 -- the most non-trivial quotient in the protocol.
    let da = &polys.acc_a - &polys.min;
    let db = &polys.acc_b - &polys.min;
    let v5 = &da * &db;
    let (q5, r5) = poly_div_rem(&v5, &zh);

    let quotients = QuotientPolynomials {
        q_acc_a_init: q1,
        q_acc_a_rec:  q2,
        q_acc_b_init: q3,
        q_acc_b_rec:  q4,
        q_kl:         q5,
    };

    let check = QuotientCheck {
        r_acc_a_init: r1.is_zero(),
        r_acc_a_rec:  r2.coeffs.iter().all(|x| x.is_zero()),
        r_acc_b_init: r3.is_zero(),
        r_acc_b_rec:  r4.coeffs.iter().all(|x| x.is_zero()),
        r_kl:         r5.coeffs.iter().all(|x| x.is_zero()),
    };

    (quotients, check)
}

//Quotient KZG commitments

pub struct QuotientCommitments {
    pub c_q_acc_a_init: G1Affine,
    pub c_q_acc_a_rec:  G1Affine,
    pub c_q_acc_b_init: G1Affine,
    pub c_q_acc_b_rec:  G1Affine,
    pub c_q_kl:         G1Affine,
}

/// Commit to all five quotient polynomials using the same SRS.
pub fn commit_quotients(setup: &KZGSetup, q: &QuotientPolynomials) -> QuotientCommitments {
    QuotientCommitments {
        c_q_acc_a_init: setup.commit(&q.q_acc_a_init),
        c_q_acc_a_rec:  setup.commit(&q.q_acc_a_rec),
        c_q_acc_b_init: setup.commit(&q.q_acc_b_init),
        c_q_acc_b_rec:  setup.commit(&q.q_acc_b_rec),
        c_q_kl:         setup.commit(&q.q_kl),
    }
}

// ===========================================================================
// #4  Layer 4 -- Constraint Verification (algebraic witness check)
// ===========================================================================

pub struct ConstraintResult {
    pub acc_a_init:       bool,
    pub acc_a_recurrence: bool,
    pub acc_b_init:       bool,
    pub acc_b_recurrence: bool,
    pub min_exclusivity:  bool,
}

impl ConstraintResult {
    pub fn all_pass(&self) -> bool {
        self.acc_a_init && self.acc_a_recurrence
            && self.acc_b_init && self.acc_b_recurrence
            && self.min_exclusivity
    }
}

pub fn verify_all(
    polys:  &Polynomials,
    domain: &GeneralEvaluationDomain<F>,
) -> ConstraintResult {
    let elems: Vec<F> = domain.elements().collect();
    let w0    = elems[0];
    let wlast = elems[N - 1];
    ConstraintResult {
        acc_a_init:       c_acc_a_init(&polys.acc_a, &polys.ask, w0),
        acc_a_recurrence: c_acc_a_rec(&polys.acc_a, &polys.ask, &elems),
        acc_b_init:       c_acc_b_init(&polys.acc_b, &polys.bid, wlast),
        acc_b_recurrence: c_acc_b_rec(&polys.acc_b, &polys.bid, &elems),
        min_exclusivity:  c_min_excl(&polys.acc_a, &polys.acc_b, &polys.min, &elems),
    }
}

fn c_acc_a_init(a: &DensePolynomial<F>, ask: &DensePolynomial<F>, w0: F) -> bool {
    a.evaluate(&w0) == ask.evaluate(&w0)
}
fn c_acc_a_rec(a: &DensePolynomial<F>, ask: &DensePolynomial<F>, e: &[F]) -> bool {
    for i in 0..N - 1 {
        if a.evaluate(&e[i+1]) != a.evaluate(&e[i]) + ask.evaluate(&e[i+1]) {
            return false;
        }
    }
    true
}
fn c_acc_b_init(b: &DensePolynomial<F>, bid: &DensePolynomial<F>, wl: F) -> bool {
    b.evaluate(&wl) == bid.evaluate(&wl)
}
fn c_acc_b_rec(b: &DensePolynomial<F>, bid: &DensePolynomial<F>, e: &[F]) -> bool {
    for i in 0..N - 1 {
        if b.evaluate(&e[i]) != b.evaluate(&e[i+1]) + bid.evaluate(&e[i]) {
            return false;
        }
    }
    true
}
fn c_min_excl(a: &DensePolynomial<F>, b: &DensePolynomial<F>,
              m: &DensePolynomial<F>, e: &[F]) -> bool {
    for i in 0..N {
        let da = a.evaluate(&e[i]) - m.evaluate(&e[i]);
        let db = b.evaluate(&e[i]) - m.evaluate(&e[i]);
        if !(da * db).is_zero() { return false; }
    }
    true
}

// ===========================================================================
// #3f  Layer 3f -- KZG Opening Proofs + Pairing Verification
// ===========================================================================
//
// After Fiat-Shamir the prover has a challenge zeta and a set of *claimed*
// polynomial evaluations.  Nothing yet stops a cheating prover from lying
// about those values -- the commitments and the evaluations are not linked.
//
// KZG opening proofs close this gap.  For each committed polynomial p(X)
// and claimed value y = p(zeta), the prover computes:
//
//   pi = [(p(X) - y) / (X - zeta)]_{G1}
//       = [quotient(tau)]_{G1}           via MSM on the G1 SRS
//
// The polynomial (p(X) - y) has p(zeta) - y = 0 as a root, so (X - zeta)
// divides it exactly.  The prover cannot fake pi for a wrong y without
// knowing tau (the discrete-log problem).
//
// The verifier checks using a pairing equation (no knowledge of tau needed):
//
//   e( C - y*G1,  G2 )  ==  e( pi,  [tau]G2 - zeta*G2 )
//
// Both sides equal e(G1, G2)^{p(tau)-y} when the proof is honest.
//
// Batched verification (2 pairings for n polynomials at the same zeta):
//
//   e( sum_i r^i*(C_i - y_i*G1),  G2 )  ==  e( sum_i r^i*pi_i,  [tau]G2 - zeta*G2 )
//
// where r is a random scalar (reuse fs.alpha here).  Accumulating with r^i
// ensures a cheating prover cannot cancel errors across polynomials.
//
// This codebase opens all 10 committed polynomials at zeta (5 witness + 5
// quotient) plus 3 polynomials at w*zeta (Acc_A, Ask, Acc_B -- needed by the
// recurrence constraints).  Total: 13 opening proofs, each one G1 point.
// Verification costs 26 pairings individually, or 4 pairings when batched
// (2 for the zeta group, 2 for the w*zeta group).

//Opening proof computation

/// Compute a single KZG opening proof for `poly` at `zeta`.
///
/// Uses synthetic division (`div_by_linear`) which returns
/// `(quotient, rem)` where `poly = quotient*(X-zeta) + rem`; here `rem = poly(zeta)`.
/// The commitment to `quotient` is the opening proof pi.
///
/// Returns `(pi, y)` where `y = poly(zeta)`.
pub fn compute_opening_proof(
    setup: &KZGSetup,
    poly:  &DensePolynomial<F>,
    zeta:  F,
) -> (G1Affine, F) {
    let (quotient, y) = div_by_linear(poly, zeta);
    let pi = if quotient.coeffs.is_empty() {
        // Zero polynomial (constant poly evaluated at zeta == constant) -> identity
        G1Projective::zero().into_affine()
    } else {
        setup.commit(&quotient)
    };
    (pi, y)
}

/// Opening proofs for all 13 evaluation claims in the proof:
/// 10 polynomials opened at zeta, plus 3 polynomials opened at w*zeta.
pub struct OpeningProofs {
    //At zeta -- all 10 committed polynomials
    pub pi_bid:          G1Affine,
    pub pi_ask:          G1Affine,
    pub pi_acc_b:        G1Affine,
    pub pi_acc_a:        G1Affine,
    pub pi_min:          G1Affine,
    pub pi_q_acc_a_init: G1Affine,
    pub pi_q_acc_a_rec:  G1Affine,
    pub pi_q_acc_b_init: G1Affine,
    pub pi_q_acc_b_rec:  G1Affine,
    pub pi_q_kl:         G1Affine,
    //At w*zeta -- shifted evaluations for recurrence constraints
    pub pi_acc_a_shift: G1Affine, // Acc_A opened at w*zeta
    pub pi_ask_shift:   G1Affine, // Ask   opened at w*zeta
    pub pi_acc_b_shift: G1Affine, // Acc_B opened at w*zeta
}

/// Compute all 13 opening proofs.
pub fn compute_all_opening_proofs(
    setup:  &KZGSetup,
    polys:  &Polynomials,
    quots:  &QuotientPolynomials,
    fs:     &FiatShamirProof,
    domain: &GeneralEvaluationDomain<F>,
) -> OpeningProofs {
    let zeta        = fs.zeta;
    let omega_zeta  = domain.group_gen() * zeta;

    let (pi_bid,          _) = compute_opening_proof(setup, &polys.bid,          zeta);
    let (pi_ask,          _) = compute_opening_proof(setup, &polys.ask,          zeta);
    let (pi_acc_b,        _) = compute_opening_proof(setup, &polys.acc_b,        zeta);
    let (pi_acc_a,        _) = compute_opening_proof(setup, &polys.acc_a,        zeta);
    let (pi_min,          _) = compute_opening_proof(setup, &polys.min,          zeta);
    let (pi_q_acc_a_init, _) = compute_opening_proof(setup, &quots.q_acc_a_init, zeta);
    let (pi_q_acc_a_rec,  _) = compute_opening_proof(setup, &quots.q_acc_a_rec,  zeta);
    let (pi_q_acc_b_init, _) = compute_opening_proof(setup, &quots.q_acc_b_init, zeta);
    let (pi_q_acc_b_rec,  _) = compute_opening_proof(setup, &quots.q_acc_b_rec,  zeta);
    let (pi_q_kl,         _) = compute_opening_proof(setup, &quots.q_kl,         zeta);

    let (pi_acc_a_shift, _) = compute_opening_proof(setup, &polys.acc_a, omega_zeta);
    let (pi_ask_shift,   _) = compute_opening_proof(setup, &polys.ask,   omega_zeta);
    let (pi_acc_b_shift, _) = compute_opening_proof(setup, &polys.acc_b, omega_zeta);

    OpeningProofs {
        pi_bid, pi_ask, pi_acc_b, pi_acc_a, pi_min,
        pi_q_acc_a_init, pi_q_acc_a_rec,
        pi_q_acc_b_init, pi_q_acc_b_rec, pi_q_kl,
        pi_acc_a_shift, pi_ask_shift, pi_acc_b_shift,
    }
}

//Opening proof verification

/// Verify one KZG opening proof via a pairing check:
///
/// `e( C - y*G1,  G2 )  ==  e( pi,  [tau]G2 - zeta*G2 )`
pub fn verify_opening(
    setup: &KZGSetup,
    comm:  G1Affine,
    pi:    G1Affine,
    zeta:  F,
    y:     F,
) -> bool {
    let g1     = G1Projective::generator();
    let g2     = G2Projective::generator();
    let g2_gen = g2.into_affine();
    let lhs_g1 = (G1Projective::from(comm) - g1 * y).into_affine();
    let rhs_g2 = (G2Projective::from(setup.srs_g2) - g2 * zeta).into_affine();
    Bn254::pairing(lhs_g1, g2_gen) == Bn254::pairing(pi, rhs_g2)
}

/// Batch-verify n openings at the same zeta using 2 pairings total.
///
/// Accumulate with random scalar `r`:
/// `e( sum_i r^i*(C_i-y_i*G1),  G2 )  ==  e( sum_i r^i*pi_i,  [tau]G2-zeta*G2 )`
pub fn verify_openings_batched(
    setup:  &KZGSetup,
    comms:  &[G1Affine],
    proofs: &[G1Affine],
    evals:  &[F],
    zeta:   F,
    r:      F,
) -> bool {
    let g1 = G1Projective::generator();
    let g2 = G2Projective::generator();
    let mut r_pow = F::one();
    let mut lhs   = G1Projective::zero();
    let mut rhs   = G1Projective::zero();
    for i in 0..comms.len() {
        lhs += (G1Projective::from(comms[i]) - g1 * evals[i]) * r_pow;
        rhs += G1Projective::from(proofs[i]) * r_pow;
        r_pow *= r;
    }
    let g2_gen = g2.into_affine();
    let rhs_g2 = (G2Projective::from(setup.srs_g2) - g2 * zeta).into_affine();
    Bn254::pairing(lhs.into_affine(), g2_gen) == Bn254::pairing(rhs.into_affine(), rhs_g2)
}

/// Results of verifying all 13 opening proofs.
pub struct OpeningVerification {
    /// Individual pairing checks: 10 polynomials at zeta.
    pub at_zeta:       [bool; 10],
    /// Individual pairing checks: 3 polynomials at w*zeta.
    pub at_omega_zeta: [bool; 3],
    /// Batched 2-pairing check for all 10 openings at zeta.
    pub batch_zeta:       bool,
    /// Batched 2-pairing check for all 3 openings at w*zeta.
    pub batch_omega_zeta: bool,
}

impl OpeningVerification {
    pub fn all_pass(&self) -> bool {
        self.at_zeta.iter().all(|&b| b)
            && self.at_omega_zeta.iter().all(|&b| b)
            && self.batch_zeta
            && self.batch_omega_zeta
    }
}

/// Verify all 13 opening proofs (individual + batched).
pub fn verify_all_openings(
    setup:  &KZGSetup,
    wcomms: &Commitments,
    qcomms: &QuotientCommitments,
    proofs: &OpeningProofs,
    fs:     &FiatShamirProof,
    domain: &GeneralEvaluationDomain<F>,
) -> OpeningVerification {
    let zeta       = fs.zeta;
    let omega_zeta = domain.group_gen() * zeta;
    let r          = fs.alpha; // pseudorandom batching scalar from FS transcript

    // Arrays for batched calls at zeta
    let comms_z = [
        wcomms.c_bid, wcomms.c_ask, wcomms.c_acc_b, wcomms.c_acc_a, wcomms.c_min,
        qcomms.c_q_acc_a_init, qcomms.c_q_acc_a_rec,
        qcomms.c_q_acc_b_init, qcomms.c_q_acc_b_rec, qcomms.c_q_kl,
    ];
    let pis_z = [
        proofs.pi_bid, proofs.pi_ask, proofs.pi_acc_b, proofs.pi_acc_a, proofs.pi_min,
        proofs.pi_q_acc_a_init, proofs.pi_q_acc_a_rec,
        proofs.pi_q_acc_b_init, proofs.pi_q_acc_b_rec, proofs.pi_q_kl,
    ];
    let evals_z = [
        fs.bid_at_zeta, fs.ask_at_zeta, fs.acc_b_at_zeta, fs.acc_a_at_zeta, fs.min_at_zeta,
        fs.q_acc_a_init_at_zeta, fs.q_acc_a_rec_at_zeta,
        fs.q_acc_b_init_at_zeta, fs.q_acc_b_rec_at_zeta, fs.q_kl_at_zeta,
    ];

    // Arrays for batched calls at w*zeta
    let comms_wz  = [wcomms.c_acc_a,          wcomms.c_ask,        wcomms.c_acc_b];
    let pis_wz    = [proofs.pi_acc_a_shift,   proofs.pi_ask_shift,  proofs.pi_acc_b_shift];
    let evals_wz  = [fs.acc_a_at_omega_zeta, fs.ask_at_omega_zeta, fs.acc_b_at_omega_zeta];

    // Individual checks at zeta
    let mut at_zeta = [false; 10];
    for i in 0..10 {
        at_zeta[i] = verify_opening(setup, comms_z[i], pis_z[i], zeta, evals_z[i]);
    }

    // Individual checks at w*zeta
    let mut at_omega_zeta = [false; 3];
    for i in 0..3 {
        at_omega_zeta[i] = verify_opening(setup, comms_wz[i], pis_wz[i], omega_zeta, evals_wz[i]);
    }

    // Batched checks
    let batch_zeta = verify_openings_batched(
        setup, &comms_z, &pis_z, &evals_z, zeta, r,
    );
    let batch_omega_zeta = verify_openings_batched(
        setup, &comms_wz, &pis_wz, &evals_wz, omega_zeta, r,
    );

    OpeningVerification { at_zeta, at_omega_zeta, batch_zeta, batch_omega_zeta }
}

// ===========================================================================
// #5  Full Pipeline (used by benchmarks)
// ===========================================================================

pub struct PipelineResult {
    pub witness_commitments:  Commitments,
    pub quotient_commitments: QuotientCommitments,
    pub constraint_result:    ConstraintResult,
    pub quotient_check:       QuotientCheck,
    pub fs_proof:             FiatShamirProof,
    pub opening_proofs:       OpeningProofs,
    pub opening_verification: OpeningVerification,
}

/// Run all layers end-to-end with a given toxic waste `tau`.
pub fn full_pipeline(tau: F) -> PipelineResult {
    let (bids, asks, bid_depth, ask_depth, min_arr) = compute_all_arrays();
    let domain  = build_domain();
    let polys   = build_all_polys(&bids, &asks, &bid_depth, &ask_depth, &min_arr, &domain);
    let setup   = KZGSetup::new(DOMAIN_SIZE - 1, tau);
    let wcomms  = commit_all(&setup, &polys);
    let (quots, q_check) = compute_quotients(&polys, &domain);
    let qcomms  = commit_quotients(&setup, &quots);
    let cresult = verify_all(&polys, &domain);
    let fs      = fiat_shamir_prove(&polys, &quots, &wcomms, &qcomms, &domain);
    let opr     = compute_all_opening_proofs(&setup, &polys, &quots, &fs, &domain);
    let opv     = verify_all_openings(&setup, &wcomms, &qcomms, &opr, &fs, &domain);
    PipelineResult {
        witness_commitments:  wcomms,
        quotient_commitments: qcomms,
        constraint_result:    cresult,
        quotient_check:       q_check,
        fs_proof:             fs,
        opening_proofs:       opr,
        opening_verification: opv,
    }
}

/// Generate a random field element for use as tau.
pub fn random_tau<R: Rng>(rng: &mut R) -> F {
    F::rand(rng)
}

// ===========================================================================
// #3e  Layer 3e -- Fiat-Shamir Non-Interactive Proof
// ===========================================================================
//
// In the interactive model the verifier sends a random challenge zeta after
// seeing the witness commitments, and the prover evaluates every polynomial
// at zeta. Making the proof *non-interactive* (via the Fiat-Shamir heuristic)
// replaces the verifier's coin with a hash of the transcript so far:
//
//   zeta  = H( C_Bid || C_Ask || C_AccB || C_AccA || C_Min
//           || C_Q_AccA_init || ... || C_Q_KL )
//
// The prover then evaluates all ten polynomials at zeta, and a second
// challenge alpha = H( zeta || all evaluations ) is used to combine the five
// residues into a single batched check:
//
//   sum_i=_1^5 alpha^(i-1) * r_i(zeta) = 0
//
// where each residue is:
//   r_i(zeta) = V_i(zeta) - Q_i(zeta) * D_i(zeta)
//
// The divisors D_i(zeta) are evaluated in O(1):
//   D_1 = zeta - w^0        D_2 = Z_H(zeta)
//   D_3 = zeta - w^20       D_4 = Z_H(zeta)
//   D_5 = Z_H(zeta)
//
// A zero batched residue--computed purely from public field elements--proves
// (under the random oracle model) that all five vanishing constraints held
// over the whole domain H, without the verifier ever seeing the witness
// polynomial coefficients.

/// All evaluations and challenges that form the non-interactive proof.
pub struct FiatShamirProof {
    //Fiat-Shamir challenges
    /// zeta -- evaluation point derived from SHA-256 of all 10 KZG commitments.
    pub zeta:  F,
    /// alpha -- batching exponent derived from SHA-256 of (zeta || all evaluations).
    pub alpha: F,

    //Witness polynomial evaluations at zeta
    pub bid_at_zeta:   F,
    pub ask_at_zeta:   F,
    pub acc_b_at_zeta: F,
    pub acc_a_at_zeta: F,
    pub min_at_zeta:   F,

    //Shifted evaluations at wzeta (needed for recurrence constraints)
    pub acc_a_at_omega_zeta: F, // Acc_A(w*zeta)
    pub ask_at_omega_zeta:   F, // Ask(w*zeta)
    pub acc_b_at_omega_zeta: F, // Acc_B(w*zeta)

    //Quotient polynomial evaluations at zeta
    pub q_acc_a_init_at_zeta: F,
    pub q_acc_a_rec_at_zeta:  F,
    pub q_acc_b_init_at_zeta: F,
    pub q_acc_b_rec_at_zeta:  F,
    pub q_kl_at_zeta:         F,

    //Public values the verifier recomputes
    /// Z_H(zeta) = zeta^{32} - 1  (recomputed by verifier, no trust needed).
    pub zh_at_zeta: F,

    //Per-constraint residues r_i = V_i(zeta) - Q_i(zeta)*D_i(zeta)
    /// Five residues; all must be zero for the proof to pass.
    pub r: [F; 5],

    //Batched check sum alpha^i r_i
    pub batch_ok: bool,
}

//Internal helpers

/// Hash arbitrary bytes into an `Fr` field element via SHA-256, interpreting
/// the 32-byte digest as a little-endian integer reduced mod |Fr|.
fn hash_to_field(data: &[u8]) -> F {
    let digest = Sha256::digest(data);
    F::from_le_bytes_mod_order(&digest)
}

/// Compress a G1Affine point into 32 bytes (BN254 compressed encoding).
fn g1_to_bytes(pt: &G1Affine) -> Vec<u8> {
    let mut b = Vec::new();
    pt.serialize_compressed(&mut b).unwrap();
    b
}

/// Compress a field element into 32 bytes (little-endian).
fn fr_to_bytes(f: &F) -> Vec<u8> {
    let mut b = Vec::new();
    f.serialize_compressed(&mut b).unwrap();
    b
}

//Main function

/// Build the non-interactive Fiat-Shamir proof.
///
/// # Steps
///
/// 1. **Derive zeta**: SHA-256 of the 10 compressed G1 commitment bytes.
///    `zeta = H(C_1 || C_2 || ... || C_10)` as a field element.
///
/// 2. **Evaluate** all 5 witness polynomials at zeta and wzeta; all 5 quotient
///    polynomials at zeta; and Z_H at zeta.
///
/// 3. **Compute residues** r_i = V_i(zeta) - Q_i(zeta)*D_i(zeta) for i = 1...5.
///
/// 4. **Derive alpha**: SHA-256 of (zeta || all 14 evaluations) as a field element.
///
/// 5. **Batched check**: `sum_i alpha^(i-1) r_i = 0`.
pub fn fiat_shamir_prove(
    polys:  &Polynomials,
    quots:  &QuotientPolynomials,
    wcomms: &Commitments,
    qcomms: &QuotientCommitments,
    domain: &GeneralEvaluationDomain<F>,
) -> FiatShamirProof {
    //Step 1: derive zeta from transcript of all 10 commitments
    let mut transcript: Vec<u8> = Vec::new();
    for pt in [
        &wcomms.c_bid,   &wcomms.c_ask,
        &wcomms.c_acc_b, &wcomms.c_acc_a, &wcomms.c_min,
        &qcomms.c_q_acc_a_init, &qcomms.c_q_acc_a_rec,
        &qcomms.c_q_acc_b_init, &qcomms.c_q_acc_b_rec,
        &qcomms.c_q_kl,
    ] {
        transcript.extend_from_slice(&g1_to_bytes(pt));
    }
    let zeta = hash_to_field(&transcript);

    //Step 2: evaluate all polynomials
    let omega       = domain.group_gen();
    let omega_zeta  = omega * zeta;

    let bid_at_zeta   = polys.bid.evaluate(&zeta);
    let ask_at_zeta   = polys.ask.evaluate(&zeta);
    let acc_b_at_zeta = polys.acc_b.evaluate(&zeta);
    let acc_a_at_zeta = polys.acc_a.evaluate(&zeta);
    let min_at_zeta   = polys.min.evaluate(&zeta);

    let acc_a_at_omega_zeta = polys.acc_a.evaluate(&omega_zeta);
    let ask_at_omega_zeta   = polys.ask.evaluate(&omega_zeta);
    let acc_b_at_omega_zeta = polys.acc_b.evaluate(&omega_zeta);

    let q_acc_a_init_at_zeta = quots.q_acc_a_init.evaluate(&zeta);
    let q_acc_a_rec_at_zeta  = quots.q_acc_a_rec.evaluate(&zeta);
    let q_acc_b_init_at_zeta = quots.q_acc_b_init.evaluate(&zeta);
    let q_acc_b_rec_at_zeta  = quots.q_acc_b_rec.evaluate(&zeta);
    let q_kl_at_zeta         = quots.q_kl.evaluate(&zeta);

    // Z_H(zeta) = zeta^{DOMAIN_SIZE} - 1  (O(log n) square-and-multiply)
    let zh_at_zeta = zeta.pow([DOMAIN_SIZE as u64]) - F::one();

    // Domain points needed for the linear divisors
    let elems: Vec<F>   = domain.elements().collect();
    let omega_0         = elems[0];              // w^0
    let omega_last_data = elems[N - 1];          // w^20
    let omega_last_dom  = elems[DOMAIN_SIZE - 1];// w^31

    //Step 3: compute per-constraint residues r_i = V_i(zeta) - Q_i(zeta)*D_i(zeta)

    // r_1  V_AccA_init:  (acc_a - ask)(zeta) - Q_1(zeta)*(zeta - w^0)
    let r1 = (acc_a_at_zeta - ask_at_zeta)
           - q_acc_a_init_at_zeta * (zeta - omega_0);

    // r_2  V_AccA_rec:  (zeta-w^20)*(acc_a(wzeta) - acc_a(zeta) - ask(wzeta)) - Q_2(zeta)*Z_H(zeta)
    let v2_zeta = (zeta - omega_last_data)
        * (acc_a_at_omega_zeta - acc_a_at_zeta - ask_at_omega_zeta);
    let r2 = v2_zeta - q_acc_a_rec_at_zeta * zh_at_zeta;

    // r_3  V_AccB_init:  (acc_b - bid)(zeta) - Q_3(zeta)*(zeta - w^20)
    let r3 = (acc_b_at_zeta - bid_at_zeta)
           - q_acc_b_init_at_zeta * (zeta - omega_last_data);

    // r_4  V_AccB_rec:  (zeta-w^31)*(acc_b(zeta) - acc_b(wzeta) - bid(zeta)) - Q_4(zeta)*Z_H(zeta)
    let v4_zeta = (zeta - omega_last_dom)
        * (acc_b_at_zeta - acc_b_at_omega_zeta - bid_at_zeta);
    let r4 = v4_zeta - q_acc_b_rec_at_zeta * zh_at_zeta;

    // r_5  V_KL:  (acc_a(zeta)-min(zeta))*(acc_b(zeta)-min(zeta)) - Q_5(zeta)*Z_H(zeta)
    let v5_zeta = (acc_a_at_zeta - min_at_zeta) * (acc_b_at_zeta - min_at_zeta);
    let r5 = v5_zeta - q_kl_at_zeta * zh_at_zeta;

    let r = [r1, r2, r3, r4, r5];

    //Step 4: derive alpha from transcript + zeta + all evaluations
    let mut alpha_input = transcript.clone();
    alpha_input.extend_from_slice(&fr_to_bytes(&zeta));
    for val in &[
        bid_at_zeta, ask_at_zeta, acc_b_at_zeta, acc_a_at_zeta, min_at_zeta,
        acc_a_at_omega_zeta, ask_at_omega_zeta, acc_b_at_omega_zeta,
        q_acc_a_init_at_zeta, q_acc_a_rec_at_zeta,
        q_acc_b_init_at_zeta, q_acc_b_rec_at_zeta, q_kl_at_zeta,
    ] {
        alpha_input.extend_from_slice(&fr_to_bytes(val));
    }
    let alpha = hash_to_field(&alpha_input);

    //Step 5: batched check sum alpha^(i-1) r_i = 0
    let mut alpha_pow = F::one();
    let mut batched   = F::zero();
    for ri in &r {
        batched  += alpha_pow * ri;
        alpha_pow *= alpha;
    }
    let batch_ok = batched.is_zero();

    FiatShamirProof {
        zeta,
        alpha,
        bid_at_zeta,
        ask_at_zeta,
        acc_b_at_zeta,
        acc_a_at_zeta,
        min_at_zeta,
        acc_a_at_omega_zeta,
        ask_at_omega_zeta,
        acc_b_at_omega_zeta,
        q_acc_a_init_at_zeta,
        q_acc_a_rec_at_zeta,
        q_acc_b_init_at_zeta,
        q_acc_b_rec_at_zeta,
        q_kl_at_zeta,
        zh_at_zeta,
        r,
        batch_ok,
    }
}
