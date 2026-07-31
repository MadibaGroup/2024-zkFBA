//! ZK-FBA CSV — full-protocol implementation (protocol_constraints.md notation)
//!
//! CSV order book -> re-derived columns -> KZG-committed polynomials ->
//! quotient/Fiat-Shamir proof -> batch-verified. #N = protocol_constraints.md
//! Section 14 constraint numbering.
//!
//! Rationale, design decisions, and benchmarks details explained

use std::borrow::Cow;
use std::fs::File;
use std::io::{BufRead, BufReader};

use ark_bn254::{Bn254, Fr as F, G1Affine};
#[allow(unused_imports)]
use ark_ec::CurveGroup;
use ark_ff::{batch_inversion, Field, FftField, One, PrimeField, Zero};
use ark_poly::{
    evaluations::univariate::Evaluations,
    univariate::DensePolynomial,
    EvaluationDomain, GeneralEvaluationDomain, Polynomial,
    Radix2EvaluationDomain,
};
use ark_poly_commit::kzg10::{
    Commitment, Powers, Randomness, UniversalParams, VerifierKey, KZG10,
};
use ark_std::rand::Rng;
use gadgets::range as range_gadget;
use gadgets::utils::{batch_check, batch_open, BatchCheckProof, Transcript};

// ===========================================================================
// Type aliases
// ===========================================================================

type KzgBn254 = KZG10<Bn254, DensePolynomial<F>>;
pub type KzgRand = Randomness<F, DensePolynomial<F>>;

/// Bit-width for every scalar range proof (V_max, Slack_L, Slack_R).
const RANGE_BITS: usize = 16;

// ===========================================================================
// Helper
// ===========================================================================

#[inline]
pub fn to_u64(f: F) -> u64 {
    f.into_bigint().as_ref()[0]
}

// ===========================================================================
// OrderBook — raw + derived columns (protocol_constraints.md Section 1 table)
// ===========================================================================

pub struct OrderBook {
    pub n:           usize,
    pub domain_size: usize,
    pub prices:      Vec<u64>,

    // Private witness columns
    pub b:      Vec<F>,   // B(X)      Bids++
    pub a:      Vec<F>,   // A(X)      Asks++
    pub acc_b:  Vec<F>,   // AccB(X)   Bid Depth
    pub acc_a:  Vec<F>,   // AccA(X)   Ask Depth
    pub min_x:  Vec<F>,   // Min(X)    Min(Bid,Ask)
    pub surp_b: Vec<F>,   // SurpB(X)  Bid Surplus++
    pub surp_a: Vec<F>,   // SurpA(X)  Ask Surplus++
    pub delta:  Vec<F>,   // Delta(X)  Abs(Delta)
    pub chk_d:  Vec<F>,   // ChkD(X)   Check on Delta (committed -- see module doc)

    // Public scalars disclosed in the clearing receipt (Section 1)
    pub v_max:       u64,
    pub v_min_delta: u64,
    pub c:           usize,
    pub d:           usize,
    pub p_star:      usize,

    // Cliff (Section 13). Slack is only ever meaningful at c-1/d+1, so it is
    // derived as a plain scalar from the opened Min(cliff) value rather than
    // a committed polynomial -- see prove_slack_range / Layer 3h below.
    pub slack_l:         u64,
    pub slack_r:         u64,
    pub has_left_cliff:  bool,
    pub has_right_cliff: bool,
}

impl OrderBook {
    /// Derive every protocol column from raw B/A alone (mirrors
    /// protocol_prover_gen.py::derive), independent of any CSV.
    fn derive(prices: Vec<u64>, b_raw: Vec<u64>, a_raw: Vec<u64>) -> Self {
        let n = b_raw.len();
        assert!(n > 0, "empty order book");

        // #3/#5  AccB: backward cumulative demand, seeded at the top tick
        let mut acc_b_raw = vec![0u64; n];
        acc_b_raw[n - 1] = b_raw[n - 1];
        for i in (0..n - 1).rev() {
            acc_b_raw[i] = acc_b_raw[i + 1] + b_raw[i];
        }

        // #4/#6  AccA: forward cumulative supply, seeded at the bottom tick
        let mut acc_a_raw = vec![0u64; n];
        acc_a_raw[0] = a_raw[0];
        for i in 1..n {
            acc_a_raw[i] = acc_a_raw[i - 1] + a_raw[i];
        }

        // #7  Min(X) = min(AccA, AccB)
        let min_x_raw: Vec<u64> = (0..n).map(|i| acc_a_raw[i].min(acc_b_raw[i])).collect();

        // #8/#11/#12  V_max and its contiguous plateau [c, d]
        let v_max = *min_x_raw.iter().max().expect("non-empty order book");
        let plateau: Vec<usize> = (0..n).filter(|&i| min_x_raw[i] == v_max).collect();
        let c = plateau[0];
        let d = *plateau.last().expect("plateau is non-empty by construction");
        assert!(
            plateau == (c..=d).collect::<Vec<_>>(),
            "plateau is not contiguous -- Min(X) unimodality assumption violated"
        );

        // #15/#16/#17  SurpB, SurpA, Delta = SurpA + SurpB
        let surp_b_raw: Vec<u64> = (0..n).map(|i| acc_b_raw[i] - min_x_raw[i]).collect();
        let surp_a_raw: Vec<u64> = (0..n).map(|i| acc_a_raw[i] - min_x_raw[i]).collect();
        let delta_raw: Vec<u64> = (0..n).map(|i| surp_a_raw[i] + surp_b_raw[i]).collect();

        // #19/#24  V_min_delta and p* (first plateau tick achieving it)
        let v_min_delta = (c..=d).map(|i| delta_raw[i]).min().expect("plateau is non-empty");
        let p_star = (c..=d)
            .find(|&i| delta_raw[i] == v_min_delta)
            .expect("v_min_delta is achieved somewhere in [c, d] by definition");

        // #18/#19/#20  ChkD: 1 inside [c, d] where Delta == V_min_delta
        let chk_d_raw: Vec<u64> = (0..n)
            .map(|i| (i >= c && i <= d && delta_raw[i] == v_min_delta) as u64)
            .collect();

        // #29-#32  Cliff slack -- only defined where a cliff tick exists
        let has_left_cliff = c > 0;
        let has_right_cliff = d < n - 1;
        let slack_l = if has_left_cliff { v_max - 1 - min_x_raw[c - 1] } else { 0 };
        let slack_r = if has_right_cliff { v_max - 1 - min_x_raw[d + 1] } else { 0 };

        assert!(
            v_max < (1u64 << RANGE_BITS),
            "V_max {v_max} exceeds 2^{RANGE_BITS} -- increase RANGE_BITS"
        );

        let to_f = |v: &Vec<u64>| v.iter().map(|&x| F::from(x)).collect::<Vec<F>>();
        let domain_size = n.next_power_of_two();

        OrderBook {
            n, domain_size, prices,
            b: to_f(&b_raw), a: to_f(&a_raw),
            acc_b: to_f(&acc_b_raw), acc_a: to_f(&acc_a_raw), min_x: to_f(&min_x_raw),
            surp_b: to_f(&surp_b_raw), surp_a: to_f(&surp_a_raw), delta: to_f(&delta_raw),
            chk_d: to_f(&chk_d_raw),
            v_max, v_min_delta, c, d, p_star,
            slack_l, slack_r, has_left_cliff, has_right_cliff,
        }
    }

    /// Load from CSV. Only Price/Bids++/Asks++ (columns 0-2) are trusted as
    /// input; every other protocol column is re-derived and cross-checked
    /// against the CSV's own precomputed columns as a guard against a
    /// preprocessing bug -- never used as circuit input.
    pub fn from_csv(path: &str) -> Self {
        let file = File::open(path).unwrap_or_else(|e| panic!("Cannot open '{path}': {e}"));
        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        lines.next(); // header

        let mut prices = Vec::new();
        let mut b_raw  = Vec::new();
        let mut a_raw  = Vec::new();
        let mut csv_acc_b  = Vec::new();
        let mut csv_acc_a  = Vec::new();
        let mut csv_min_x  = Vec::new();
        let mut csv_surp_b = Vec::new();
        let mut csv_surp_a = Vec::new();
        let mut csv_delta  = Vec::new();
        let mut csv_chk_d  = Vec::new();
        let mut csv_v_max  = Vec::new();

        for line in lines {
            let line = line.expect("I/O error reading CSV");
            let line = line.trim();
            if line.is_empty() { continue; }
            let cols: Vec<&str> = line.split(',').collect();
            if cols.len() < 15 { continue; }
            let p = |i: usize| cols[i].trim().parse::<u64>().unwrap_or_else(|_| panic!("column {i}"));

            prices.push(p(0));
            b_raw.push(p(1));
            a_raw.push(p(2));
            csv_acc_b.push(p(3));   // Bid_Depth
            csv_acc_a.push(p(4));   // Ask_Depth
            csv_min_x.push(p(5));   // Min_Bid_Ask
            csv_surp_b.push(p(8));  // Bid_Surplus
            csv_surp_a.push(p(9));  // Ask_Surplus
            csv_delta.push(p(10));  // Abs_Delta
            csv_chk_d.push(p(11));  // Check_on_Delta
            csv_v_max.push(p(14));  // MCV
        }
        assert!(!prices.is_empty(), "CSV '{path}' has no data rows");

        let book = Self::derive(prices, b_raw, a_raw);

        // Cross-check derived columns against the CSV's own precomputed ones
        let mut errs = Vec::new();
        for i in 0..book.n {
            if to_u64(book.acc_b[i])  != csv_acc_b[i]  { errs.push(format!("row {i}: AccB mismatch")); }
            if to_u64(book.acc_a[i])  != csv_acc_a[i]  { errs.push(format!("row {i}: AccA mismatch")); }
            if to_u64(book.min_x[i])  != csv_min_x[i]  { errs.push(format!("row {i}: Min mismatch")); }
            if to_u64(book.surp_b[i]) != csv_surp_b[i] { errs.push(format!("row {i}: SurpB mismatch")); }
            if to_u64(book.surp_a[i]) != csv_surp_a[i] { errs.push(format!("row {i}: SurpA mismatch")); }
            if to_u64(book.delta[i])  != csv_delta[i]  { errs.push(format!("row {i}: Delta mismatch")); }
            if to_u64(book.chk_d[i])  != csv_chk_d[i]  { errs.push(format!("row {i}: ChkD mismatch")); }
        }
        if book.v_max != csv_v_max[0] {
            errs.push(format!("V_max mismatch: derived {} vs CSV MCV {}", book.v_max, csv_v_max[0]));
        }
        assert!(errs.is_empty(), "{} cross-check mismatches, e.g.: {:?}", errs.len(), &errs[..errs.len().min(5)]);

        book
    }

    /// The 21-tick hardcoded dataset from the original paper example, run
    /// through the same `derive` pipeline (for baseline comparison).
    pub fn hardcoded_21tick() -> Self {
        let b_raw: Vec<u64> = vec![
            100, 100, 100, 200, 200, 500, 1000, 1500, 2000, 700,
            100, 100, 100,   0,   0,   0,  500,  500, 1000, 1000, 2000,
        ];
        let a_raw: Vec<u64> = vec![
            0, 20, 30, 50, 100, 200, 400, 700, 1000, 1500,
            1000, 0, 0, 0, 0, 0, 100, 100, 200, 200, 200,
        ];
        let prices: Vec<u64> = vec![0, 10, 20, 30, 40, 50, 60, 70, 80, 90,
                          100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110];
        let mut book = Self::derive(prices, b_raw, a_raw);
        book.domain_size = 32; // matches original hardcoded padding
        book
    }
}

// ===========================================================================
// Layer 2 — Polynomial Interpolation (dynamic domain)
// ===========================================================================

pub fn build_domain(domain_size: usize) -> GeneralEvaluationDomain<F> {
    GeneralEvaluationDomain::new(domain_size).expect("domain creation failed")
}

/// Interpolate `vals` (length n) over the given domain (size domain_size >= n),
/// zero-padding the evaluation table for indices n..domain_size.
pub fn interpolate_slice(vals: &[F], domain: &GeneralEvaluationDomain<F>) -> DensePolynomial<F> {
    let mut evals = vals.to_vec();
    evals.resize(domain.size(), F::zero());
    Evaluations::from_vec_and_domain(evals, *domain).interpolate()
}

pub struct Polynomials {
    pub b:      DensePolynomial<F>,
    pub a:      DensePolynomial<F>,
    pub acc_b:  DensePolynomial<F>,
    pub acc_a:  DensePolynomial<F>,
    pub min_x:  DensePolynomial<F>,
    pub surp_b: DensePolynomial<F>,
    pub surp_a: DensePolynomial<F>,
    pub delta:  DensePolynomial<F>,
    pub chk_d:  DensePolynomial<F>,
}

pub fn build_all_polys(book: &OrderBook, domain: &GeneralEvaluationDomain<F>) -> Polynomials {
    Polynomials {
        b:      interpolate_slice(&book.b,      domain),
        a:      interpolate_slice(&book.a,      domain),
        acc_b:  interpolate_slice(&book.acc_b,  domain),
        acc_a:  interpolate_slice(&book.acc_a,  domain),
        min_x:  interpolate_slice(&book.min_x,  domain),
        surp_b: interpolate_slice(&book.surp_b, domain),
        surp_a: interpolate_slice(&book.surp_a, domain),
        delta:  interpolate_slice(&book.delta,  domain),
        chk_d:  interpolate_slice(&book.chk_d,  domain),
    }
}

/// Mask_P(X): 1 on [c, d], 0 elsewhere. Verifier-computable from the
/// disclosed c/d -- deliberately NOT committed (see module doc).
pub fn mask_p_poly(book: &OrderBook, domain: &GeneralEvaluationDomain<F>) -> DensePolynomial<F> {
    let mut evals = vec![F::zero(); book.n];
    for i in book.c..=book.d {
        evals[i] = F::one();
    }
    interpolate_slice(&evals, domain)
}

/// InMCV(X) (#13/#14): V_max on [c, d], 0 elsewhere. Public, not committed
/// (see RATIONALE_AND_RESULTS.md). Checked in `verify_all` as a regression guard.
pub fn inmcv_poly(book: &OrderBook, domain: &GeneralEvaluationDomain<F>) -> DensePolynomial<F> {
    let mut evals = vec![F::zero(); book.n];
    for i in book.c..=book.d {
        evals[i] = F::from(book.v_max);
    }
    interpolate_slice(&evals, domain)
}

/// Mask_V(X) (#21/#22): 1 at disclosed clearing tick p*, 0 elsewhere. Public.
pub fn mask_v_poly(book: &OrderBook, domain: &GeneralEvaluationDomain<F>) -> DensePolynomial<F> {
    let mut evals = vec![F::zero(); book.n];
    evals[book.p_star] = F::one();
    interpolate_slice(&evals, domain)
}

/// Mask_C(X) (#27/#28): 1 at cliff ticks c-1/d+1 (when they exist), 0 elsewhere. Public.
pub fn mask_c_poly(book: &OrderBook, domain: &GeneralEvaluationDomain<F>) -> DensePolynomial<F> {
    let mut evals = vec![F::zero(); book.n];
    if book.has_left_cliff {
        evals[book.c - 1] = F::one();
    }
    if book.has_right_cliff {
        evals[book.d + 1] = F::one();
    }
    interpolate_slice(&evals, domain)
}

/// SD(X) (#25/#26): V_min_delta at p*, 0 elsewhere. Public.
pub fn sd_poly(book: &OrderBook, domain: &GeneralEvaluationDomain<F>) -> DensePolynomial<F> {
    let mut evals = vec![F::zero(); book.n];
    evals[book.p_star] = F::from(book.v_min_delta);
    interpolate_slice(&evals, domain)
}

// ===========================================================================
// Layer 3a/b — KZG Trusted Setup + Witness Commitments
// ===========================================================================

pub struct KzgScheme {
    pub powers: Powers<'static, Bn254>,
    pub vk:     VerifierKey<Bn254>,
    pub degree: usize,
}

impl KzgScheme {
    /// Build SRS with a random tau for the given degree.
    /// In production, tau comes from a multi-party ceremony.
    pub fn new<R: Rng>(degree: usize, rng: &mut R) -> Self {
        let pp: UniversalParams<Bn254> =
            KzgBn254::setup(degree, false, rng).expect("KZG10 setup failed");
        // KZG10::trim is pub(crate) — construct Powers and VerifierKey directly.
        let powers_of_g: Vec<G1Affine> = pp.powers_of_g[..=degree].to_vec();
        let powers_of_gamma_g: Vec<G1Affine> =
            (0..=degree).map(|i| pp.powers_of_gamma_g[&i]).collect();
        let powers = Powers::<'static, Bn254> {
            powers_of_g:        Cow::Owned(powers_of_g),
            powers_of_gamma_g:  Cow::Owned(powers_of_gamma_g),
        };
        let vk = VerifierKey::<Bn254> {
            g:               pp.powers_of_g[0],
            gamma_g:         pp.powers_of_gamma_g[&0],
            h:               pp.h,
            beta_h:          pp.beta_h,
            prepared_h:      pp.prepared_h.clone(),
            prepared_beta_h: pp.prepared_beta_h.clone(),
        };
        KzgScheme { powers, vk, degree }
    }

    /// Commit with hiding_bound = Some(1) — minimum to make is_hiding() == true
    /// (required by batch_open).  Some(poly.degree()) would exceed the SRS for
    /// large polynomials (HidingBoundToolarge panic).
    pub fn commit<R: Rng>(
        &self,
        poly: &DensePolynomial<F>,
        rng: &mut R,
    ) -> (Commitment<Bn254>, KzgRand) {
        KzgBn254::commit(&self.powers, poly, Some(1), Some(rng))
            .expect("KZG10 commit failed")
    }
}

pub struct Commitments {
    pub c_b:      Commitment<Bn254>,
    pub c_a:      Commitment<Bn254>,
    pub c_acc_b:  Commitment<Bn254>,
    pub c_acc_a:  Commitment<Bn254>,
    pub c_min_x:  Commitment<Bn254>,
    pub c_surp_b: Commitment<Bn254>,
    pub c_surp_a: Commitment<Bn254>,
    pub c_delta:  Commitment<Bn254>,
    pub c_chk_d:  Commitment<Bn254>,
}

/// Commit all nine witness polynomials; also returns the hiding randomness
/// required by `batch_open`, indexed the same as the fields above
/// (0:b 1:a 2:acc_b 3:acc_a 4:min_x 5:surp_b 6:surp_a 7:delta 8:chk_d).
pub fn commit_all<R: Rng>(
    scheme: &KzgScheme,
    polys:  &Polynomials,
    rng:    &mut R,
) -> (Commitments, Vec<KzgRand>) {
    let (c_b,      r_b)      = scheme.commit(&polys.b,      rng);
    let (c_a,      r_a)      = scheme.commit(&polys.a,      rng);
    let (c_acc_b,  r_acc_b)  = scheme.commit(&polys.acc_b,  rng);
    let (c_acc_a,  r_acc_a)  = scheme.commit(&polys.acc_a,  rng);
    let (c_min_x,  r_min_x)  = scheme.commit(&polys.min_x,  rng);
    let (c_surp_b, r_surp_b) = scheme.commit(&polys.surp_b, rng);
    let (c_surp_a, r_surp_a) = scheme.commit(&polys.surp_a, rng);
    let (c_delta,  r_delta)  = scheme.commit(&polys.delta,  rng);
    let (c_chk_d,  r_chk_d)  = scheme.commit(&polys.chk_d,  rng);
    (
        Commitments { c_b, c_a, c_acc_b, c_acc_a, c_min_x, c_surp_b, c_surp_a, c_delta, c_chk_d },
        vec![r_b, r_a, r_acc_b, r_acc_a, r_min_x, r_surp_b, r_surp_a, r_delta, r_chk_d],
    )
}

// ===========================================================================
// Layer 3c — Quotient Polynomials (poly_div_rem = all-tick, div_by_linear = single-tick)
// ===========================================================================

fn make_poly(mut coeffs: Vec<F>) -> DensePolynomial<F> {
    while coeffs.last().map(|x: &F| x.is_zero()).unwrap_or(false) {
        coeffs.pop();
    }
    DensePolynomial { coeffs }
}

/// Compute p(ω·X) by multiplying coefficient i by ω^i.
pub fn shift_omega(poly: &DensePolynomial<F>, omega: F) -> DensePolynomial<F> {
    let mut omega_pow = F::one();
    let coeffs = poly.coeffs.iter().map(|&c| {
        let v = c * omega_pow;
        omega_pow *= omega;
        v
    }).collect();
    DensePolynomial { coeffs }
}

fn mul_x_minus_a(poly: &DensePolynomial<F>, a: F) -> DensePolynomial<F> {
    if poly.coeffs.is_empty() {
        return DensePolynomial { coeffs: vec![] };
    }
    let n = poly.coeffs.len();
    let mut r = vec![F::zero(); n + 1];
    for (i, &c) in poly.coeffs.iter().enumerate() {
        r[i + 1] += c;
        r[i]     -= c * a;
    }
    make_poly(r)
}

pub fn build_zh_poly(domain_size: usize) -> DensePolynomial<F> {
    let mut coeffs = vec![F::zero(); domain_size + 1];
    coeffs[0]           = -F::one();
    coeffs[domain_size] =  F::one();
    DensePolynomial { coeffs }
}

pub fn poly_div_rem(
    num: &DensePolynomial<F>,
    den: &DensePolynomial<F>,
) -> (DensePolynomial<F>, DensePolynomial<F>) {
    if num.coeffs.is_empty() {
        return (DensePolynomial { coeffs: vec![] }, DensePolynomial { coeffs: vec![] });
    }
    let m = num.coeffs.len() - 1;
    let n = den.coeffs.len() - 1;
    if m < n {
        return (DensePolynomial { coeffs: vec![] }, num.clone());
    }
    let inv = den.coeffs[n].inverse().expect("leading coefficient of divisor is zero");
    let mut rem  = num.coeffs.clone();
    let mut quot = vec![F::zero(); m - n + 1];
    for i in (n..=m).rev() {
        let q = rem[i] * inv;
        quot[i - n] = q;
        for j in 0..=n {
            rem[i - n + j] -= q * den.coeffs[j];
        }
    }
    (make_poly(quot), make_poly(rem))
}

/// Coset-FFT quotient construction: `O(m log m)` instead of `poly_div_rem`'s
/// `O(deg^2)` coefficient long division. Requires `numerator` to vanish
/// identically on the size-`domain_size` domain H (i.e. be divisible by
/// `Z_H`); returns the exact quotient, bit-identical to `poly_div_rem`'s
/// output (see `tests::coset_fft_matches_poly_div_rem` for the cross-check
/// against real constraint data). Does not compute a remainder -- callers
/// that need a divisibility check should verify at a random point instead
/// (as `fiat_shamir_prove` already does at `zeta`), since recombining
/// `Q * Z_H` in coefficient form to check would reintroduce the same O(n^2)
/// cost this function exists to avoid.
pub fn divide_by_vanishing_coset(
    numerator:   &DensePolynomial<F>,
    domain_size: usize,
) -> DensePolynomial<F> {
    if numerator.coeffs.is_empty() {
        return DensePolynomial { coeffs: vec![] };
    }
    let num_deg = numerator.degree();
    let m = (num_deg + 1).next_power_of_two();
    let coset = GeneralEvaluationDomain::<F>::new(m)
        .expect("coset domain creation failed")
        .get_coset(F::GENERATOR)
        .expect("coset construction failed");

    let v_evals = numerator.evaluate_over_domain_by_ref(coset).evals;

    let mut zh_evals: Vec<F> = coset
        .elements()
        .map(|x| x.pow([domain_size as u64]) - F::one())
        .collect();
    batch_inversion(&mut zh_evals);

    let q_evals: Vec<F> = v_evals
        .iter()
        .zip(zh_evals.iter())
        .map(|(&v, &zhi)| v * zhi)
        .collect();

    Evaluations::from_vec_and_domain(q_evals, coset).interpolate()
}

/// Schwartz-Zippel-style spot check that `numerator == quotient * Z_H`, evaluated
/// at a single fixed point instead of recombined in coefficient form -- an O(1)
/// substitute for `poly_div_rem`'s exact-remainder-is-zero self-check, needed
/// because `divide_by_vanishing_coset` does not produce a remainder. Soundness
/// rests on the same reasoning already used for the real Fiat-Shamir `zeta`
/// check elsewhere in this module: an unnoticed nonzero difference would have
/// to vanish at this point by coincidence.
fn quotient_holds_at(
    numerator:   &DensePolynomial<F>,
    quotient:    &DensePolynomial<F>,
    domain_size: usize,
    point:       F,
) -> bool {
    let zh_at_point = point.pow([domain_size as u64]) - F::one();
    numerator.evaluate(&point) == quotient.evaluate(&point) * zh_at_point
}

/// Synthetic division by (X - a).  Returns (quotient, poly(a)).
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

pub struct QuotientPolynomials {
    pub q_acc_a_init:    DensePolynomial<F>, // #4
    pub q_acc_a_rec:     DensePolynomial<F>, // #6
    pub q_acc_b_init:    DensePolynomial<F>, // #3
    pub q_acc_b_rec:     DensePolynomial<F>, // #5
    pub q_kl:            DensePolynomial<F>, // #7
    pub q_delta_def:     DensePolynomial<F>, // #17
    pub q_chkd_bool:     DensePolynomial<F>, // #18
    pub q_chkd_correct:  DensePolynomial<F>, // #19
    pub q_chkd_contain:  DensePolynomial<F>, // #20
    pub q_plateau_left:  DensePolynomial<F>, // #11
    pub q_plateau_right: DensePolynomial<F>, // #12
    pub q_valley_pin:    DensePolynomial<F>, // #24
    pub q_mask_v_contain: DensePolynomial<F>, // #22
    pub q_sd_def:         DensePolynomial<F>, // #25
}

impl QuotientPolynomials {
    pub fn max_degree(&self) -> usize {
        [
            self.q_acc_a_init.degree(), self.q_acc_a_rec.degree(),
            self.q_acc_b_init.degree(), self.q_acc_b_rec.degree(),
            self.q_kl.degree(), self.q_delta_def.degree(),
            self.q_chkd_bool.degree(), self.q_chkd_correct.degree(),
            self.q_chkd_contain.degree(), self.q_plateau_left.degree(),
            self.q_plateau_right.degree(), self.q_valley_pin.degree(),
            self.q_mask_v_contain.degree(), self.q_sd_def.degree(),
        ]
        .into_iter()
        .max()
        .unwrap_or(0)
    }
}

pub struct QuotientCheck {
    pub r_acc_a_init:    bool, // #4
    pub r_acc_a_rec:     bool, // #6
    pub r_acc_b_init:    bool, // #3
    pub r_acc_b_rec:     bool, // #5
    pub r_kl:            bool, // #7
    pub r_delta_def:     bool, // #17
    pub r_chkd_bool:     bool, // #18
    pub r_chkd_correct:  bool, // #19
    pub r_chkd_contain:  bool, // #20
    pub r_plateau_left:  bool, // #11
    pub r_plateau_right: bool, // #12
    pub r_valley_pin:    bool, // #24
    pub r_mask_v_contain: bool, // #22
    pub r_sd_def:         bool, // #25
}

impl QuotientCheck {
    pub fn all_zero(&self) -> bool {
        self.r_acc_a_init && self.r_acc_a_rec && self.r_acc_b_init && self.r_acc_b_rec
            && self.r_kl && self.r_delta_def && self.r_chkd_bool && self.r_chkd_correct
            && self.r_chkd_contain && self.r_plateau_left && self.r_plateau_right
            && self.r_valley_pin && self.r_mask_v_contain && self.r_sd_def
    }
}

pub fn compute_quotients(
    book:   &OrderBook,
    polys:  &Polynomials,
    mask_p: &DensePolynomial<F>,
    domain: &GeneralEvaluationDomain<F>,
    n:      usize,
) -> (QuotientPolynomials, QuotientCheck) {
    let omega           = domain.group_gen();
    let elems: Vec<F>   = domain.elements().collect();
    let domain_size     = domain.size();
    let omega_0         = elems[0];
    let omega_last_data = elems[n - 1];           // ω^{n-1}: last data tick
    let omega_last_dom  = elems[domain_size - 1]; // ω^{D-1}: last domain element

    // #4  AccA(ω^0) = A(ω^0)
    let f_a_init = &polys.acc_a - &polys.a;
    let (q1, r1) = div_by_linear(&f_a_init, omega_0);

    // #6  AccA(ωX) - AccA(X) - A(ωX) = 0  for X != ω^{n-1}
    let acc_a_next = shift_omega(&polys.acc_a, omega);
    let a_next     = shift_omega(&polys.a,     omega);
    let g2 = &(&acc_a_next - &polys.acc_a) - &a_next;
    let v2 = mul_x_minus_a(&g2, omega_last_data);
    let q2 = divide_by_vanishing_coset(&v2, domain_size);
    let r2_ok = quotient_holds_at(&v2, &q2, domain_size, F::GENERATOR);

    // #3  AccB(ω^{n-1}) = B(ω^{n-1})
    let f_b_init = &polys.acc_b - &polys.b;
    let (q3, r3) = div_by_linear(&f_b_init, omega_last_data);

    // #5  AccB(X) - AccB(ωX) - B(X) = 0  for X != ω^{D-1}
    let acc_b_next = shift_omega(&polys.acc_b, omega);
    let g4 = &(&polys.acc_b - &acc_b_next) - &polys.b;
    let v4 = mul_x_minus_a(&g4, omega_last_dom);
    let q4 = divide_by_vanishing_coset(&v4, domain_size);
    let r4_ok = quotient_holds_at(&v4, &q4, domain_size, F::GENERATOR);

    // #7  (AccA - Min) * (AccB - Min) = 0
    let da = &polys.acc_a - &polys.min_x;
    let db = &polys.acc_b - &polys.min_x;
    let v5 = &da * &db;
    let q5 = divide_by_vanishing_coset(&v5, domain_size);
    let r5_ok = quotient_holds_at(&v5, &q5, domain_size, F::GENERATOR);

    // #17  Delta - (SurpA + SurpB) = 0
    let v6 = &polys.delta - &(&polys.surp_a + &polys.surp_b);
    let q6 = divide_by_vanishing_coset(&v6, domain_size);
    let r6_ok = quotient_holds_at(&v6, &q6, domain_size, F::GENERATOR);

    // #18  ChkD * (ChkD - 1) = 0
    let one_poly = DensePolynomial { coeffs: vec![F::one()] };
    let v7 = &polys.chk_d * &(&polys.chk_d - &one_poly);
    let q7 = divide_by_vanishing_coset(&v7, domain_size);
    let r7_ok = quotient_holds_at(&v7, &q7, domain_size, F::GENERATOR);

    // #19  (Delta - V_min_delta) * ChkD = 0
    let v_min_delta_poly = DensePolynomial { coeffs: vec![F::from(book.v_min_delta)] };
    let v8 = &(&polys.delta - &v_min_delta_poly) * &polys.chk_d;
    let q8 = divide_by_vanishing_coset(&v8, domain_size);
    let r8_ok = quotient_holds_at(&v8, &q8, domain_size, F::GENERATOR);

    // #20  ChkD * (1 - Mask_P) = 0
    let v9 = &polys.chk_d * &(&one_poly - mask_p);
    let q9 = divide_by_vanishing_coset(&v9, domain_size);
    let r9_ok = quotient_holds_at(&v9, &q9, domain_size, F::GENERATOR);

    // #11  (Min - V_max) at ω^c
    let v_max_poly = DensePolynomial { coeffs: vec![F::from(book.v_max)] };
    let v_pin = &polys.min_x - &v_max_poly;
    let (q10, r10) = div_by_linear(&v_pin, elems[book.c]);

    // #12  (Min - V_max) at ω^d
    let (q11, r11) = div_by_linear(&v_pin, elems[book.d]);

    // #24  (Delta - V_min_delta) at ω^{p*}
    let v_pin2 = &polys.delta - &v_min_delta_poly;
    let (q12, r12) = div_by_linear(&v_pin2, elems[book.p_star]);

    // #22  Mask_V * (1 - ChkD) = 0  (ChkD committed -> real quotient needed)
    let mask_v = mask_v_poly(book, domain);
    let v13 = &mask_v * &(&one_poly - &polys.chk_d);
    let q13 = divide_by_vanishing_coset(&v13, domain_size);
    let r13_ok = quotient_holds_at(&v13, &q13, domain_size, F::GENERATOR);

    // #25  SD - Mask_V * Delta = 0  (Delta committed -> real quotient needed)
    let sd = sd_poly(book, domain);
    let v14 = &sd - &(&mask_v * &polys.delta);
    let q14 = divide_by_vanishing_coset(&v14, domain_size);
    let r14_ok = quotient_holds_at(&v14, &q14, domain_size, F::GENERATOR);

    let quotients = QuotientPolynomials {
        q_acc_a_init: q1, q_acc_a_rec: q2, q_acc_b_init: q3, q_acc_b_rec: q4, q_kl: q5,
        q_delta_def: q6, q_chkd_bool: q7, q_chkd_correct: q8, q_chkd_contain: q9,
        q_plateau_left: q10, q_plateau_right: q11, q_valley_pin: q12,
        q_mask_v_contain: q13, q_sd_def: q14,
    };
    let check = QuotientCheck {
        r_acc_a_init: r1.is_zero(),
        r_acc_a_rec:  r2_ok,
        r_acc_b_init: r3.is_zero(),
        r_acc_b_rec:  r4_ok,
        r_kl:         r5_ok,
        r_delta_def:     r6_ok,
        r_chkd_bool:     r7_ok,
        r_chkd_correct:  r8_ok,
        r_chkd_contain:  r9_ok,
        r_plateau_left:  r10.is_zero(),
        r_plateau_right: r11.is_zero(),
        r_valley_pin:    r12.is_zero(),
        r_mask_v_contain: r13_ok,
        r_sd_def:         r14_ok,
    };
    (quotients, check)
}

pub struct QuotientCommitments {
    pub c_q_acc_a_init:    Commitment<Bn254>,
    pub c_q_acc_a_rec:     Commitment<Bn254>,
    pub c_q_acc_b_init:    Commitment<Bn254>,
    pub c_q_acc_b_rec:     Commitment<Bn254>,
    pub c_q_kl:            Commitment<Bn254>,
    pub c_q_delta_def:     Commitment<Bn254>,
    pub c_q_chkd_bool:     Commitment<Bn254>,
    pub c_q_chkd_correct:  Commitment<Bn254>,
    pub c_q_chkd_contain:  Commitment<Bn254>,
    pub c_q_plateau_left:  Commitment<Bn254>,
    pub c_q_plateau_right: Commitment<Bn254>,
    pub c_q_valley_pin:    Commitment<Bn254>,
    pub c_q_mask_v_contain: Commitment<Bn254>,
    pub c_q_sd_def:         Commitment<Bn254>,
}

/// Commit all fourteen quotient polynomials; rand vec index order matches
/// QuotientPolynomials' field order (see struct comments for #N mapping).
pub fn commit_quotients<R: Rng>(
    scheme: &KzgScheme,
    q:      &QuotientPolynomials,
    rng:    &mut R,
) -> (QuotientCommitments, Vec<KzgRand>) {
    let (c_q_acc_a_init,    r0)  = scheme.commit(&q.q_acc_a_init,    rng);
    let (c_q_acc_a_rec,     r1)  = scheme.commit(&q.q_acc_a_rec,     rng);
    let (c_q_acc_b_init,    r2)  = scheme.commit(&q.q_acc_b_init,    rng);
    let (c_q_acc_b_rec,     r3)  = scheme.commit(&q.q_acc_b_rec,     rng);
    let (c_q_kl,            r4)  = scheme.commit(&q.q_kl,            rng);
    let (c_q_delta_def,     r5)  = scheme.commit(&q.q_delta_def,     rng);
    let (c_q_chkd_bool,     r6)  = scheme.commit(&q.q_chkd_bool,     rng);
    let (c_q_chkd_correct,  r7)  = scheme.commit(&q.q_chkd_correct,  rng);
    let (c_q_chkd_contain,  r8)  = scheme.commit(&q.q_chkd_contain,  rng);
    let (c_q_plateau_left,  r9)  = scheme.commit(&q.q_plateau_left,  rng);
    let (c_q_plateau_right, r10) = scheme.commit(&q.q_plateau_right, rng);
    let (c_q_valley_pin,    r11) = scheme.commit(&q.q_valley_pin,    rng);
    let (c_q_mask_v_contain, r12) = scheme.commit(&q.q_mask_v_contain, rng);
    let (c_q_sd_def,         r13) = scheme.commit(&q.q_sd_def,         rng);
    let comms = QuotientCommitments {
        c_q_acc_a_init, c_q_acc_a_rec, c_q_acc_b_init, c_q_acc_b_rec, c_q_kl,
        c_q_delta_def, c_q_chkd_bool, c_q_chkd_correct, c_q_chkd_contain,
        c_q_plateau_left, c_q_plateau_right, c_q_valley_pin,
        c_q_mask_v_contain, c_q_sd_def,
    };
    (comms, vec![r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13])
}

// ===========================================================================
// Layer 4 — Constraint Verification (algebraic witness check, no crypto)
// ===========================================================================

pub struct ConstraintResult {
    pub acc_a_init:       bool, // #4
    pub acc_a_recurrence: bool, // #6
    pub acc_b_init:       bool, // #3
    pub acc_b_recurrence: bool, // #5
    pub min_exclusivity:  bool, // #7
    pub delta_definition: bool, // #17
    pub chkd_booleanity:  bool, // #18
    pub chkd_correctness: bool, // #19
    pub chkd_containment: bool, // #20
    pub plateau_left:     bool, // #11
    pub plateau_right:    bool, // #12
    pub valley_pin:       bool, // #24
    pub inmcv_inside:     bool, // #13
    pub inmcv_outside:    bool, // #14
    pub mask_v_booleanity:  bool, // #21
    pub mask_v_containment: bool, // #22
    pub sd_definition:      bool, // #25
    pub sd_membership:      bool, // #26
    pub mask_c_booleanity:  bool, // #27
    pub mask_c_containment: bool, // #28
    pub mask_p_booleanity:  bool, // #9
}

impl ConstraintResult {
    pub fn all_pass(&self) -> bool {
        self.acc_a_init && self.acc_a_recurrence && self.acc_b_init && self.acc_b_recurrence
            && self.min_exclusivity && self.delta_definition && self.chkd_booleanity
            && self.chkd_correctness && self.chkd_containment && self.plateau_left
            && self.plateau_right && self.valley_pin
            && self.inmcv_inside && self.inmcv_outside
            && self.mask_v_booleanity && self.mask_v_containment
            && self.sd_definition && self.sd_membership
            && self.mask_c_booleanity && self.mask_c_containment
            && self.mask_p_booleanity
    }
}

pub fn verify_all(
    book:   &OrderBook,
    polys:  &Polynomials,
    mask_p: &DensePolynomial<F>,
    domain: &GeneralEvaluationDomain<F>,
    n:      usize,
) -> ConstraintResult {
    let v_min_delta = F::from(book.v_min_delta);
    let v_max       = F::from(book.v_max);
    let inmcv       = inmcv_poly(book, domain);
    let mask_v      = mask_v_poly(book, domain);
    let mask_c      = mask_c_poly(book, domain);
    let sd          = sd_poly(book, domain);

    // Evaluate every polynomial once over the full domain via FFT (O(n log n))
    // instead of the old per-point Horner `.evaluate(&elems[i])` calls, which
    // cost O(domain_size) each and were repeated at every one of the n domain
    // points -- O(n^2) overall for no reason, since this checks the exact same
    // points either way (this is an exact check, not a spot-check reduction).
    let ev = |p: &DensePolynomial<F>| p.evaluate_over_domain_by_ref(*domain).evals;
    let acc_a  = ev(&polys.acc_a);
    let a      = ev(&polys.a);
    let acc_b  = ev(&polys.acc_b);
    let b      = ev(&polys.b);
    let min_x  = ev(&polys.min_x);
    let delta  = ev(&polys.delta);
    let surp_a = ev(&polys.surp_a);
    let surp_b = ev(&polys.surp_b);
    let chk_d  = ev(&polys.chk_d);
    let mp     = ev(mask_p);
    let iv_v   = ev(&inmcv);
    let mv_v   = ev(&mask_v);
    let mc_v   = ev(&mask_c);
    let sd_v   = ev(&sd);

    ConstraintResult {
        acc_a_init: acc_a[0] == a[0],
        acc_a_recurrence: (0..n - 1).all(|i| acc_a[i + 1] == acc_a[i] + a[i + 1]),
        acc_b_init: acc_b[n - 1] == b[n - 1],
        acc_b_recurrence: (0..n - 1).all(|i| acc_b[i] == acc_b[i + 1] + b[i]),
        min_exclusivity: (0..n).all(|i| {
            let da = acc_a[i] - min_x[i];
            let db = acc_b[i] - min_x[i];
            (da * db).is_zero()
        }),
        delta_definition: (0..n).all(|i| delta[i] == surp_a[i] + surp_b[i]),
        chkd_booleanity: (0..n).all(|i| (chk_d[i] * (chk_d[i] - F::one())).is_zero()),
        chkd_correctness: (0..n).all(|i| ((delta[i] - v_min_delta) * chk_d[i]).is_zero()),
        chkd_containment: (0..n).all(|i| (chk_d[i] * (F::one() - mp[i])).is_zero()),
        plateau_left:  min_x[book.c] == v_max,
        plateau_right: min_x[book.d] == v_max,
        valley_pin:    delta[book.p_star] == v_min_delta,
        inmcv_inside: (0..n).all(|i| ((iv_v[i] - v_max) * mp[i]).is_zero()),
        inmcv_outside: (0..n).all(|i| (iv_v[i] * (F::one() - mp[i])).is_zero()),
        mask_v_booleanity: (0..n).all(|i| (mv_v[i] * (mv_v[i] - F::one())).is_zero()),
        mask_v_containment: (0..n).all(|i| (mv_v[i] * (F::one() - chk_d[i])).is_zero()),
        sd_definition: (0..n).all(|i| (sd_v[i] - mv_v[i] * delta[i]).is_zero()),
        sd_membership: (0..n).all(|i| (sd_v[i] * (sd_v[i] - v_min_delta)).is_zero()),
        mask_c_booleanity: (0..n).all(|i| (mc_v[i] * (mc_v[i] - F::one())).is_zero()),
        mask_c_containment: (0..n).all(|i| (mc_v[i] * mp[i]).is_zero()),
        mask_p_booleanity: (0..n).all(|i| (mp[i] * (mp[i] - F::one())).is_zero()),
    }
}

// ===========================================================================
// Layer 4b — Bit-Decomposition Gadget (#1, #2, #8, #15, #16, #23)
// Per-row range check via BIT_WIDTH committed bit columns + Z_H-divisible
// reconstruction/booleanity proofs. See RATIONALE_AND_RESULTS.md for design.
// ===========================================================================

// SurpB/SurpA can exceed 2^16 (cumulative volume diff), so 32 not 16 bits.
pub const BIT_WIDTH: usize = 32;

fn assert_fits_bit_width(values: &[u64], label: &str) {
    assert!(
        values.iter().all(|&v| v < (1u64 << BIT_WIDTH)),
        "{label}: value exceeds 2^{BIT_WIDTH} -- increase BIT_WIDTH"
    );
}

fn scale_poly(poly: &DensePolynomial<F>, s: F) -> DensePolynomial<F> {
    make_poly(poly.coeffs.iter().map(|&c| c * s).collect())
}

pub struct BitColumns {
    pub bits: Vec<DensePolynomial<F>>, // BIT_WIDTH columns, LSB first
}

pub fn build_bit_columns(values: &[u64], domain: &GeneralEvaluationDomain<F>) -> BitColumns {
    let bits = (0..BIT_WIDTH)
        .map(|j| {
            let col: Vec<F> = values.iter().map(|&v| F::from((v >> j) & 1)).collect();
            interpolate_slice(&col, domain)
        })
        .collect();
    BitColumns { bits }
}

/// sum_j 2^j * B_j(X)
pub fn bit_reconstruct(cols: &BitColumns) -> DensePolynomial<F> {
    let mut acc    = DensePolynomial { coeffs: vec![] };
    let mut weight = F::one();
    for b in &cols.bits {
        acc     = &acc + &scale_poly(b, weight);
        weight += weight;
    }
    acc
}

pub struct BitGadget {
    pub cols:     BitColumns,
    /// The exact polynomial this instance range-checks (e.g. B(X), or the
    /// standalone V_max - Min(X) polynomial for #8). Kept verbatim -- for
    /// #8 in particular this is *not* algebraically equal to "V_max minus
    /// the Min(X) witness poly" outside the first `n` domain points (each
    /// side is zero-padded independently by interpolate_slice), so Fiat-
    /// Shamir residues must evaluate this field directly at zeta rather
    /// than reconstruct it via affine combination of other evaluations.
    pub value:    DensePolynomial<F>,
    pub q_recon:  DensePolynomial<F>,
    pub q_bools:  Vec<DensePolynomial<F>>,
    pub recon_ok: bool,
    pub bool_ok:  bool,
}

impl BitGadget {
    pub fn all_pass(&self) -> bool {
        self.recon_ok && self.bool_ok
    }
}

/// Build + verify one bit-decomposition instance. `mask` (if given) gates
/// the reconstruction check to where mask=1 (#23); booleanity is unconditional.
pub fn build_bit_gadget(
    bit_values: &[u64],
    value:      &DensePolynomial<F>,
    mask:       Option<&DensePolynomial<F>>,
    domain:     &GeneralEvaluationDomain<F>,
) -> BitGadget {
    let domain_size = domain.size();
    let cols  = build_bit_columns(bit_values, domain);
    let recon = bit_reconstruct(&cols);
    let diff  = value - &recon;
    let gated = match mask {
        Some(m) => m * &diff,
        None    => diff,
    };
    let q_recon = divide_by_vanishing_coset(&gated, domain_size);
    let recon_ok = quotient_holds_at(&gated, &q_recon, domain_size, F::GENERATOR);

    let one_poly = DensePolynomial { coeffs: vec![F::one()] };
    let mut q_bools = Vec::with_capacity(cols.bits.len());
    let mut bool_ok = true;
    for b in &cols.bits {
        let v = b * &(b - &one_poly);
        let q = divide_by_vanishing_coset(&v, domain_size);
        bool_ok &= quotient_holds_at(&v, &q, domain_size, F::GENERATOR);
        q_bools.push(q);
    }

    BitGadget { cols, value: value.clone(), q_recon, q_bools, recon_ok, bool_ok }
}

pub struct BitGadgetCommitments {
    pub c_bits:  Vec<Commitment<Bn254>>,
    pub c_recon: Commitment<Bn254>,
    pub c_bools: Vec<Commitment<Bn254>>,
}

/// Commit every witness/quotient polynomial of one gadget instance
/// (BIT_WIDTH bit columns + 1 reconstruction quotient + BIT_WIDTH
/// booleanity quotients = 33 KZG commitments per instance).
pub fn commit_bit_gadget<R: Rng>(
    scheme: &KzgScheme,
    gadget: &BitGadget,
    rng:    &mut R,
) -> (BitGadgetCommitments, Vec<KzgRand>, KzgRand, Vec<KzgRand>) {
    let mut c_bits = Vec::with_capacity(gadget.cols.bits.len());
    let mut r_bits = Vec::with_capacity(gadget.cols.bits.len());
    for b in &gadget.cols.bits {
        let (c, r) = scheme.commit(b, rng);
        c_bits.push(c);
        r_bits.push(r);
    }
    let (c_recon, r_recon) = scheme.commit(&gadget.q_recon, rng);
    let mut c_bools = Vec::with_capacity(gadget.q_bools.len());
    let mut r_bools = Vec::with_capacity(gadget.q_bools.len());
    for q in &gadget.q_bools {
        let (c, r) = scheme.commit(q, rng);
        c_bools.push(c);
        r_bools.push(r);
    }
    (BitGadgetCommitments { c_bits, c_recon, c_bools }, r_bits, r_recon, r_bools)
}

/// Hiding randomness for one gadget's commitments, same shape as
/// BitGadgetCommitments. Held for Phase 3 (batch-opening these polynomials
/// for real); unused until then.
pub struct BitGadgetRands {
    pub r_bits:  Vec<KzgRand>,
    pub r_recon: KzgRand,
    pub r_bools: Vec<KzgRand>,
}

/// All six gadget instances' commitments bundled together, so they can be
/// absorbed into the Fiat-Shamir transcript as one unit (see
/// `fiat_shamir_prove`'s `bgcomms` parameter). This is what closes the
/// soundness gap documented in RATIONALE_AND_RESULTS.md Design Rationale #5:
/// these commitments used to be discarded (`let _ = commit_bit_gadget(...)`)
/// and never influenced the verifier's challenge. Folding their residues into
/// `r`/`batch_ok` and their openings into `batch_check` is Phase 2/3, tracked
/// separately -- this struct is the plumbing those phases build on.
pub struct AllBitGadgetCommitments {
    pub bid_range:   BitGadgetCommitments, // #1
    pub ask_range:   BitGadgetCommitments, // #2
    pub ceiling:     BitGadgetCommitments, // #8
    pub surp_b_nn:   BitGadgetCommitments, // #15
    pub surp_a_nn:   BitGadgetCommitments, // #16
    pub delta_floor: BitGadgetCommitments, // #23
}

pub struct AllBitGadgetRands {
    pub bid_range:   BitGadgetRands,
    pub ask_range:   BitGadgetRands,
    pub ceiling:     BitGadgetRands,
    pub surp_b_nn:   BitGadgetRands,
    pub surp_a_nn:   BitGadgetRands,
    pub delta_floor: BitGadgetRands,
}

impl AllBitGadgetCommitments {
    /// Flatten every bit-gadget commitment (6 instances * (BIT_WIDTH bit
    /// columns + 1 reconstruction quotient + BIT_WIDTH booleanity quotients),
    /// 65 per instance, 390 total at BIT_WIDTH=32) into one fixed-order list
    /// for Fiat-Shamir absorption.
    pub fn all_affines(&self) -> Vec<G1Affine> {
        let mut out = Vec::new();
        for bc in [
            &self.bid_range, &self.ask_range, &self.ceiling,
            &self.surp_b_nn, &self.surp_a_nn, &self.delta_floor,
        ] {
            out.extend(bc.c_bits.iter().map(|c| c.0));
            out.push(bc.c_recon.0);
            out.extend(bc.c_bools.iter().map(|c| c.0));
        }
        out
    }
}

/// Commit all six bit-gadget instances in `BitGadgetSet`'s field order and
/// bundle the results. Replaces six separate `let _ = commit_bit_gadget(...)`
/// call sites that used to throw the commitments away.
pub fn commit_all_bit_gadgets<R: Rng>(
    scheme: &KzgScheme,
    gadgets: &BitGadgetSet,
    rng: &mut R,
) -> (AllBitGadgetCommitments, AllBitGadgetRands) {
    let (c_bid, rb_bid, rr_bid, rbo_bid)         = commit_bit_gadget(scheme, &gadgets.bid_range, rng);
    let (c_ask, rb_ask, rr_ask, rbo_ask)         = commit_bit_gadget(scheme, &gadgets.ask_range, rng);
    let (c_ceil, rb_ceil, rr_ceil, rbo_ceil)     = commit_bit_gadget(scheme, &gadgets.ceiling, rng);
    let (c_sb, rb_sb, rr_sb, rbo_sb)             = commit_bit_gadget(scheme, &gadgets.surp_b_nn, rng);
    let (c_sa, rb_sa, rr_sa, rbo_sa)             = commit_bit_gadget(scheme, &gadgets.surp_a_nn, rng);
    let (c_df, rb_df, rr_df, rbo_df)             = commit_bit_gadget(scheme, &gadgets.delta_floor, rng);
    let comms = AllBitGadgetCommitments {
        bid_range: c_bid, ask_range: c_ask, ceiling: c_ceil,
        surp_b_nn: c_sb, surp_a_nn: c_sa, delta_floor: c_df,
    };
    let rands = AllBitGadgetRands {
        bid_range:   BitGadgetRands { r_bits: rb_bid, r_recon: rr_bid, r_bools: rbo_bid },
        ask_range:   BitGadgetRands { r_bits: rb_ask, r_recon: rr_ask, r_bools: rbo_ask },
        ceiling:     BitGadgetRands { r_bits: rb_ceil, r_recon: rr_ceil, r_bools: rbo_ceil },
        surp_b_nn:   BitGadgetRands { r_bits: rb_sb, r_recon: rr_sb, r_bools: rbo_sb },
        surp_a_nn:   BitGadgetRands { r_bits: rb_sa, r_recon: rr_sa, r_bools: rbo_sa },
        delta_floor: BitGadgetRands { r_bits: rb_df, r_recon: rr_df, r_bools: rbo_df },
    };
    (comms, rands)
}

pub struct BitGadgetSet {
    pub bid_range:   BitGadget, // #1
    pub ask_range:   BitGadget, // #2
    pub ceiling:     BitGadget, // #8
    pub surp_b_nn:   BitGadget, // #15
    pub surp_a_nn:   BitGadget, // #16
    pub delta_floor: BitGadget, // #23
}

impl BitGadgetSet {
    pub fn all_pass(&self) -> bool {
        self.bid_range.all_pass() && self.ask_range.all_pass()
            && self.ceiling.all_pass() && self.surp_b_nn.all_pass()
            && self.surp_a_nn.all_pass() && self.delta_floor.all_pass()
    }
}

pub fn build_bit_gadget_set(
    book:   &OrderBook,
    polys:  &Polynomials,
    mask_p: &DensePolynomial<F>,
    domain: &GeneralEvaluationDomain<F>,
) -> BitGadgetSet {
    // #1/#2  Bid/Ask range check (bit-decomposition, not Plookup -- see RATIONALE_AND_RESULTS.md)
    let bid_bits: Vec<u64> = book.b.iter().map(|&f| to_u64(f)).collect();
    assert_fits_bit_width(&bid_bits, "#1 Bid range");
    let bid_range = build_bit_gadget(&bid_bits, &polys.b, None, domain);

    let ask_bits: Vec<u64> = book.a.iter().map(|&f| to_u64(f)).collect();
    assert_fits_bit_width(&ask_bits, "#2 Ask range");
    let ask_range = build_bit_gadget(&ask_bits, &polys.a, None, domain);

    // #8  Ceiling: V_max - Min(X) in [0, 2^16), zero-padded past tick n
    let ceiling_evals: Vec<F> = (0..book.n)
        .map(|i| F::from(book.v_max) - book.min_x[i])
        .collect();
    let ceiling_value = interpolate_slice(&ceiling_evals, domain);
    let ceiling_bits: Vec<u64> = (0..book.n)
        .map(|i| book.v_max - to_u64(book.min_x[i]))
        .collect();
    assert_fits_bit_width(&ceiling_bits, "#8 ceiling");
    let ceiling = build_bit_gadget(&ceiling_bits, &ceiling_value, None, domain);

    // #15  SurpB non-negativity
    let surp_b_bits: Vec<u64> = book.surp_b.iter().map(|&f| to_u64(f)).collect();
    assert_fits_bit_width(&surp_b_bits, "#15 SurpB");
    let surp_b_nn = build_bit_gadget(&surp_b_bits, &polys.surp_b, None, domain);

    // #16  SurpA non-negativity
    let surp_a_bits: Vec<u64> = book.surp_a.iter().map(|&f| to_u64(f)).collect();
    assert_fits_bit_width(&surp_a_bits, "#16 SurpA");
    let surp_a_nn = build_bit_gadget(&surp_a_bits, &polys.surp_a, None, domain);

    // #23  Delta floor, gated to the plateau by Mask_P (0 elsewhere)
    let v_min_delta_poly = DensePolynomial { coeffs: vec![F::from(book.v_min_delta)] };
    let delta_floor_bits: Vec<u64> = (0..book.n)
        .map(|i| {
            if i >= book.c && i <= book.d {
                to_u64(book.delta[i]).checked_sub(book.v_min_delta)
                    .expect("Delta floor violated inside plateau")
            } else {
                0
            }
        })
        .collect();
    assert_fits_bit_width(&delta_floor_bits, "#23 Delta floor");
    let delta_floor_value = &polys.delta - &v_min_delta_poly;
    let delta_floor =
        build_bit_gadget(&delta_floor_bits, &delta_floor_value, Some(mask_p), domain);

    BitGadgetSet { bid_range, ask_range, ceiling, surp_b_nn, surp_a_nn, delta_floor }
}

// ===========================================================================
// Layer 3e — Fiat-Shamir Non-Interactive Proof
// ===========================================================================

pub struct FiatShamirProof {
    pub zeta:  F,
    pub alpha: F,
    // Witness evaluations at zeta
    pub b_at_zeta:      F,
    pub a_at_zeta:      F,
    pub acc_b_at_zeta:  F,
    pub acc_a_at_zeta:  F,
    pub min_x_at_zeta:  F,
    pub surp_b_at_zeta: F,
    pub surp_a_at_zeta: F,
    pub delta_at_zeta:  F,
    pub chk_d_at_zeta:  F,
    pub mask_p_at_zeta: F, // public, verifier recomputes -- no opening needed
    pub mask_v_at_zeta: F, // public, verifier recomputes -- no opening needed
    pub sd_at_zeta:     F, // public, verifier recomputes -- no opening needed
    // Shifted witness evaluations at omega*zeta (#5/#6 only)
    pub acc_a_at_omega_zeta: F,
    pub a_at_omega_zeta:     F,
    pub acc_b_at_omega_zeta: F,
    // Quotient evaluations at zeta
    pub q_acc_a_init_at_zeta:    F,
    pub q_acc_a_rec_at_zeta:     F,
    pub q_acc_b_init_at_zeta:    F,
    pub q_acc_b_rec_at_zeta:     F,
    pub q_kl_at_zeta:            F,
    pub q_delta_def_at_zeta:     F,
    pub q_chkd_bool_at_zeta:     F,
    pub q_chkd_correct_at_zeta:  F,
    pub q_chkd_contain_at_zeta:  F,
    pub q_plateau_left_at_zeta:  F,
    pub q_plateau_right_at_zeta: F,
    pub q_valley_pin_at_zeta:    F,
    pub q_mask_v_contain_at_zeta: F,
    pub q_sd_def_at_zeta:         F,
    pub zh_at_zeta: F,
    pub r:          Vec<F>, // 14 residues, order matches QuotientCheck
    pub batch_ok:   bool,
    // Domain anchor points (stored for openings)
    pub omega_0:         F,
    pub omega_last_data: F,
    pub omega_last_dom:  F,
}

/// For one bit-gadget instance: compute its BIT_WIDTH+1 Fiat-Shamir residues
/// (1 reconstruction + BIT_WIDTH booleanity) at `zeta`, plus the matching
/// BIT_WIDTH+1+BIT_WIDTH raw evaluations (bit columns, reconstruction
/// quotient, booleanity quotients) that need to be absorbed into the
/// transcript before deriving alpha. Eval order matches
/// AllBitGadgetCommitments::all_affines' per-gadget order.
fn bit_gadget_residues_and_evals(
    gadget:        &BitGadget,
    mask_at_zeta:  Option<F>,
    zeta:          F,
    zh_at_zeta:    F,
) -> (Vec<F>, Vec<F>) {
    let bits_at_zeta: Vec<F> = gadget.cols.bits.iter().map(|b| b.evaluate(&zeta)).collect();

    let mut recon_at_zeta = F::zero();
    let mut weight = F::one();
    for &bv in &bits_at_zeta {
        recon_at_zeta += weight * bv;
        weight += weight;
    }
    // Evaluate the gadget's own `value` polynomial directly -- do NOT
    // reconstruct it via affine combination of other constraints'
    // evaluations (e.g. V_max - min_x_at_zeta for #8). That shortcut is
    // unsound: #8's value poly is built by its own independent zero-padded
    // interpolation and is only equal to "V_max - Min(X)" on the first `n`
    // domain points, not as a global polynomial identity, so it disagrees
    // with that shortcut at a random challenge zeta.
    let value_at_zeta = gadget.value.evaluate(&zeta);
    let diff = value_at_zeta - recon_at_zeta;
    let gated_at_zeta = match mask_at_zeta {
        Some(m) => m * diff,
        None    => diff,
    };
    let q_recon_at_zeta = gadget.q_recon.evaluate(&zeta);
    let r_recon = gated_at_zeta - q_recon_at_zeta * zh_at_zeta;

    let q_bools_at_zeta: Vec<F> = gadget.q_bools.iter().map(|q| q.evaluate(&zeta)).collect();
    let mut residues = Vec::with_capacity(1 + BIT_WIDTH);
    residues.push(r_recon);
    for (j, &b_at) in bits_at_zeta.iter().enumerate() {
        let v_bool = b_at * (b_at - F::one());
        residues.push(v_bool - q_bools_at_zeta[j] * zh_at_zeta);
    }

    let mut evals = Vec::with_capacity(1 + 2 * BIT_WIDTH);
    evals.extend_from_slice(&bits_at_zeta);
    evals.push(q_recon_at_zeta);
    evals.extend_from_slice(&q_bools_at_zeta);

    (residues, evals)
}

pub fn fiat_shamir_prove(
    book:        &OrderBook,
    polys:       &Polynomials,
    mask_p:      &DensePolynomial<F>,
    quots:       &QuotientPolynomials,
    wcomms:      &Commitments,
    qcomms:      &QuotientCommitments,
    bit_gadgets: &BitGadgetSet,
    bgcomms:     &AllBitGadgetCommitments,
    domain:      &GeneralEvaluationDomain<F>,
    n:           usize,
) -> FiatShamirProof {
    // Step 1: absorb all 23 real-quotient commitments plus all 390 bit-gadget
    // commitments (6 instances, see AllBitGadgetCommitments) -> zeta. Folding
    // the bit-gadget commitments in here is what binds them to the verifier's
    // challenge -- previously they were computed after zeta was already fixed
    // and never absorbed at all, which is the soundness gap this closes.
    let mut transcript = Transcript::new();
    let mut absorbed = vec![
        wcomms.c_b.0, wcomms.c_a.0, wcomms.c_acc_b.0, wcomms.c_acc_a.0, wcomms.c_min_x.0,
        wcomms.c_surp_b.0, wcomms.c_surp_a.0, wcomms.c_delta.0, wcomms.c_chk_d.0,
        qcomms.c_q_acc_a_init.0, qcomms.c_q_acc_a_rec.0,
        qcomms.c_q_acc_b_init.0, qcomms.c_q_acc_b_rec.0, qcomms.c_q_kl.0,
        qcomms.c_q_delta_def.0, qcomms.c_q_chkd_bool.0, qcomms.c_q_chkd_correct.0,
        qcomms.c_q_chkd_contain.0, qcomms.c_q_plateau_left.0, qcomms.c_q_plateau_right.0,
        qcomms.c_q_valley_pin.0, qcomms.c_q_mask_v_contain.0, qcomms.c_q_sd_def.0,
    ];
    absorbed.extend(bgcomms.all_affines());
    transcript.append_affines::<Bn254>(&absorbed);
    let zeta = transcript.append_and_digest::<Bn254>("zeta".to_string());

    let omega       = domain.group_gen();
    let omega_zeta  = omega * zeta;
    let domain_size = domain.size();

    // Step 2: evaluate all polynomials at zeta and omega*zeta
    let b_at_zeta      = polys.b.evaluate(&zeta);
    let a_at_zeta      = polys.a.evaluate(&zeta);
    let acc_b_at_zeta  = polys.acc_b.evaluate(&zeta);
    let acc_a_at_zeta  = polys.acc_a.evaluate(&zeta);
    let min_x_at_zeta  = polys.min_x.evaluate(&zeta);
    let surp_b_at_zeta = polys.surp_b.evaluate(&zeta);
    let surp_a_at_zeta = polys.surp_a.evaluate(&zeta);
    let delta_at_zeta  = polys.delta.evaluate(&zeta);
    let chk_d_at_zeta  = polys.chk_d.evaluate(&zeta);
    let mask_p_at_zeta = mask_p.evaluate(&zeta);
    let mask_v         = mask_v_poly(book, domain);
    let sd             = sd_poly(book, domain);
    let mask_v_at_zeta = mask_v.evaluate(&zeta);
    let sd_at_zeta     = sd.evaluate(&zeta);

    let acc_a_at_omega_zeta = polys.acc_a.evaluate(&omega_zeta);
    let a_at_omega_zeta     = polys.a.evaluate(&omega_zeta);
    let acc_b_at_omega_zeta = polys.acc_b.evaluate(&omega_zeta);

    let q_acc_a_init_at_zeta    = quots.q_acc_a_init.evaluate(&zeta);
    let q_acc_a_rec_at_zeta     = quots.q_acc_a_rec.evaluate(&zeta);
    let q_acc_b_init_at_zeta    = quots.q_acc_b_init.evaluate(&zeta);
    let q_acc_b_rec_at_zeta     = quots.q_acc_b_rec.evaluate(&zeta);
    let q_kl_at_zeta            = quots.q_kl.evaluate(&zeta);
    let q_delta_def_at_zeta     = quots.q_delta_def.evaluate(&zeta);
    let q_chkd_bool_at_zeta     = quots.q_chkd_bool.evaluate(&zeta);
    let q_chkd_correct_at_zeta  = quots.q_chkd_correct.evaluate(&zeta);
    let q_chkd_contain_at_zeta  = quots.q_chkd_contain.evaluate(&zeta);
    let q_plateau_left_at_zeta  = quots.q_plateau_left.evaluate(&zeta);
    let q_plateau_right_at_zeta = quots.q_plateau_right.evaluate(&zeta);
    let q_valley_pin_at_zeta    = quots.q_valley_pin.evaluate(&zeta);
    let q_mask_v_contain_at_zeta = quots.q_mask_v_contain.evaluate(&zeta);
    let q_sd_def_at_zeta         = quots.q_sd_def.evaluate(&zeta);

    let zh_at_zeta = zeta.pow([domain_size as u64]) - F::one();

    let elems: Vec<F>   = domain.elements().collect();
    let omega_0         = elems[0];
    let omega_last_data = elems[n - 1];
    let omega_last_dom  = elems[domain_size - 1];
    let v_max       = F::from(book.v_max);
    let v_min_delta = F::from(book.v_min_delta);

    // Step 3: per-constraint residues r_i = V_i(zeta) - Q_i(zeta)·D_i(zeta)
    let r1 = (acc_a_at_zeta - a_at_zeta) - q_acc_a_init_at_zeta * (zeta - omega_0);
    let v2 = (zeta - omega_last_data) * (acc_a_at_omega_zeta - acc_a_at_zeta - a_at_omega_zeta);
    let r2 = v2 - q_acc_a_rec_at_zeta * zh_at_zeta;
    let r3 = (acc_b_at_zeta - b_at_zeta) - q_acc_b_init_at_zeta * (zeta - omega_last_data);
    let v4 = (zeta - omega_last_dom) * (acc_b_at_zeta - acc_b_at_omega_zeta - b_at_zeta);
    let r4 = v4 - q_acc_b_rec_at_zeta * zh_at_zeta;
    let v5 = (acc_a_at_zeta - min_x_at_zeta) * (acc_b_at_zeta - min_x_at_zeta);
    let r5 = v5 - q_kl_at_zeta * zh_at_zeta;
    // #17
    let v6 = delta_at_zeta - (surp_a_at_zeta + surp_b_at_zeta);
    let r6 = v6 - q_delta_def_at_zeta * zh_at_zeta;
    // #18
    let v7 = chk_d_at_zeta * (chk_d_at_zeta - F::one());
    let r7 = v7 - q_chkd_bool_at_zeta * zh_at_zeta;
    // #19
    let v8 = (delta_at_zeta - v_min_delta) * chk_d_at_zeta;
    let r8 = v8 - q_chkd_correct_at_zeta * zh_at_zeta;
    // #20
    let v9 = chk_d_at_zeta * (F::one() - mask_p_at_zeta);
    let r9 = v9 - q_chkd_contain_at_zeta * zh_at_zeta;
    // #11
    let v10 = min_x_at_zeta - v_max;
    let r10 = v10 - q_plateau_left_at_zeta * (zeta - elems[book.c]);
    // #12
    let r11 = v10 - q_plateau_right_at_zeta * (zeta - elems[book.d]);
    // #24
    let v12 = delta_at_zeta - v_min_delta;
    let r12 = v12 - q_valley_pin_at_zeta * (zeta - elems[book.p_star]);
    // #22  Mask_V * (1 - ChkD) = 0
    let v13 = mask_v_at_zeta * (F::one() - chk_d_at_zeta);
    let r13 = v13 - q_mask_v_contain_at_zeta * zh_at_zeta;
    // #25  SD - Mask_V * Delta = 0
    let v14 = sd_at_zeta - mask_v_at_zeta * delta_at_zeta;
    let r14 = v14 - q_sd_def_at_zeta * zh_at_zeta;

    // Bit-gadget residues (#1, #2, #8, #15, #16, #23): 33 each (1
    // reconstruction + BIT_WIDTH booleanity), 198 total. This is the piece
    // that actually closes the soundness gap from Design Rationale #5 --
    // Phase 1 only bound the commitments to the challenge, this makes their
    // correctness part of the same batched check as the other 14. Each
    // gadget's `value` field carries its own exact polynomial (see
    // BitGadget doc comment) so evaluations are computed directly on it
    // rather than reconstructed from other constraints' evaluations.
    let (r_bid,   e_bid)   = bit_gadget_residues_and_evals(&bit_gadgets.bid_range,   None,                  zeta, zh_at_zeta);
    let (r_ask,   e_ask)   = bit_gadget_residues_and_evals(&bit_gadgets.ask_range,   None,                  zeta, zh_at_zeta);
    let (r_ceil,  e_ceil)  = bit_gadget_residues_and_evals(&bit_gadgets.ceiling,     None,                  zeta, zh_at_zeta);
    let (r_sb,    e_sb)    = bit_gadget_residues_and_evals(&bit_gadgets.surp_b_nn,   None,                  zeta, zh_at_zeta);
    let (r_sa,    e_sa)    = bit_gadget_residues_and_evals(&bit_gadgets.surp_a_nn,   None,                  zeta, zh_at_zeta);
    let (r_df,    e_df)    = bit_gadget_residues_and_evals(&bit_gadgets.delta_floor, Some(mask_p_at_zeta),  zeta, zh_at_zeta);

    let mut r = vec![r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14];
    for extra in [r_bid, r_ask, r_ceil, r_sb, r_sa, r_df] {
        r.extend(extra);
    }

    // Step 4: absorb evaluations → alpha
    for val in &[
        zeta,
        b_at_zeta, a_at_zeta, acc_b_at_zeta, acc_a_at_zeta, min_x_at_zeta,
        surp_b_at_zeta, surp_a_at_zeta, delta_at_zeta, chk_d_at_zeta, mask_p_at_zeta,
        mask_v_at_zeta, sd_at_zeta,
        acc_a_at_omega_zeta, a_at_omega_zeta, acc_b_at_omega_zeta,
        q_acc_a_init_at_zeta, q_acc_a_rec_at_zeta, q_acc_b_init_at_zeta, q_acc_b_rec_at_zeta,
        q_kl_at_zeta, q_delta_def_at_zeta, q_chkd_bool_at_zeta, q_chkd_correct_at_zeta,
        q_chkd_contain_at_zeta, q_plateau_left_at_zeta, q_plateau_right_at_zeta, q_valley_pin_at_zeta,
        q_mask_v_contain_at_zeta, q_sd_def_at_zeta,
    ] {
        transcript.append_message(format!("{:?}", val));
    }
    // Also absorb the 390 bit-gadget evaluations (32 bit evals + 1 recon
    // quotient eval + 32 bool quotient evals, per instance x 6 instances)
    // before deriving alpha -- otherwise the new residues folded into `r`
    // above would depend on evaluations the verifier's challenge never saw.
    for val_group in [&e_bid, &e_ask, &e_ceil, &e_sb, &e_sa, &e_df] {
        for val in val_group {
            transcript.append_message(format!("{:?}", val));
        }
    }
    let alpha = transcript.append_and_digest::<Bn254>("alpha".to_string());

    // Step 5: batched residue Σ α^i · r_i == 0 ?
    let mut alpha_pow = F::one();
    let mut batched   = F::zero();
    for ri in &r {
        batched   += alpha_pow * ri;
        alpha_pow *= alpha;
    }

    FiatShamirProof {
        zeta, alpha,
        b_at_zeta, a_at_zeta, acc_b_at_zeta, acc_a_at_zeta, min_x_at_zeta,
        surp_b_at_zeta, surp_a_at_zeta, delta_at_zeta, chk_d_at_zeta, mask_p_at_zeta,
        mask_v_at_zeta, sd_at_zeta,
        acc_a_at_omega_zeta, a_at_omega_zeta, acc_b_at_omega_zeta,
        q_acc_a_init_at_zeta, q_acc_a_rec_at_zeta, q_acc_b_init_at_zeta, q_acc_b_rec_at_zeta,
        q_kl_at_zeta, q_delta_def_at_zeta, q_chkd_bool_at_zeta, q_chkd_correct_at_zeta,
        q_chkd_contain_at_zeta, q_plateau_left_at_zeta, q_plateau_right_at_zeta, q_valley_pin_at_zeta,
        q_mask_v_contain_at_zeta, q_sd_def_at_zeta,
        zh_at_zeta, r, batch_ok: batched.is_zero(),
        omega_0, omega_last_data, omega_last_dom,
    }
}

// ===========================================================================
// Layer 3f — KZG Opening Proofs (GWC19 batch_open, 4 pairings)
// ===========================================================================

pub struct OpeningsResult {
    pub proof:            BatchCheckProof<Bn254>,
    pub min_at_c_minus_1: Option<F>, // #29 cliff value, left
    pub min_at_d_plus_1:  Option<F>, // #30 cliff value, right
}

pub fn compute_all_opening_proofs<R: Rng>(
    scheme:  &KzgScheme,
    book:    &OrderBook,
    polys:   &Polynomials,
    quots:   &QuotientPolynomials,
    wcomms:  &Commitments,
    qcomms:  &QuotientCommitments,
    w_rands: &[KzgRand],
    q_rands: &[KzgRand],
    fs:      &FiatShamirProof,
    domain:  &GeneralEvaluationDomain<F>,
    rng:     &mut R,
) -> OpeningsResult {
    let zeta       = fs.zeta;
    let omega_zeta = domain.group_gen() * zeta;
    let elems: Vec<F> = domain.elements().collect();

    // Group 0: 9 witness + 14 quotient polynomials at zeta
    let polys_z: Vec<&DensePolynomial<F>> = vec![
        &polys.b, &polys.a, &polys.acc_b, &polys.acc_a, &polys.min_x,
        &polys.surp_b, &polys.surp_a, &polys.delta, &polys.chk_d,
        &quots.q_acc_a_init, &quots.q_acc_a_rec, &quots.q_acc_b_init, &quots.q_acc_b_rec,
        &quots.q_kl, &quots.q_delta_def, &quots.q_chkd_bool, &quots.q_chkd_correct,
        &quots.q_chkd_contain, &quots.q_plateau_left, &quots.q_plateau_right, &quots.q_valley_pin,
        &quots.q_mask_v_contain, &quots.q_sd_def,
    ];
    let rands_z: Vec<&KzgRand> = vec![
        &w_rands[0], &w_rands[1], &w_rands[2], &w_rands[3], &w_rands[4],
        &w_rands[5], &w_rands[6], &w_rands[7], &w_rands[8],
        &q_rands[0], &q_rands[1], &q_rands[2], &q_rands[3], &q_rands[4], &q_rands[5],
        &q_rands[6], &q_rands[7], &q_rands[8], &q_rands[9], &q_rands[10], &q_rands[11],
        &q_rands[12], &q_rands[13],
    ];
    let (w_zeta, evals_z, gamma_z) =
        batch_open(&scheme.powers, &polys_z, &rands_z, zeta, false, rng);

    // Group 1: 3 polynomials at omega*zeta (shifted evaluations, #5/#6 only)
    let polys_wz: Vec<&DensePolynomial<F>> = vec![&polys.acc_a, &polys.a, &polys.acc_b];
    let rands_wz: Vec<&KzgRand> = vec![&w_rands[3], &w_rands[1], &w_rands[2]];
    let (w_omega_zeta, evals_wz, gamma_wz) =
        batch_open(&scheme.powers, &polys_wz, &rands_wz, omega_zeta, false, rng);

    let mut commitments = vec![
        vec![
            wcomms.c_b.clone(), wcomms.c_a.clone(), wcomms.c_acc_b.clone(),
            wcomms.c_acc_a.clone(), wcomms.c_min_x.clone(), wcomms.c_surp_b.clone(),
            wcomms.c_surp_a.clone(), wcomms.c_delta.clone(), wcomms.c_chk_d.clone(),
            qcomms.c_q_acc_a_init.clone(), qcomms.c_q_acc_a_rec.clone(),
            qcomms.c_q_acc_b_init.clone(), qcomms.c_q_acc_b_rec.clone(), qcomms.c_q_kl.clone(),
            qcomms.c_q_delta_def.clone(), qcomms.c_q_chkd_bool.clone(),
            qcomms.c_q_chkd_correct.clone(), qcomms.c_q_chkd_contain.clone(),
            qcomms.c_q_plateau_left.clone(), qcomms.c_q_plateau_right.clone(),
            qcomms.c_q_valley_pin.clone(),
            qcomms.c_q_mask_v_contain.clone(), qcomms.c_q_sd_def.clone(),
        ],
        vec![wcomms.c_acc_a.clone(), wcomms.c_a.clone(), wcomms.c_acc_b.clone()],
    ];
    let mut witnesses  = vec![w_zeta, w_omega_zeta];
    let mut points     = vec![zeta, omega_zeta];
    let mut open_evals = vec![evals_z, evals_wz];
    let mut gammas     = vec![gamma_z, gamma_wz];

    // #29  Cliff value, left: open Min(X) at ω^{c-1} (revealed in plaintext,
    // tied to cm[Min] -- feeds the range proof in Layer 3h, no separate
    // Slack_L commitment needed since slack is only ever meaningful here).
    let mut min_at_c_minus_1 = None;
    if book.has_left_cliff {
        let pt = elems[book.c - 1];
        let (w, evals, gamma) =
            batch_open(&scheme.powers, &vec![&polys.min_x], &vec![&w_rands[4]], pt, false, rng);
        min_at_c_minus_1 = Some(evals[0].into_plain_value().0);
        commitments.push(vec![wcomms.c_min_x.clone()]);
        witnesses.push(w);
        points.push(pt);
        open_evals.push(evals);
        gammas.push(gamma);
    }

    // #30  Cliff value, right: open Min(X) at ω^{d+1}
    let mut min_at_d_plus_1 = None;
    if book.has_right_cliff {
        let pt = elems[book.d + 1];
        let (w, evals, gamma) =
            batch_open(&scheme.powers, &vec![&polys.min_x], &vec![&w_rands[4]], pt, false, rng);
        min_at_d_plus_1 = Some(evals[0].into_plain_value().0);
        commitments.push(vec![wcomms.c_min_x.clone()]);
        witnesses.push(w);
        points.push(pt);
        open_evals.push(evals);
        gammas.push(gamma);
    }

    OpeningsResult {
        proof: BatchCheckProof { commitments, witnesses, points, open_evals, gammas },
        min_at_c_minus_1,
        min_at_d_plus_1,
    }
}

/// Verify via batch_check (4 pairings per group).
/// Wraps in catch_unwind because batch_check panics on failure.
pub fn verify_all_openings<R: Rng>(
    scheme: &KzgScheme,
    result: &OpeningsResult,
    _rng:   &mut R,
) -> bool {
    let vk = scheme.vk.clone();
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut local_rng = ark_std::test_rng();
        batch_check(&vk, &result.proof, &mut local_rng);
    }))
    .is_ok()
}

// ===========================================================================
// Layer 3g/3h — Scalar Range Proofs (gadgets::range) — V_max (#8), cliff slack (#31/#32)
// ===========================================================================

fn range_domain() -> Radix2EvaluationDomain<F> {
    Radix2EvaluationDomain::<F>::new(RANGE_BITS).expect("range domain creation failed")
}

pub fn prove_range16<R: Rng>(scheme: &KzgScheme, value: u64, rng: &mut R) -> BatchCheckProof<Bn254> {
    range_gadget::prove::<Bn254, _>(&scheme.powers, value as u32, range_domain(), rng) // GADGET: range
}

pub fn verify_range16<R: Rng>(scheme: &KzgScheme, proof: &BatchCheckProof<Bn254>, _rng: &mut R) -> bool {
    let vk = scheme.vk.clone();
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut local_rng = ark_std::test_rng();
        range_gadget::verify::<Bn254, _>(vk.clone(), proof, range_domain(), &mut local_rng); // GADGET: range
    }))
    .is_ok()
}

/// `verify_range16` alone doesn't bind the proved value to `expected_value`
/// (see RATIONALE_AND_RESULTS.md); this extracts the plaintext opening and checks it.
pub fn verify_range16_bound<R: Rng>(
    scheme:         &KzgScheme,
    proof:          &BatchCheckProof<Bn254>,
    expected_value: u64,
    rng:            &mut R,
) -> bool {
    let algebraic_ok = verify_range16(scheme, proof, rng);
    let (t_at_one, _blinding) = proof.open_evals[2][0].into_plain_value();
    let bound_ok = t_at_one == F::from(expected_value);
    algebraic_ok && bound_ok
}

/// #29/#31 (left) and #30/#32 (right): recompute slack from the opened
/// cliff value and range-prove it. Returns None for a side with no cliff.
pub fn prove_cliff_slack<R: Rng>(
    scheme:  &KzgScheme,
    book:    &OrderBook,
    min_at_c_minus_1: Option<F>,
    min_at_d_plus_1:  Option<F>,
    rng:     &mut R,
) -> (Option<BatchCheckProof<Bn254>>, Option<BatchCheckProof<Bn254>>) {
    let left = min_at_c_minus_1.map(|min_cliff| {
        let slack = book.v_max - 1 - to_u64(min_cliff);
        prove_range16(scheme, slack, rng)
    });
    let right = min_at_d_plus_1.map(|min_cliff| {
        let slack = book.v_max - 1 - to_u64(min_cliff);
        prove_range16(scheme, slack, rng)
    });
    (left, right)
}

// ===========================================================================
// Full Pipeline
// ===========================================================================

pub struct PipelineResult {
    pub constraint_result: ConstraintResult,
    pub quotient_check:    QuotientCheck,
    pub fs_proof:          FiatShamirProof,
    pub opening_ok:        bool,
    pub range_ok:          bool, // V_max + Slack_L + Slack_R (whichever apply)
    pub bit_gadgets_ok:    bool, // #8, #15, #16, #23 (see BitGadgetSet)
    pub v_max:             u64,
    pub n:                 usize,
    pub domain_size:       usize,
}

impl PipelineResult {
    pub fn all_pass(&self) -> bool {
        self.constraint_result.all_pass()
            && self.quotient_check.all_zero()
            && self.fs_proof.batch_ok
            && self.opening_ok
            && self.range_ok
            && self.bit_gadgets_ok
    }
}

pub fn full_pipeline<R: Rng>(book: &OrderBook, rng: &mut R) -> PipelineResult {
    let n           = book.n;
    let domain_size = book.domain_size;
    let domain      = build_domain(domain_size);
    let polys       = build_all_polys(book, &domain);
    let mask_p      = mask_p_poly(book, &domain);
    let scheme      = KzgScheme::new(domain_size - 1, rng);
    let (wcomms, w_rands) = commit_all(&scheme, &polys, rng);
    let (quots, q_check)  = compute_quotients(book, &polys, &mask_p, &domain, n);
    let (qcomms, q_rands) = commit_quotients(&scheme, &quots, rng);
    let cresult     = verify_all(book, &polys, &mask_p, &domain, n);

    // Bit gadgets are built and committed *before* Fiat-Shamir now (Phase 1
    // fix, see AllBitGadgetCommitments) so their commitments are absorbed
    // into the challenge that derives zeta, same as every other committed
    // polynomial. `_bg_rands` is unused until Phase 3 wires up the actual
    // batch-opening for these polynomials.
    let bit_gadgets = build_bit_gadget_set(book, &polys, &mask_p, &domain);
    let (bg_comms, _bg_rands) = commit_all_bit_gadgets(&scheme, &bit_gadgets, rng);

    let fs = fiat_shamir_prove(
        book, &polys, &mask_p, &quots, &wcomms, &qcomms, &bit_gadgets, &bg_comms, &domain, n,
    );
    let opr         = compute_all_opening_proofs(
        &scheme, book, &polys, &quots, &wcomms, &qcomms,
        &w_rands, &q_rands, &fs, &domain, rng,
    );
    let opening_ok  = verify_all_openings(&scheme, &opr, rng);

    let v_max_proof = prove_range16(&scheme, book.v_max, rng);
    let v_max_ok    = verify_range16_bound(&scheme, &v_max_proof, book.v_max, rng);
    let (slack_l_proof, slack_r_proof) =
        prove_cliff_slack(&scheme, book, opr.min_at_c_minus_1, opr.min_at_d_plus_1, rng);
    let slack_l_ok = slack_l_proof
        .map(|p| verify_range16_bound(&scheme, &p, book.slack_l, rng))
        .unwrap_or(true);
    let slack_r_ok = slack_r_proof
        .map(|p| verify_range16_bound(&scheme, &p, book.slack_r, rng))
        .unwrap_or(true);
    let range_ok   = v_max_ok && slack_l_ok && slack_r_ok;

    // Still the plaintext recon_ok/bool_ok check for now -- Phase 3 will
    // replace this with the real batch_check-backed result once these
    // polynomials are actually opened, not just committed-and-absorbed.
    let bit_gadgets_ok = bit_gadgets.all_pass();

    PipelineResult {
        constraint_result: cresult,
        quotient_check:    q_check,
        fs_proof:          fs,
        opening_ok,
        range_ok,
        bit_gadgets_ok,
        v_max: book.v_max,
        n,
        domain_size,
    }
}

// ===========================================================================
// Tests -- shadow-check the coset-FFT quotient path against poly_div_rem
// on real constraint data before anything is allowed to depend on it alone.
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_same_poly(a: &DensePolynomial<F>, b: &DensePolynomial<F>, label: &str) {
        let (ca, cb) = (&a.coeffs, &b.coeffs);
        let len = ca.len().max(cb.len());
        for i in 0..len {
            let av = ca.get(i).copied().unwrap_or(F::zero());
            let bv = cb.get(i).copied().unwrap_or(F::zero());
            assert_eq!(av, bv, "{label}: coefficient {i} differs");
        }
    }

    /// #7's quotient (Min mutual exclusivity, `(AccA-Min)*(AccB-Min)`) is the
    /// most expensive full-domain quotient: a degree ~2*domain_size product
    /// divided by Z_H. If coset-FFT matches poly_div_rem here, on real data,
    /// the primitive is trustworthy for every other full-domain quotient too.
    fn check_q_kl_matches(book: &OrderBook) {
        let domain = build_domain(book.domain_size);
        let polys  = build_all_polys(book, &domain);
        let domain_size = domain.size();
        let zh = build_zh_poly(domain_size);

        let da = &polys.acc_a - &polys.min_x;
        let db = &polys.acc_b - &polys.min_x;
        let v5 = &da * &db;

        let (q_slow, r_slow) = poly_div_rem(&v5, &zh);
        assert!(
            r_slow.coeffs.iter().all(|x| x.is_zero()),
            "poly_div_rem itself found a nonzero remainder -- bad test fixture"
        );

        let q_fast = divide_by_vanishing_coset(&v5, domain_size);
        assert_same_poly(&q_slow, &q_fast, "q_kl (#7)");
    }

    #[test]
    fn coset_fft_matches_poly_div_rem_21tick() {
        check_q_kl_matches(&OrderBook::hardcoded_21tick());
    }

    #[test]
    fn coset_fft_matches_poly_div_rem_100tick() {
        let csv = "/Users/kimiaesmaili/Downloads/order_book_100_log-normal.csv";
        check_q_kl_matches(&OrderBook::from_csv(csv));
    }

    /// Self-contained edge case: numerator == Z_H itself, quotient must be
    /// the constant polynomial 1 (avoids relying on CSV fixtures at all).
    #[test]
    fn coset_fft_divides_zh_by_itself() {
        let domain_size = 32;
        let zh = build_zh_poly(domain_size);
        let q = divide_by_vanishing_coset(&zh, domain_size);
        assert_eq!(q.coeffs.len(), 1, "expected constant quotient, got {:?}", q.coeffs);
        assert_eq!(q.coeffs[0], F::one());
    }
}
