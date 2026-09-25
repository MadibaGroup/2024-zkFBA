//! Step 1 of the bit-gadget aggregation redesign -- additive only, does not
//! modify any existing function/struct in lib.rs (see NEXT_OPTIMIZATION_
//! HANDOFF.md item 4 for the background this responds to).
//!
//! Each of the 6 bit-decomposition gadget instances in lib.rs currently
//! commits+opens 65 polynomials per instance (BIT_WIDTH=32 bit columns + 1
//! reconstruction quotient + 32 individually-committed booleanity
//! quotients, one per bit column). This module replaces the 32 individual
//! booleanity quotients with ONE random-linear-combination (RLC) aggregate
//! quotient, for a single gadget instance (mirroring constraint #1,
//! bid-range: mask=None, value=B(X), same choice lib.rs's own
//! `bit_gadget_opening_rejects_swapped_commitment` test uses). That takes
//! this one instance from 65 -> 34 committed/opened polynomials (32 bit
//! columns + 1 recon quotient + 1 aggregated booleanity quotient).
//!
//! ## Soundness of the aggregation
//!
//! Instead of proving `numerator_j(X) = B_j(X)*(B_j(X)-1)` is divisible by
//! `Z_H(X)` for each bit column j=0..31 independently, this proves
//! `sum_j beta^j * numerator_j(X)` is divisible by `Z_H(X)` for a single
//! challenge `beta`. If any individual `numerator_j` were NOT divisible by
//! `Z_H` (i.e. some bit column takes a non-{0,1} value somewhere on the
//! domain), the RLC could only still divide evenly by `Z_H` if the
//! (deg < domain_size)-degree combination of the individual remainders,
//! viewed as a polynomial in `beta`, happens to vanish at the *specific*
//! `beta` used -- a Schwartz-Zippel bound puts that probability at
//! `(domain_size - 1) / |F|`, negligible over BN254's ~254-bit scalar
//! field. This is the same style of argument this codebase already relies
//! on for the alpha-folded residue check in `fiat_shamir_prove` and for the
//! r-weighted aggregation inside `gadgets::utils::batch_check` itself.
//!
//! Critically, `beta` is derived via Fiat-Shamir *after* the 32 bit-column
//! commitments already exist (`build_and_commit_agg_bit_gadget` below),
//! exactly the "challenge after commit" ordering this codebase already
//! depends on elsewhere (`alpha`/`zeta` in `fiat_shamir_prove`) -- a
//! prover who could pick `beta` first could search for one bit pattern
//! that happens to cancel out.
//!
//! A second, independent soundness layer exists at verification time:
//! `divide_by_vanishing_coset_cached` is FFT-based and does not itself
//! reject an inexact division (unlike `poly_div_rem`'s explicit
//! remainder-is-zero check) -- so the real barrier against a forged
//! aggregate quotient is `verify_aggregated`'s algebraic identity check at
//! the Fiat-Shamir point `zeta`: `q_bool_agg(zeta) * Z_H(zeta) ==
//! sum_j beta^j * (b_j(zeta)^2 - b_j(zeta))`, computed purely from the
//! `batch_check`-verified plain openings. `zeta` is unpredictable to the
//! prover at forgery time (it is fixed only after all commitments exist),
//! so this is the same "spot check" argument lib.rs's own
//! `quotient_holds_at` doc comment already makes for the un-aggregated
//! gadgets. See `agg_bool_forgery_rejected_at_zeta` below for the test
//! that exercises exactly this path.

use ark_bn254::{Bn254, Fr as F, G1Affine};
use ark_ff::{batch_inversion, FftField, Field, One, Zero};
use ark_poly::{
    univariate::DensePolynomial, EvaluationDomain, Evaluations, GeneralEvaluationDomain,
    Polynomial,
};
use ark_poly_commit::kzg10::Commitment;
use ark_std::rand::Rng;
use gadgets::utils::{batch_check, BatchCheckProof, Transcript};
use rayon::prelude::*;

use crate::{
    bit_reconstruct, divide_by_vanishing_coset_cached, parallel_batch_open,
    parallel_commit_many, BitColumns, CosetCache, KzgRand, KzgScheme, BIT_WIDTH,
};

// ===========================================================================
// Local reimplementations of lib.rs's private helpers.
//
// `quotient_holds_at` and `scale_poly` are private to lib.rs's root module
// (Rust privacy is per-module, not per-crate), and per the chosen "new
// parallel path, original untouched" layout, lib.rs's visibility is not to
// be changed for this step. Both are 2-3 line pure functions; duplicating
// them here is cheaper and safer than widening lib.rs's public surface.
// ===========================================================================

fn quotient_holds_at_local(
    numerator: &DensePolynomial<F>,
    quotient: &DensePolynomial<F>,
    domain_size: usize,
    point: F,
) -> bool {
    let zh_at_point = point.pow([domain_size as u64]) - F::one();
    numerator.evaluate(&point) == quotient.evaluate(&point) * zh_at_point
}

// Only used by `agg_bool_quotient_fast_matches_naive`'s reconstruction of
// the old (now-replaced) naive construction path -- see that test.
#[cfg(test)]
fn scale_poly_local(poly: &DensePolynomial<F>, s: F) -> DensePolynomial<F> {
    let mut coeffs: Vec<F> = poly.coeffs.iter().map(|&c| c * s).collect();
    while coeffs.last().map(|x: &F| x.is_zero()).unwrap_or(false) {
        coeffs.pop();
    }
    DensePolynomial { coeffs }
}

/// Compute the RLC-aggregated quotient `q` such that `q * Z_H == sum_k
/// weights[k] * transform(polys[k](X))`, via one coset-evaluation pass per
/// input polynomial and a single inverse FFT -- generalizes what was
/// originally a bespoke, one-off implementation of this trick specific to
/// booleanity quotients (`compute_agg_bool_quotient_fast` below is now a
/// thin wrapper around this).
///
/// The naive construction of such a quotient (still what
/// `divide_by_vanishing_coset_cached` does for every other quotient in this
/// codebase, and what `naive_agg_bool_quotient`'s test-only reconstruction
/// below emulates) builds the aggregated numerator in *coefficient form*
/// via `polys.len()` iterations of a poly-poly `transform`, each an
/// FFT-based `DensePolynomial` `Mul` (2 forward FFTs + 1 inverse FFT via
/// `interpolate()`, `ark-poly-0.4.2/src/polynomial/univariate/dense.rs:
/// 578-595`), then scales, sums, and finally re-evaluates the *summed*
/// result on a coset (1 more forward FFT) before a final inverse FFT.
/// That's `~3*polys.len()+2` FFTs total (~98 for `polys.len()`=`BIT_WIDTH`
/// =32), all of size ~2*domain_size, dominating this codebase's real
/// measured latency (`bitgadget_build_commit_ms` was ~67% of total wall
/// time at n=1000 -- see AGG_PIPELINE_HANDOFF.md's profiling update).
///
/// This function computes the mathematically identical polynomial with far
/// fewer FFTs: evaluation is a ring homomorphism, so `transform(poly)(x) ==
/// transform(poly(x))` pointwise for every coset point `x` when `transform`
/// is itself a polynomial function of one variable (true for both call
/// sites this module uses: `|v| v*v - v` for booleanity, and the identity
/// `|v| v` a future linear aggregation -- e.g. RLC-aggregating reconstruction
/// quotients, see AGG_PIPELINE_HANDOFF.md future work -- would use) -- no
/// need to ever form the degree-doubled product polynomial in coefficient
/// form. Each input polynomial is evaluated on the coset exactly once (1
/// forward FFT each, rayon-parallel), the RLC-weighted sum is accumulated
/// directly in evaluation form (pure pointwise field arithmetic, no FFT),
/// and exactly one inverse FFT recovers `q`'s coefficients. Total:
/// `polys.len()+1` FFTs instead of `~3*polys.len()+2` -- about a 3x
/// reduction in FFT count, on top of removing the old per-call-site loop's
/// serial dependency (accumulating in coefficient form via `&agg_numerator
/// + &...` had to run one input at a time; the pointwise sum here has no
/// such constraint since only the per-input evaluate step, not the
/// accumulate step, does FFT work).
///
/// `m = 2 * domain_size` matches what `divide_by_vanishing_coset_cached`
/// would derive itself from the aggregated numerator's degree whenever
/// `transform` is degree <= 2 (true for both current call sites): every
/// input polynomial has degree <= domain_size - 1 (interpolated over a
/// domain_size-point domain), so a degree-2 `transform` produces degree <=
/// 2*(domain_size-1), and `domain_size` is always a power of two
/// (`Radix2EvaluationDomain` requirement, see this project's own arkworks
/// notes) -- so `next_power_of_two(2*(domain_size-1)+1) == 2*domain_size`
/// unconditionally. Coset-FFT interpolation recovers the unique
/// minimal-degree polynomial satisfying the quotient identity regardless of
/// which sufficiently-large `m` was used, so this does not need to match
/// the *exact* `m` the naive path would compute from the literal (possibly
/// lower-degree, e.g. if a high bit column happens to be identically zero)
/// summed numerator -- using a fixed, always-sufficient `m` here is still
/// guaranteed to produce the bit-identical result, which
/// `agg_bool_quotient_fast_matches_naive` and
/// `agg_bool_quotient_fast_matches_naive_random` (below) confirm
/// empirically (one fixed case, one property-tested across randomized
/// inputs) as well as this doc comment argues algebraically. A caller
/// using a higher-degree `transform` would need a correspondingly larger
/// `m` -- not needed by either current call site, so not parameterized
/// here; widen this signature if/when one is added.
///
/// Local reimplementation, not a wrapper around lib.rs's private
/// `CosetCache`, for the same "new parallel path, don't widen lib.rs's
/// public surface" reason `quotient_holds_at_local`/`scale_poly_local`
/// above are local reimplementations -- `CosetCache::get_or_build`/`ensure`/
/// `get` are private to lib.rs's module. Each current call site invokes
/// this once per gadget instance (6 times per pipeline run), so the
/// coset/`Z_H` setup this duplicates (`O(m)` field ops + one
/// `batch_inversion`) is negligible next to the FFTs it saves.
pub(crate) fn compute_rlc_quotient_via_coset<T>(
    polys:       &[&DensePolynomial<F>],
    weights:     &[F],
    domain_size: usize,
    transform:   T,
) -> DensePolynomial<F>
where
    T: Fn(F) -> F + Sync,
{
    assert_eq!(polys.len(), weights.len(), "one weight per polynomial");
    let m = 2 * domain_size;
    let coset = GeneralEvaluationDomain::<F>::new(m)
        .expect("coset domain creation failed")
        .get_coset(F::GENERATOR)
        .expect("coset construction failed");
    let mut zh_evals: Vec<F> = coset
        .elements()
        .map(|x| x.pow([domain_size as u64]) - F::one())
        .collect();
    batch_inversion(&mut zh_evals);

    let poly_evals: Vec<Vec<F>> = polys
        .par_iter()
        .map(|p| p.evaluate_over_domain_by_ref(coset).evals)
        .collect();

    let mut agg_evals = vec![F::zero(); coset.size()];
    for (evals, &w) in poly_evals.iter().zip(weights.iter()) {
        for (acc, &v) in agg_evals.iter_mut().zip(evals.iter()) {
            *acc += w * transform(v);
        }
    }

    let q_evals: Vec<F> = agg_evals
        .iter()
        .zip(zh_evals.iter())
        .map(|(&v, &zhi)| v * zhi)
        .collect();
    Evaluations::from_vec_and_domain(q_evals, coset).interpolate()
}

/// Fast path for building the RLC-aggregated booleanity quotient
/// `q_bool_agg` such that `q_bool_agg * Z_H == sum_j beta^j * (b_j^2 - b_j)`
/// -- thin wrapper around `compute_rlc_quotient_via_coset` with
/// `weights = [beta^0, beta^1, ..]` and `transform = |v| v*v - v`. See that
/// function's doc comment for the full algebraic argument and FFT-count
/// accounting.
fn compute_agg_bool_quotient_fast(
    cols:        &BitColumns,
    beta:        F,
    domain_size: usize,
) -> DensePolynomial<F> {
    let mut beta_pow = F::one();
    let weights: Vec<F> = cols
        .bits
        .iter()
        .map(|_| {
            let w = beta_pow;
            beta_pow *= beta;
            w
        })
        .collect();
    let polys: Vec<&DensePolynomial<F>> = cols.bits.iter().collect();
    compute_rlc_quotient_via_coset(&polys, &weights, domain_size, |v| v * v - v)
}

/// Parallel replacement for `crate::build_bit_columns` -- numerically
/// identical output (each bit column is an independent function of `values`
/// and `j` alone, so computing them out of order changes nothing), just
/// interpolated concurrently via rayon instead of `lib.rs`'s sequential
/// `.map()` over `0..BIT_WIDTH`.
///
/// This closes the actual gap `NEXT_OPTIMIZATION_HANDOFF.md` flagged and
/// left unexplained ("Layer 4b's build step only dropped ~1.4-1.5x despite
/// 32-way parallelism per instance"): the booleanity-quotient loop
/// (`build_bit_gadget`'s `q_bools`/this module's `q_bool_agg` computation)
/// was already parallel, but bit-*column* construction -- the BIT_WIDTH
/// `interpolate_slice` (FFT) calls that dominate an instance's build cost --
/// was not; it ran fully sequentially in both the original and (until this
/// change) the aggregated path. This is a pure build-order change with zero
/// effect on what's computed, so it carries no soundness implications and
/// needs no negative-control test -- `build_bit_columns_parallel_local_matches_sequential`
/// below is a correctness/equivalence check against the trusted sequential
/// original, not a forgery test.
fn build_bit_columns_parallel_local(
    values: &[u64],
    domain: &GeneralEvaluationDomain<F>,
) -> BitColumns {
    let bits: Vec<DensePolynomial<F>> = (0..BIT_WIDTH)
        .into_par_iter()
        .map(|j| {
            let col: Vec<F> = values.iter().map(|&v| F::from((v >> j) & 1)).collect();
            crate::interpolate_slice(&col, domain)
        })
        .collect();
    BitColumns { bits }
}

// ===========================================================================
// Build + commit (two-stage Fiat-Shamir: commit bits -> derive beta -> commit
// recon + aggregated-booleanity quotients)
// ===========================================================================

pub struct AggBitGadget {
    pub cols:        BitColumns,
    pub value:       DensePolynomial<F>,
    pub q_recon:     DensePolynomial<F>,
    pub q_bool_agg:  DensePolynomial<F>,
    pub beta:        F,
    pub recon_ok:    bool,
    pub bool_agg_ok: bool,
}

impl AggBitGadget {
    pub fn all_pass(&self) -> bool {
        self.recon_ok && self.bool_agg_ok
    }
}

pub struct AggBitGadgetCommitments {
    pub c_bits:     Vec<Commitment<Bn254>>,
    pub c_recon:    Commitment<Bn254>,
    pub c_bool_agg: Commitment<Bn254>,
}

impl AggBitGadgetCommitments {
    /// Fixed order: bit columns, then recon quotient, then aggregated
    /// booleanity quotient -- BIT_WIDTH + 2 = 34 commitments total, must
    /// stay in the same order `open_and_prove_aggregated` opens them in.
    pub fn all_affines(&self) -> Vec<G1Affine> {
        let mut out: Vec<G1Affine> = self.c_bits.iter().map(|c| c.0).collect();
        out.push(self.c_recon.0);
        out.push(self.c_bool_agg.0);
        out
    }

    pub fn all_commitments(&self) -> Vec<Commitment<Bn254>> {
        let mut out: Vec<Commitment<Bn254>> = self.c_bits.clone();
        out.push(self.c_recon.clone());
        out.push(self.c_bool_agg.clone());
        out
    }
}

pub struct AggBitGadgetRands {
    pub r_bits:     Vec<KzgRand>,
    pub r_recon:    KzgRand,
    pub r_bool_agg: KzgRand,
}

/// Build the aggregated quotients (recon + RLC booleanity) from an
/// already-constructed `BitColumns`, and commit everything. Split out from
/// `build_and_commit_agg_bit_gadget` so soundness tests can inject a
/// hand-forged (non-boolean) `BitColumns` directly -- see
/// `agg_bool_forgery_rejected_at_zeta`.
pub fn build_and_commit_agg_bit_gadget_from_cols<R: Rng>(
    scheme: &KzgScheme,
    cols:   BitColumns,
    value:  &DensePolynomial<F>,
    mask:   Option<&DensePolynomial<F>>,
    domain: &GeneralEvaluationDomain<F>,
    cache:  &mut CosetCache,
    rng:    &mut R,
) -> (AggBitGadget, AggBitGadgetCommitments, AggBitGadgetRands) {
    let domain_size = domain.size();
    assert_eq!(cols.bits.len(), BIT_WIDTH, "expected BIT_WIDTH bit columns");

    // Stage A: commit the BIT_WIDTH bit columns (unaggregated -- unchanged
    // from lib.rs's build_bit_gadget/commit_bit_gadget for this step).
    let bit_polys: Vec<&DensePolynomial<F>> = cols.bits.iter().collect();
    let (c_bits, r_bits): (Vec<_>, Vec<_>) = parallel_commit_many(scheme, &bit_polys, rng)
        .into_iter()
        .unzip();

    // Stage B: derive beta *after* the bit commitments exist (soundness-
    // critical -- see module doc comment).
    let mut ts = Transcript::new();
    let bit_affines: Vec<G1Affine> = c_bits.iter().map(|c| c.0).collect();
    ts.append_affines::<Bn254>(&bit_affines);
    let beta: F = ts.append_and_digest::<Bn254>("agg-bit-gadget-beta".to_string());

    // Stage C: reconstruction quotient -- unaggregated (there was only ever
    // one of these; nothing to aggregate here).
    let recon = bit_reconstruct(&cols);
    let diff = value - &recon;
    let gated = match mask {
        Some(m) => m * &diff,
        None => diff,
    };
    let q_recon = divide_by_vanishing_coset_cached(&gated, domain_size, cache);
    let recon_ok = quotient_holds_at_local(&gated, &q_recon, domain_size, F::GENERATOR);

    // Stage C': the RLC-aggregated booleanity quotient -- the actual change
    // versus lib.rs's build_bit_gadget (which commits BIT_WIDTH separate
    // quotients here instead of one). Uses `compute_agg_bool_quotient_fast`
    // (see its doc comment) instead of building `agg_numerator` via
    // BIT_WIDTH FFT-based `DensePolynomial` multiplications -- same
    // polynomial, ~3x fewer FFTs.
    let q_bool_agg = compute_agg_bool_quotient_fast(&cols, beta, domain_size);
    // Self-check tripwire (implementation-bug detector, NOT a soundness
    // mechanism -- see module doc comment / AGG_PIPELINE_HANDOFF.md #4.3):
    // recompute `agg_numerator(F::GENERATOR)` directly from the bit
    // columns' own coefficient-form `.evaluate()` (Horner, O(domain_size)
    // per column, independent of `compute_agg_bool_quotient_fast`'s coset
    // internals) and compare against `q_bool_agg(F::GENERATOR) *
    // Z_H(F::GENERATOR)` -- same check `quotient_holds_at_local` performed
    // against the old coefficient-form `agg_numerator`, just without ever
    // materializing that polynomial.
    let bool_agg_ok = {
        let zh_at_generator = F::GENERATOR.pow([domain_size as u64]) - F::one();
        let mut agg_num_at_gen = F::zero();
        let mut bp = F::one();
        for b in &cols.bits {
            let bv = b.evaluate(&F::GENERATOR);
            agg_num_at_gen += bp * (bv * bv - bv);
            bp *= beta;
        }
        let q_at_gen = q_bool_agg.evaluate(&F::GENERATOR);
        agg_num_at_gen == q_at_gen * zh_at_generator
    };

    let (c_recon, r_recon) = scheme.commit(&q_recon, rng);
    let (c_bool_agg, r_bool_agg) = scheme.commit(&q_bool_agg, rng);

    let gadget = AggBitGadget {
        cols,
        value: value.clone(),
        q_recon,
        q_bool_agg,
        beta,
        recon_ok,
        bool_agg_ok,
    };
    let comms = AggBitGadgetCommitments { c_bits, c_recon, c_bool_agg };
    let rands = AggBitGadgetRands { r_bits, r_recon, r_bool_agg };
    (gadget, comms, rands)
}

/// Thin wrapper: build `BitColumns` from raw `bit_values` (same input shape
/// as lib.rs's `build_bit_gadget`) and delegate to the `_from_cols` version.
pub fn build_and_commit_agg_bit_gadget<R: Rng>(
    scheme: &KzgScheme,
    bit_values: &[u64],
    value: &DensePolynomial<F>,
    mask: Option<&DensePolynomial<F>>,
    domain: &GeneralEvaluationDomain<F>,
    cache: &mut CosetCache,
    rng: &mut R,
) -> (AggBitGadget, AggBitGadgetCommitments, AggBitGadgetRands) {
    let cols = build_bit_columns_parallel_local(bit_values, domain);
    build_and_commit_agg_bit_gadget_from_cols(scheme, cols, value, mask, domain, cache, rng)
}

// ===========================================================================
// Open + verify (standalone -- not yet wired into lib.rs's real
// fiat_shamir_prove / compute_all_opening_proofs / full_pipeline)
// ===========================================================================

pub struct AggOpeningResult {
    pub proof:       BatchCheckProof<Bn254>,
    pub zeta:        F,
    pub beta:        F,
    pub domain_size: usize,
}

/// Opens all 34 of this instance's polynomials at a single Fiat-Shamir
/// point `zeta`, derived after all 34 commitments exist (same discipline as
/// `beta` above and as the real `zeta` in lib.rs's `fiat_shamir_prove`).
pub fn open_and_prove_aggregated<R: Rng>(
    scheme: &KzgScheme,
    gadget: &AggBitGadget,
    comms:  &AggBitGadgetCommitments,
    rands:  &AggBitGadgetRands,
    domain_size: usize,
    rng:    &mut R,
) -> AggOpeningResult {
    let mut ts = Transcript::new();
    ts.append_affines::<Bn254>(&comms.all_affines());
    let zeta: F = ts.append_and_digest::<Bn254>("agg-bit-gadget-zeta".to_string());

    let mut polys: Vec<&DensePolynomial<F>> = gadget.cols.bits.iter().collect();
    polys.push(&gadget.q_recon);
    polys.push(&gadget.q_bool_agg);

    let mut randoms: Vec<&KzgRand> = rands.r_bits.iter().collect();
    randoms.push(&rands.r_recon);
    randoms.push(&rands.r_bool_agg);

    let (witness, open_evals, gamma) =
        parallel_batch_open(&scheme.powers, &polys, &randoms, zeta, false, rng);

    let proof = BatchCheckProof {
        commitments: vec![comms.all_commitments()],
        witnesses:   vec![witness],
        points:      vec![zeta],
        open_evals:  vec![open_evals],
        gammas:      vec![gamma],
    };

    AggOpeningResult { proof, zeta, beta: gadget.beta, domain_size }
}

/// Three independent checks, all required:
///   1. `batch_check` -- the real pairing check that every opened
///      evaluation actually came from its committed polynomial (this is
///      what makes a commitment-swap attack fail, same mechanism as
///      lib.rs's Phase 3 / `verify_all_openings`).
///   2. The RLC identity `q_bool_agg(zeta)*Z_H(zeta) == sum_j beta^j *
///      (b_j(zeta)^2 - b_j(zeta))`, recomputed *only* from the
///      pairing-verified plain openings -- this is what makes the
///      aggregated quotient's commitment meaningful rather than
///      decorative (the analogue of lib.rs's alpha-folded residue check
///      for the main 14-constraint set).
///   3. The reconstruction identity `mask(zeta)*(value(zeta) -
///      sum_j 2^j*b_j(zeta)) == q_recon(zeta)*Z_H(zeta)`, recomputed the
///      same way (only from pairing-verified openings + the caller-
///      supplied `value_at_zeta`/`mask_at_zeta`, which the real pipeline
///      opens and pairing-checks separately -- see
///      `agg_pipeline.rs::agg_bit_gadget_residues`, whose `r_recon` this
///      mirrors). **Found missing here** while auditing negative-control
///      coverage for the hand-rolled arithmetization: this function
///      previously checked booleanity only, so a bit-decomposition that
///      doesn't reconstruct the claimed value (but is still individually
///      boolean) would pass. Not a bug in the benchmarked/real pipeline --
///      `agg_pipeline.rs`'s `agg_full_pipeline` always computes its own
///      `r_recon` and was never missing this check -- this function is a
///      standalone, lower-level helper used only by this module's own
///      unit tests (grep confirms no caller outside this file), but a
///      `pub fn verify_aggregated` that silently skipped a committed-to
///      constraint was exactly the kind of latent landmine worth closing.
pub fn verify_aggregated<R: Rng>(
    scheme:        &KzgScheme,
    result:        &AggOpeningResult,
    value_at_zeta: F,
    mask_at_zeta:  Option<F>,
    _rng:          &mut R,
) -> bool {
    let vk = scheme.vk.clone();
    let pairing_ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut local_rng = ark_std::test_rng();
        batch_check(&vk, &result.proof, &mut local_rng);
    }))
    .is_ok();
    if !pairing_ok {
        return false;
    }

    let evals = &result.proof.open_evals[0];
    if evals.len() != BIT_WIDTH + 2 {
        return false;
    }
    let bit_evals: Vec<F> = (0..BIT_WIDTH).map(|j| evals[j].into_plain_value().0).collect();
    let q_recon_eval    = evals[BIT_WIDTH].into_plain_value().0;
    let q_bool_agg_eval = evals[BIT_WIDTH + 1].into_plain_value().0;

    let mut agg_numerator_at_zeta = F::zero();
    let mut beta_pow = F::one();
    for &bj in &bit_evals {
        agg_numerator_at_zeta += beta_pow * (bj * bj - bj);
        beta_pow *= result.beta;
    }

    let zh_at_zeta = result.zeta.pow([result.domain_size as u64]) - F::one();
    let bool_ok = agg_numerator_at_zeta == q_bool_agg_eval * zh_at_zeta;

    let mut recon_at_zeta = F::zero();
    let mut weight = F::one();
    for &bj in &bit_evals {
        recon_at_zeta += weight * bj;
        weight += weight;
    }
    let diff = value_at_zeta - recon_at_zeta;
    let gated_at_zeta = match mask_at_zeta {
        Some(m) => m * diff,
        None    => diff,
    };
    let recon_ok = gated_at_zeta == q_recon_eval * zh_at_zeta;

    bool_ok && recon_ok
}

// ===========================================================================
// Step 2: all 6 gadget instances, bundled -- same instances/masks lib.rs's
// `build_bit_gadget_set` uses (#1 bid_range, #2 ask_range, #8 ceiling, #15
// surp_b_nn, #16 surp_a_nn, #23 delta_floor, the last one gated by mask_p).
// Brings the whole bit-gadget layer from 390 -> 204 committed/opened
// polynomials (6 * 34 instead of 6 * 65).
// ===========================================================================

pub struct AggBitGadgetSet {
    pub bid_range:   AggBitGadget, // #1
    pub ask_range:   AggBitGadget, // #2
    pub ceiling:     AggBitGadget, // #8
    pub surp_b_nn:   AggBitGadget, // #15
    pub surp_a_nn:   AggBitGadget, // #16
    pub delta_floor: AggBitGadget, // #23
}

impl AggBitGadgetSet {
    pub fn all_pass(&self) -> bool {
        self.bid_range.all_pass()
            && self.ask_range.all_pass()
            && self.ceiling.all_pass()
            && self.surp_b_nn.all_pass()
            && self.surp_a_nn.all_pass()
            && self.delta_floor.all_pass()
    }
}

pub struct AllAggBitGadgetCommitments {
    pub bid_range:   AggBitGadgetCommitments,
    pub ask_range:   AggBitGadgetCommitments,
    pub ceiling:     AggBitGadgetCommitments,
    pub surp_b_nn:   AggBitGadgetCommitments,
    pub surp_a_nn:   AggBitGadgetCommitments,
    pub delta_floor: AggBitGadgetCommitments,
}

impl AllAggBitGadgetCommitments {
    /// Fixed order: bid_range/ask_range/ceiling/surp_b_nn/surp_a_nn/
    /// delta_floor, each instance's own [bits.., recon, bool_agg] order --
    /// 6*34 = 204 commitments total. Must match
    /// `agg_bit_gadget_set_polys_and_rands`'s order exactly (index-matched
    /// by `batch_check`/`parallel_batch_open`).
    pub fn all_affines(&self) -> Vec<G1Affine> {
        let mut out = Vec::new();
        for c in [
            &self.bid_range, &self.ask_range, &self.ceiling,
            &self.surp_b_nn, &self.surp_a_nn, &self.delta_floor,
        ] {
            out.extend(c.all_affines());
        }
        out
    }

    pub fn all_commitments(&self) -> Vec<Commitment<Bn254>> {
        let mut out = Vec::new();
        for c in [
            &self.bid_range, &self.ask_range, &self.ceiling,
            &self.surp_b_nn, &self.surp_a_nn, &self.delta_floor,
        ] {
            out.extend(c.all_commitments());
        }
        out
    }
}

pub struct AllAggBitGadgetRands {
    pub bid_range:   AggBitGadgetRands,
    pub ask_range:   AggBitGadgetRands,
    pub ceiling:     AggBitGadgetRands,
    pub surp_b_nn:   AggBitGadgetRands,
    pub surp_a_nn:   AggBitGadgetRands,
    pub delta_floor: AggBitGadgetRands,
}

/// Build + commit all 6 instances, in the exact same per-instance
/// definitions (bits/value/mask) as lib.rs's `build_bit_gadget_set`, just
/// routed through the aggregated (Step 1) construction instead of the
/// original per-bit one. One `CosetCache` shared across all 6, same reuse
/// rationale as `build_bit_gadget_set`'s own cache.
pub fn build_and_commit_all_agg_bit_gadgets<R: Rng>(
    scheme: &KzgScheme,
    book:   &crate::OrderBook,
    polys:  &crate::Polynomials,
    mask_p: &DensePolynomial<F>,
    domain: &GeneralEvaluationDomain<F>,
    rng:    &mut R,
) -> (AggBitGadgetSet, AllAggBitGadgetCommitments, AllAggBitGadgetRands) {
    let mut cache = CosetCache::new();

    // #1/#2 Bid/Ask range check
    let bid_bits: Vec<u64> = book.b.iter().map(|&f| crate::to_u64(f)).collect();
    let (bid_range, c_bid, r_bid) = build_and_commit_agg_bit_gadget(
        scheme, &bid_bits, &polys.b, None, domain, &mut cache, rng,
    );
    let ask_bits: Vec<u64> = book.a.iter().map(|&f| crate::to_u64(f)).collect();
    let (ask_range, c_ask, r_ask) = build_and_commit_agg_bit_gadget(
        scheme, &ask_bits, &polys.a, None, domain, &mut cache, rng,
    );

    // #8 Ceiling: V_max - Min(X), zero-padded past tick n
    let ceiling_evals: Vec<F> = (0..book.n)
        .map(|i| F::from(book.v_max) - book.min_x[i])
        .collect();
    let ceiling_value = crate::interpolate_slice(&ceiling_evals, domain);
    let ceiling_bits: Vec<u64> = (0..book.n)
        .map(|i| book.v_max - crate::to_u64(book.min_x[i]))
        .collect();
    let (ceiling, c_ceil, r_ceil) = build_and_commit_agg_bit_gadget(
        scheme, &ceiling_bits, &ceiling_value, None, domain, &mut cache, rng,
    );

    // #15/#16 SurpB/SurpA non-negativity
    let surp_b_bits: Vec<u64> = book.surp_b.iter().map(|&f| crate::to_u64(f)).collect();
    let (surp_b_nn, c_sb, r_sb) = build_and_commit_agg_bit_gadget(
        scheme, &surp_b_bits, &polys.surp_b, None, domain, &mut cache, rng,
    );
    let surp_a_bits: Vec<u64> = book.surp_a.iter().map(|&f| crate::to_u64(f)).collect();
    let (surp_a_nn, c_sa, r_sa) = build_and_commit_agg_bit_gadget(
        scheme, &surp_a_bits, &polys.surp_a, None, domain, &mut cache, rng,
    );

    // #23 Delta floor, gated to the plateau by Mask_P
    let v_min_delta_poly = DensePolynomial { coeffs: vec![F::from(book.v_min_delta)] };
    let delta_floor_bits: Vec<u64> = (0..book.n)
        .map(|i| {
            if i >= book.c && i <= book.d {
                crate::to_u64(book.delta[i])
                    .checked_sub(book.v_min_delta)
                    .expect("Delta floor violated inside plateau")
            } else {
                0
            }
        })
        .collect();
    let delta_floor_value = &polys.delta - &v_min_delta_poly;
    let (delta_floor, c_df, r_df) = build_and_commit_agg_bit_gadget(
        scheme, &delta_floor_bits, &delta_floor_value, Some(mask_p), domain, &mut cache, rng,
    );

    let gadgets = AggBitGadgetSet { bid_range, ask_range, ceiling, surp_b_nn, surp_a_nn, delta_floor };
    let comms = AllAggBitGadgetCommitments {
        bid_range: c_bid, ask_range: c_ask, ceiling: c_ceil,
        surp_b_nn: c_sb, surp_a_nn: c_sa, delta_floor: c_df,
    };
    let rands = AllAggBitGadgetRands {
        bid_range: r_bid, ask_range: r_ask, ceiling: r_ceil,
        surp_b_nn: r_sb, surp_a_nn: r_sa, delta_floor: r_df,
    };
    (gadgets, comms, rands)
}

/// Flatten all 6 instances' polys/rands into the fixed order matching
/// `AllAggBitGadgetCommitments::all_commitments()`, for a single
/// `parallel_batch_open` call across the whole 204-polynomial group (same
/// role as lib.rs's private `bit_gadget_set_polys_and_rands`, reimplemented
/// here since this module cannot use that private helper).
pub fn agg_bit_gadget_set_polys_and_rands<'a>(
    gadgets: &'a AggBitGadgetSet,
    rands:   &'a AllAggBitGadgetRands,
) -> (Vec<&'a DensePolynomial<F>>, Vec<&'a KzgRand>) {
    let mut polys: Vec<&DensePolynomial<F>> = Vec::new();
    let mut rnds:  Vec<&KzgRand>             = Vec::new();
    for (g, r) in [
        (&gadgets.bid_range, &rands.bid_range),
        (&gadgets.ask_range, &rands.ask_range),
        (&gadgets.ceiling, &rands.ceiling),
        (&gadgets.surp_b_nn, &rands.surp_b_nn),
        (&gadgets.surp_a_nn, &rands.surp_a_nn),
        (&gadgets.delta_floor, &rands.delta_floor),
    ] {
        for b in &g.cols.bits {
            polys.push(b);
        }
        polys.push(&g.q_recon);
        polys.push(&g.q_bool_agg);
        for rb in &r.r_bits {
            rnds.push(rb);
        }
        rnds.push(&r.r_recon);
        rnds.push(&r.r_bool_agg);
    }
    (polys, rnds)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build_all_polys, build_bit_columns, build_domain, interpolate_slice, to_u64, OrderBook};
    use proptest::prelude::*;

    fn setup() -> (OrderBook, GeneralEvaluationDomain<F>, KzgScheme) {
        let book = OrderBook::hardcoded_21tick();
        let domain = build_domain(book.domain_size);
        let mut rng = ark_std::test_rng();
        let scheme = KzgScheme::new(book.domain_size - 1, &mut rng);
        (book, domain, scheme)
    }

    /// The naive (pre-optimization) construction of the RLC-aggregated
    /// booleanity quotient, in coefficient form: `BIT_WIDTH` FFT-based
    /// `b*(b-1)` multiplications, scaled and summed, then divided by `Z_H`
    /// via `divide_by_vanishing_coset_cached`. Shared ground truth for both
    /// `agg_bool_quotient_fast_matches_naive` (one fixed real-data case) and
    /// `agg_bool_quotient_fast_matches_naive_random` (property-tested across
    /// randomized inputs) below, so the two tests can never silently drift
    /// apart into checking different things.
    fn naive_agg_bool_quotient(
        cols:        &BitColumns,
        beta:        F,
        domain_size: usize,
        cache:       &mut CosetCache,
    ) -> DensePolynomial<F> {
        let one_poly = DensePolynomial { coeffs: vec![F::one()] };
        let mut agg_numerator = DensePolynomial { coeffs: vec![] };
        let mut beta_pow = F::one();
        for b in &cols.bits {
            let numerator_j = b * &(b - &one_poly);
            agg_numerator = &agg_numerator + &scale_poly_local(&numerator_j, beta_pow);
            beta_pow *= beta;
        }
        divide_by_vanishing_coset_cached(&agg_numerator, domain_size, cache)
    }

    /// Correctness/equivalence check (not a soundness test): the rayon-
    /// parallelized `build_bit_columns_parallel_local` must produce
    /// bit-for-bit identical polynomials to `lib.rs`'s trusted sequential
    /// `build_bit_columns` for the same inputs -- this is a build-order
    /// change only, so the two must never diverge.
    #[test]
    fn build_bit_columns_parallel_local_matches_sequential() {
        let (book, domain, _scheme) = setup();
        let bid_bits: Vec<u64> = book.b.iter().map(|&f| to_u64(f)).collect();
        let sequential = build_bit_columns(&bid_bits, &domain);
        let parallel = build_bit_columns_parallel_local(&bid_bits, &domain);
        assert_eq!(sequential.bits.len(), parallel.bits.len());
        for (s, p) in sequential.bits.iter().zip(parallel.bits.iter()) {
            assert_eq!(s.coeffs, p.coeffs, "parallel bit-column construction diverged from sequential");
        }
    }

    /// Correctness/equivalence check (not a soundness test):
    /// `compute_agg_bool_quotient_fast`'s coset-evaluation shortcut must
    /// produce bit-for-bit identical coefficients to the original
    /// coefficient-form construction (`BIT_WIDTH` FFT-based `b*(b-1)`
    /// multiplications, scaled and summed, then divided by `Z_H` via
    /// `divide_by_vanishing_coset_cached`) it replaced -- this is purely a
    /// "fewer FFTs to reach the same polynomial" change (see the function's
    /// doc comment for the algebraic argument: evaluation is a ring
    /// homomorphism, so `(b*(b-1))(x) == b(x)^2 - b(x)` pointwise), so the
    /// two must never diverge. Uses real bid-price data (100-tick CSV, a
    /// larger domain than the 21-tick fixture) so the two coset sizes
    /// (`domain_size` for the naive per-column products vs `2*domain_size`
    /// used directly here) genuinely exercise different-sized FFTs, not a
    /// degenerate small case.
    #[test]
    fn agg_bool_quotient_fast_matches_naive() {
        let book = OrderBook::from_csv(concat!(env!("CARGO_MANIFEST_DIR"), "/test_data/order_book_100_log-normal.csv"));
        let domain = build_domain(book.domain_size);
        let domain_size = domain.size();
        let mut cache = CosetCache::new();

        let bid_bits: Vec<u64> = book.b.iter().map(|&f| to_u64(f)).collect();
        let cols = build_bit_columns(&bid_bits, &domain);

        // Same beta a real run would use, just fixed here for reproducibility
        // -- the equivalence being tested holds for any beta, so the exact
        // value is not soundness-relevant.
        let beta = F::from(12345u64);

        // Naive path: exactly the code `compute_agg_bool_quotient_fast`
        // replaced (see git history / this module's prior version) --
        // reconstructed via the shared `naive_agg_bool_quotient` helper so
        // the equivalence has something concrete to check against.
        let naive = naive_agg_bool_quotient(&cols, beta, domain_size, &mut cache);

        let fast = compute_agg_bool_quotient_fast(&cols, beta, domain_size);

        assert_eq!(
            naive.coeffs, fast.coeffs,
            "compute_agg_bool_quotient_fast diverged from the naive per-column construction"
        );
    }

    // Same equivalence as `agg_bool_quotient_fast_matches_naive` above, but
    // property-tested across randomized `beta`, randomized bit-column
    // input values, and two different domain sizes, instead of one fixed
    // CSV + one fixed beta. This is the trust upgrade the primitive
    // extraction was for: `compute_rlc_quotient_via_coset`'s "evaluation is
    // a ring homomorphism" argument is now backed by many random cases
    // instead of a single example, and any future call site built on that
    // primitive (see its doc comment -- e.g. a future RLC-aggregated
    // reconstruction-quotient optimization) inherits this same evidence
    // rather than needing its own bespoke fuzz coverage. `values` is
    // truncated to `domain_size` to respect `interpolate_slice`'s
    // `vals.len() <= domain.size()` contract (see lib.rs); values shorter
    // than `domain_size` are zero-padded by that same contract, so both
    // under- and over-length raw inputs are exercised.
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]
        #[test]
        fn agg_bool_quotient_fast_matches_naive_random(
            beta_raw in any::<u64>(),
            use_small_domain in any::<bool>(),
            raw_values in prop::collection::vec(any::<u64>(), 1..40),
        ) {
            let domain_size = if use_small_domain { 32 } else { 256 };
            let domain = build_domain(domain_size);
            let mut cache = CosetCache::new();

            let mut values = raw_values;
            values.truncate(domain_size);
            let cols = build_bit_columns(&values, &domain);
            let beta = F::from(beta_raw);

            let naive = naive_agg_bool_quotient(&cols, beta, domain_size, &mut cache);
            let fast = compute_agg_bool_quotient_fast(&cols, beta, domain_size);

            prop_assert_eq!(
                naive.coeffs, fast.coeffs,
                "compute_agg_bool_quotient_fast diverged from naive on randomized input"
            );
        }
    }

    /// Honest path: real bid-price data, real proof, must verify.
    #[test]
    fn agg_bit_gadget_honest_path_verifies() {
        let (book, domain, scheme) = setup();
        let polys = build_all_polys(&book, &domain);
        let mut cache = CosetCache::new();
        let mut rng = ark_std::test_rng();

        let bid_bits: Vec<u64> = book.b.iter().map(|&f| to_u64(f)).collect();
        let (gadget, comms, rands) = build_and_commit_agg_bit_gadget(
            &scheme, &bid_bits, &polys.b, None, &domain, &mut cache, &mut rng,
        );
        assert!(gadget.all_pass(), "honest bid-range data must satisfy recon + aggregated booleanity");
        assert_eq!(comms.c_bits.len(), BIT_WIDTH, "expected BIT_WIDTH bit commitments");
        // The whole point of step 1: 34 polynomials instead of 65 for this instance.
        assert_eq!(comms.all_affines().len(), BIT_WIDTH + 2);

        let opening = open_and_prove_aggregated(&scheme, &gadget, &comms, &rands, domain.size(), &mut rng);
        let value_at_zeta = gadget.value.evaluate(&opening.zeta);
        assert!(
            verify_aggregated(&scheme, &opening, value_at_zeta, None, &mut rng),
            "honest aggregated bit-gadget proof failed to verify"
        );
    }

    /// Negative control #1 (commitment binding): swap one bit commitment
    /// for an unrelated-but-valid one (another instance's bit commitment)
    /// without changing the opened evaluation. Mirrors lib.rs's
    /// `bit_gadget_opening_rejects_swapped_commitment`. If `batch_check`
    /// were decorative here, this would still verify.
    #[test]
    fn agg_bit_gadget_rejects_swapped_commitment() {
        let (book, domain, scheme) = setup();
        let polys = build_all_polys(&book, &domain);
        let mut cache = CosetCache::new();
        let mut rng = ark_std::test_rng();

        let bid_bits: Vec<u64> = book.b.iter().map(|&f| to_u64(f)).collect();
        let (gadget, mut comms, rands) = build_and_commit_agg_bit_gadget(
            &scheme, &bid_bits, &polys.b, None, &domain, &mut cache, &mut rng,
        );

        // An unrelated-but-valid commitment: a second, independently-built
        // instance's first bit commitment (ask-range).
        let ask_bits: Vec<u64> = book.a.iter().map(|&f| to_u64(f)).collect();
        let (_ask_gadget, ask_comms, _ask_rands) = build_and_commit_agg_bit_gadget(
            &scheme, &ask_bits, &polys.a, None, &domain, &mut cache, &mut rng,
        );

        let honest_commitment = comms.c_bits[0].clone();
        comms.c_bits[0] = ask_comms.c_bits[0].clone();
        assert_ne!(
            honest_commitment, comms.c_bits[0],
            "test fixture bug: swapped-in commitment must differ from the honest one"
        );

        // Opening still uses the REAL (bid-range) polynomials/randomness via
        // `gadget`/`rands` -- only the commitment list (and therefore what
        // the pairing check verifies against) is tampered.
        let opening = open_and_prove_aggregated(&scheme, &gadget, &comms, &rands, domain.size(), &mut rng);
        let value_at_zeta = gadget.value.evaluate(&opening.zeta);
        assert!(
            !verify_aggregated(&scheme, &opening, value_at_zeta, None, &mut rng),
            "batch_check accepted a tampered bit commitment -- aggregation's commitment binding is not load-bearing"
        );
    }

    /// Negative control #2 (aggregation binding): forge a `BitColumns`
    /// whose first column takes the value 2 (not boolean) at one domain
    /// point, but commit and open it completely honestly (no commitment
    /// tampering at all -- `batch_check`'s pairing check will pass). This
    /// isolates whether the RLC identity check in `verify_aggregated`
    /// actually catches a non-boolean column, which is the entire point of
    /// the aggregation (as opposed to just checking that openings match
    /// commitments, already covered by the swap test above).
    #[test]
    fn agg_bool_forgery_rejected_at_zeta() {
        let (book, domain, _honest_scheme) = setup();
        let polys = build_all_polys(&book, &domain);
        let mut cache = CosetCache::new();
        let mut rng = ark_std::test_rng();

        // A forged (non-boolean) column makes the aggregated numerator not
        // exactly divisible by Z_H, so `divide_by_vanishing_coset_cached`'s
        // FFT-based "quotient" comes back at roughly double the honest
        // degree (it's evaluating over a larger coset to be safe, then
        // interpolating whatever garbage results, rather than raising an
        // error the way an exact-remainder division would). Committing that
        // garbage polynomial legitimately requires a bigger SRS than the
        // honest-path scheme provides -- use one sized for that here, so
        // this test isolates the algebraic identity check in
        // `verify_aggregated` (the actual target) rather than an unrelated
        // "SRS too small" panic on `commit`.
        let scheme = KzgScheme::new(4 * domain.size() - 1, &mut rng);

        let bid_bits: Vec<u64> = book.b.iter().map(|&f| to_u64(f)).collect();
        let mut cols = build_bit_columns(&bid_bits, &domain);

        // Forge bit column 0: set its value at domain point 0 to 2 instead
        // of whatever honest {0,1} value it held. `build_bit_gadget`-style
        // construction can never produce this (bit extraction `(v>>j)&1` is
        // always 0/1), so this bypasses `build_bit_columns` entirely to
        // simulate a dishonest prover directly forging a column.
        let mut col0_evals: Vec<F> = domain.elements().map(|x| cols.bits[0].evaluate(&x)).collect();
        col0_evals[0] = F::from(2u64);
        cols.bits[0] = interpolate_slice(&col0_evals, &domain);

        let (gadget, comms, rands) = build_and_commit_agg_bit_gadget_from_cols(
            &scheme, cols, &polys.b, None, &domain, &mut cache, &mut rng,
        );
        // NOTE: `gadget.bool_agg_ok` (the prover-side `quotient_holds_at`
        // spot check) is *not* a meaningful forgery detector and is
        // deliberately not asserted here. It's checked at `F::GENERATOR`,
        // which is also the shift used to build the coset that
        // `divide_by_vanishing_coset_cached` solves *by construction* --
        // so `q_bool_agg(F::GENERATOR) * Z_H(F::GENERATOR) ==
        // agg_numerator(F::GENERATOR)` holds trivially for any input,
        // forged or not (this is inherited, unmodified behavior: lib.rs's
        // own `build_bit_gadget` checks `recon_ok`/`bool_ok` the same way,
        // at the same point, for the same structural reason -- it's an
        // implementation-bug tripwire, not a soundness mechanism). The
        // actual soundness barrier is `verify_aggregated`'s identity check
        // at Fiat-Shamir's `zeta`, which is unpredictable to the prover at
        // forgery time and is *not* one of the coset points used to build
        // `q_bool_agg` -- that's what this test isolates below.
        //
        // Note: since `verify_aggregated` now also checks the reconstruction
        // identity (see its doc comment), forging bit 0 to a non-boolean
        // value incidentally breaks reconstruction too (the weighted sum no
        // longer equals `polys.b`) -- so this rejection is no longer purely
        // isolating the booleanity check the way it was when this test was
        // first written. That's fine: it doesn't weaken what this test
        // proves (a forged column is still correctly rejected), and
        // `agg_recon_forgery_rejected_at_zeta` below isolates the
        // reconstruction side cleanly (boolean-valid bits, broken sum).
        let opening = open_and_prove_aggregated(&scheme, &gadget, &comms, &rands, domain.size(), &mut rng);
        let value_at_zeta = gadget.value.evaluate(&opening.zeta);
        assert!(
            !verify_aggregated(&scheme, &opening, value_at_zeta, None, &mut rng),
            "verify_aggregated accepted a forged non-boolean bit column -- RLC aggregation is not load-bearing"
        );
    }

    /// Negative control #3 (reconstruction binding): decompose a value one
    /// bit *different* from the honestly-committed `polys.b` (flip bit 0 of
    /// the first bid), then commit and open completely honestly -- every
    /// bit column is still exactly boolean (0/1), so the RLC booleanity
    /// identity checked by `agg_bool_forgery_rejected_at_zeta` above passes
    /// trivially. This isolates the *other* half of the range-proof
    /// argument: does `q_recon` actually enforce
    /// `sum_j 2^j * B_j(X) == value(X)`? Before this test, only the
    /// booleanity half had adversarial coverage anywhere in this repo --
    /// neither this file nor lib.rs's `bit_gadget_opening_rejects_swapped_commitment`
    /// forges reconstruction. Found and closed this session while auditing
    /// negative-control coverage for the hand-rolled arithmetization --
    /// see AGG_PIPELINE_HANDOFF.md's trust-hardening follow-up.
    #[test]
    fn agg_recon_forgery_rejected_at_zeta() {
        let (book, domain, _honest_scheme) = setup();
        let polys = build_all_polys(&book, &domain);
        let mut cache = CosetCache::new();
        let mut rng = ark_std::test_rng();

        // Same reasoning as agg_bool_forgery_rejected_at_zeta: an inexact
        // Z_H division (here, because the forged decomposition no longer
        // reconstructs `value`) comes back from the safe-coset FFT at
        // roughly double the honest degree, so committing it legitimately
        // needs a bigger SRS than the honest-path scheme provides.
        let scheme = KzgScheme::new(4 * domain.size() - 1, &mut rng);

        // Forge: flip bit 0 of the first bid's decomposition. Still an
        // exactly-boolean BIT_WIDTH-bit decomposition -- just of a
        // different value than the honestly-committed `polys.b`, exactly
        // what a dishonest prover claiming a false range-checked value
        // would submit.
        let mut bid_bits: Vec<u64> = book.b.iter().map(|&f| to_u64(f)).collect();
        bid_bits[0] ^= 1;

        let (gadget, comms, rands) = build_and_commit_agg_bit_gadget(
            &scheme, &bid_bits, &polys.b, None, &domain, &mut cache, &mut rng,
        );
        let opening = open_and_prove_aggregated(&scheme, &gadget, &comms, &rands, domain.size(), &mut rng);
        // The honest value (polys.b) -- `gadget.value` was never forged,
        // only the bit decomposition was, so this is exactly what an
        // external verifier's own KZG-opened `polys.b` evaluation would be.
        let value_at_zeta = gadget.value.evaluate(&opening.zeta);
        assert!(
            !verify_aggregated(&scheme, &opening, value_at_zeta, None, &mut rng),
            "verify_aggregated accepted a bit-decomposition that doesn't reconstruct the claimed value -- q_recon is not load-bearing"
        );
    }
}
