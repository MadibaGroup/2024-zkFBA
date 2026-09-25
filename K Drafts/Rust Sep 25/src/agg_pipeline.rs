//! Step 3 of the bit-gadget aggregation redesign: a full, additive,
//! end-to-end pipeline that swaps `lib.rs`'s 390-polynomial bit-gadget layer
//! (6 instances x 65 polys: 32 bits + 1 recon quotient + 32 individual
//! booleanity quotients) for the RLC-aggregated 204-polynomial layer built
//! in `agg_bit_gadget.rs` (6 instances x 34 polys: 32 bits + 1 recon
//! quotient + 1 aggregated booleanity quotient), while reusing every other
//! part of `lib.rs`'s pipeline -- interpolation, the 14 main-constraint
//! quotients, KZG commit/open/verify, range proofs -- completely unchanged.
//! Nothing in `lib.rs` is modified for this file; see `agg_bit_gadget.rs`'s
//! module doc comment for the "new parallel path, original untouched"
//! layout and the RLC soundness argument this file depends on.
//!
//! ## Why this file recomputes residues itself instead of calling
//! `fiat_shamir_prove` / reusing `PipelineResult`
//!
//! `lib.rs::fiat_shamir_prove` and `full_pipeline`'s `bit_gadgets_ok` take a
//! `&BitGadgetSet`/`&AllBitGadgetCommitments` (the *original* 390-poly
//! types) as parameters -- they cannot be called against the new
//! `AggBitGadgetSet`/`AllAggBitGadgetCommitments` types without modifying
//! `lib.rs`, which the chosen layout forbids. So this file reimplements the
//! same *style* of check, parametrized on the 204-poly design, at the same
//! rigor level `full_pipeline` already uses (not weaker, not stronger):
//!   - `batch_ok`: alpha-folded algebraic residue check over the 14 main
//!     constraints (verbatim copy of `fiat_shamir_prove`'s Step 3 math,
//!     since that part of the protocol is completely untouched by this
//!     optimization) plus, per bit-gadget instance, exactly 2 residues
//!     (1 reconstruction + 1 aggregated-booleanity, mirroring
//!     `agg_bit_gadget::verify_aggregated`'s identity) instead of the
//!     original's 33 (1 + 32) -- 12 total instead of 198, since that's the
//!     literal mathematical content of the RLC aggregation.
//!   - `opening_ok`: real `batch_check` (pairing-verified) opening of all
//!     227 committed polynomials (23 main/quotient + 204 aggregated
//!     bit-gadget, vs the original's 23 + 390 = 413) -- this is the
//!     genuine cryptographic guarantee, identical in kind to the original
//!     pipeline's `opening_ok`, just over fewer polynomials.
//!   - `range_ok` / `constraint_result` / `quotient_check`: reused
//!     completely unchanged (`verify_all`, `compute_quotients`,
//!     `prove_range16`/`verify_range16_bound`/`prove_cliff_slack`), since
//!     none of that logic depends on the bit-gadget internals at all.
//!
//! Reducing the bit-gadget layer from 390 to 204 committed/opened
//! polynomials directly targets the two costs `RESULTS.md` and
//! `NEXT_OPTIMIZATION_HANDOFF.md` identify as dominant: Layer 4b
//! (build+commit, ~119ms on real data, by far the single biggest line
//! item) and Layer 3f (`batch_open`/`parallel_batch_open`, proportional to
//! polynomial count), and it also shrinks proof size, since `BatchCheckProof`
//! serializes one G1 commitment *per polynomial* (only the KZG witness is
//! aggregated per group, not the commitments) -- so fewer bit-gadget
//! commitments means a smaller proof, not just a faster prover.

use std::time::Instant;

use ark_bn254::{Bn254, Fr as F};
use ark_ff::{Field, One, Zero};
use ark_poly::{univariate::DensePolynomial, EvaluationDomain, GeneralEvaluationDomain, Polynomial};
use ark_std::rand::Rng;
use gadgets::utils::{batch_open, BatchCheckProof, Transcript};

use crate::{
    build_all_polys, build_domain, commit_all, commit_quotients, compute_quotients, mask_p_poly,
    mask_v_poly, prove_cliff_slack, prove_range16, sd_poly, verify_all, verify_all_openings,
    verify_range16_bound, Commitments, ConstraintResult, KzgScheme, OpeningsResult, OrderBook,
    Polynomials, QuotientCheck, QuotientCommitments, QuotientPolynomials,
};

use crate::agg_bit_gadget::{
    agg_bit_gadget_set_polys_and_rands, build_and_commit_all_agg_bit_gadgets, AggBitGadget,
    AggBitGadgetSet, AllAggBitGadgetCommitments, AllAggBitGadgetRands,
};

pub struct AggPipelineResult {
    pub constraint_result: ConstraintResult, // Layer 4 sanity dup, unchanged from lib.rs
    pub quotient_check:    QuotientCheck,    // prover-side per-point check, unchanged from lib.rs
    pub batch_ok:          bool, // 14 main + 12 agg-bit-gadget = 26 residues (vs 14 + 198 = 212 originally)
    pub opening_ok:        bool, // batch_check over 227 committed polys (vs 413 originally)
    pub range_ok:          bool,
    pub gadgets_self_check: bool, // recon_ok/bool_agg_ok, same role as original's bit_gadgets_ok
    pub v_max:                 u64,
    pub n:                     usize,
    pub domain_size:           usize,
    pub num_bit_gadget_polys:      usize, // 204
    pub num_total_committed_polys: usize, // 23 + 204 = 227
    // Real proof-size element counts (G1 points, Fr elements), same
    // accounting method as main.rs's own `count()`: commitments + 1
    // witness/group for G1, 2 Fr/opened-eval + 1 gamma/group for Fr.
    // Covers the KZG opening proof (compute_all_opening_proofs analogue)
    // plus the V_max/slack range proofs, exactly like main.rs's printed
    // "PROOF SIZE" section, so this is directly comparable to the
    // already-published 42,464-byte figure for the original N=100 pipeline.
    pub proof_g1_elements: usize,
    pub proof_fr_elements: usize,
    // Isolated timing, added to answer AGG_PIPELINE_HANDOFF.md's open item
    // 7.1 ("measure verify time for the new pipeline separately"). Pure
    // Instant::now()/elapsed() wrapping around the existing, unmodified
    // verify_all_openings call below -- no reordering, no logic change.
    pub opening_verify_ms: f64,
    // Additional per-layer timing, all pure Instant::now()/elapsed() wraps
    // around existing, unmodified calls -- diagnostic only, added to find
    // the next latency bottleneck (handoff doc's next-steps item), no
    // reordering or logic change versus the un-instrumented version.
    pub build_ms:            f64, // build_domain + build_all_polys + mask_p_poly
    pub commit_main_ms:      f64, // commit_all (9 main polys)
    pub quotients_ms:        f64, // compute_quotients + commit_quotients (14 quotients)
    pub verify_all_ms:       f64, // verify_all (prover-side sanity dup, Layer 4)
    pub bitgadget_build_commit_ms: f64, // build_and_commit_all_agg_bit_gadgets (204 polys)
    pub transcript_residues_ms:    f64, // zeta/alpha derivation + all residues
    pub open_main_ms:         f64, // batch_open for the 23 main/quotient polys (+shifted)
    pub open_bitgadget_ms:    f64, // parallel_batch_open for the 204 bit-gadget polys
    pub range_proofs_ms:      f64, // prove_range16/prove_cliff_slack + their verifies
}

impl AggPipelineResult {
    pub fn proof_bytes(&self) -> usize {
        (self.proof_g1_elements + self.proof_fr_elements) * 32
    }
}

impl AggPipelineResult {
    pub fn all_pass(&self) -> bool {
        self.constraint_result.all_pass()
            && self.quotient_check.all_zero()
            && self.batch_ok
            && self.opening_ok
            && self.range_ok
            && self.gadgets_self_check
    }
}

/// Per-instance RLC residues at `zeta`: (r_recon, r_bool_agg). Mirrors
/// `lib.rs`'s private `bit_gadget_residues_and_evals`, but for the
/// aggregated design -- 2 residues instead of 1 + BIT_WIDTH, and the
/// booleanity residue is the single RLC identity
/// `q_bool_agg(zeta)*Z_H(zeta) == sum_j beta^j * (b_j(zeta)^2 - b_j(zeta))`
/// instead of BIT_WIDTH separate ones.
fn agg_bit_gadget_residues(
    gadget:       &AggBitGadget,
    mask_at_zeta: Option<F>,
    zeta:         F,
    zh_at_zeta:   F,
) -> (F, F, Vec<F>) {
    let bits_at_zeta: Vec<F> = gadget.cols.bits.iter().map(|b| b.evaluate(&zeta)).collect();

    let mut recon_at_zeta = F::zero();
    let mut weight = F::one();
    for &bv in &bits_at_zeta {
        recon_at_zeta += weight * bv;
        weight += weight;
    }
    let value_at_zeta = gadget.value.evaluate(&zeta);
    let diff = value_at_zeta - recon_at_zeta;
    let gated_at_zeta = match mask_at_zeta {
        Some(m) => m * diff,
        None    => diff,
    };
    let q_recon_at_zeta = gadget.q_recon.evaluate(&zeta);
    let r_recon = gated_at_zeta - q_recon_at_zeta * zh_at_zeta;

    let mut agg_numerator_at_zeta = F::zero();
    let mut beta_pow = F::one();
    for &bv in &bits_at_zeta {
        agg_numerator_at_zeta += beta_pow * (bv * bv - bv);
        beta_pow *= gadget.beta;
    }
    let q_bool_agg_at_zeta = gadget.q_bool_agg.evaluate(&zeta);
    let r_bool_agg = agg_numerator_at_zeta - q_bool_agg_at_zeta * zh_at_zeta;

    // Raw evals to absorb into the transcript before deriving alpha (same
    // "absorb everything the challenge depends on" discipline as lib.rs's
    // Step 4): 32 bits + 1 recon quotient + 1 aggregated quotient = 34.
    let mut evals = Vec::with_capacity(34);
    evals.extend_from_slice(&bits_at_zeta);
    evals.push(q_recon_at_zeta);
    evals.push(q_bool_agg_at_zeta);

    (r_recon, r_bool_agg, evals)
}

pub fn agg_full_pipeline<R: Rng>(book: &OrderBook, rng: &mut R) -> AggPipelineResult {
    let n           = book.n;
    let domain_size = book.domain_size;

    let t0 = Instant::now();
    let domain: GeneralEvaluationDomain<F> = build_domain(domain_size);
    let polys: Polynomials  = build_all_polys(book, &domain);
    let mask_p      = mask_p_poly(book, &domain);
    let scheme      = KzgScheme::new(domain_size - 1, rng);
    let build_ms = t0.elapsed().as_secs_f64() * 1000.0;

    let t1 = Instant::now();
    let (wcomms, w_rands): (Commitments, Vec<_>) = commit_all(&scheme, &polys, rng);
    let commit_main_ms = t1.elapsed().as_secs_f64() * 1000.0;

    let t2 = Instant::now();
    let (quots, q_check): (QuotientPolynomials, QuotientCheck) =
        compute_quotients(book, &polys, &mask_p, &domain, n);
    let (qcomms, q_rands): (QuotientCommitments, Vec<_>) = commit_quotients(&scheme, &quots, rng);
    let quotients_ms = t2.elapsed().as_secs_f64() * 1000.0;

    let t3 = Instant::now();
    let cresult: ConstraintResult = verify_all(book, &polys, &mask_p, &domain, n);
    let verify_all_ms = t3.elapsed().as_secs_f64() * 1000.0;

    // Aggregated bit-gadget layer: 6 instances x 34 polys = 204, committed
    // *before* the Fiat-Shamir challenge (same "commit before challenge"
    // discipline as the original), so `zeta` binds them too.
    let t4 = Instant::now();
    let (agg_gadgets, agg_bgcomms, agg_bg_rands): (
        AggBitGadgetSet,
        AllAggBitGadgetCommitments,
        AllAggBitGadgetRands,
    ) = build_and_commit_all_agg_bit_gadgets(&scheme, book, &polys, &mask_p, &domain, rng);
    let bitgadget_build_commit_ms = t4.elapsed().as_secs_f64() * 1000.0;

    let t5 = Instant::now();

    // ---- Fiat-Shamir: derive zeta from the 23 main/quotient commitments
    // plus the 204 aggregated bit-gadget commitments (verbatim analogue of
    // fiat_shamir_prove's Step 1, just with the new 204-commitment set).
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
    absorbed.extend(agg_bgcomms.all_affines());
    transcript.append_affines::<Bn254>(&absorbed);
    let zeta = transcript.append_and_digest::<Bn254>("zeta".to_string());

    let omega       = domain.group_gen();
    let omega_zeta  = omega * zeta;

    // ---- Step 2/3: evaluate + compute the 14 main-constraint residues.
    // Verbatim copy of fiat_shamir_prove's math -- this part of the
    // protocol is completely untouched by the bit-gadget optimization.
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
    let mask_v         = mask_v_poly(book, &domain);
    let sd             = sd_poly(book, &domain);
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

    let r1 = (acc_a_at_zeta - a_at_zeta) - q_acc_a_init_at_zeta * (zeta - omega_0);
    let v2 = (zeta - omega_last_data) * (acc_a_at_omega_zeta - acc_a_at_zeta - a_at_omega_zeta);
    let r2 = v2 - q_acc_a_rec_at_zeta * zh_at_zeta;
    let r3 = (acc_b_at_zeta - b_at_zeta) - q_acc_b_init_at_zeta * (zeta - omega_last_data);
    let v4 = (zeta - omega_last_dom) * (acc_b_at_zeta - acc_b_at_omega_zeta - b_at_zeta);
    let r4 = v4 - q_acc_b_rec_at_zeta * zh_at_zeta;
    let v5 = (acc_a_at_zeta - min_x_at_zeta) * (acc_b_at_zeta - min_x_at_zeta);
    let r5 = v5 - q_kl_at_zeta * zh_at_zeta;
    let v6 = delta_at_zeta - (surp_a_at_zeta + surp_b_at_zeta);
    let r6 = v6 - q_delta_def_at_zeta * zh_at_zeta;
    let v7 = chk_d_at_zeta * (chk_d_at_zeta - F::one());
    let r7 = v7 - q_chkd_bool_at_zeta * zh_at_zeta;
    let v8 = (delta_at_zeta - v_min_delta) * chk_d_at_zeta;
    let r8 = v8 - q_chkd_correct_at_zeta * zh_at_zeta;
    let v9 = chk_d_at_zeta * (F::one() - mask_p_at_zeta);
    let r9 = v9 - q_chkd_contain_at_zeta * zh_at_zeta;
    let v10 = min_x_at_zeta - v_max;
    let r10 = v10 - q_plateau_left_at_zeta * (zeta - elems[book.c]);
    let r11 = v10 - q_plateau_right_at_zeta * (zeta - elems[book.d]);
    let v12 = delta_at_zeta - v_min_delta;
    let r12 = v12 - q_valley_pin_at_zeta * (zeta - elems[book.p_star]);
    let v13 = mask_v_at_zeta * (F::one() - chk_d_at_zeta);
    let r13 = v13 - q_mask_v_contain_at_zeta * zh_at_zeta;
    let v14 = sd_at_zeta - mask_v_at_zeta * delta_at_zeta;
    let r14 = v14 - q_sd_def_at_zeta * zh_at_zeta;

    // ---- Aggregated bit-gadget residues: 2 per instance x 6 = 12 (vs 33 x
    // 6 = 198 originally), same instances/masks as build_and_commit_all_agg_bit_gadgets.
    let (r_bid_recon,  r_bid_bool,  e_bid)  = agg_bit_gadget_residues(&agg_gadgets.bid_range,   None,                 zeta, zh_at_zeta);
    let (r_ask_recon,  r_ask_bool,  e_ask)  = agg_bit_gadget_residues(&agg_gadgets.ask_range,   None,                 zeta, zh_at_zeta);
    let (r_ceil_recon, r_ceil_bool, e_ceil) = agg_bit_gadget_residues(&agg_gadgets.ceiling,     None,                 zeta, zh_at_zeta);
    let (r_sb_recon,   r_sb_bool,   e_sb)   = agg_bit_gadget_residues(&agg_gadgets.surp_b_nn,   None,                 zeta, zh_at_zeta);
    let (r_sa_recon,   r_sa_bool,   e_sa)   = agg_bit_gadget_residues(&agg_gadgets.surp_a_nn,   None,                 zeta, zh_at_zeta);
    let (r_df_recon,   r_df_bool,   e_df)   = agg_bit_gadget_residues(&agg_gadgets.delta_floor, Some(mask_p_at_zeta), zeta, zh_at_zeta);

    let mut r = vec![r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14];
    r.extend([
        r_bid_recon, r_bid_bool, r_ask_recon, r_ask_bool, r_ceil_recon, r_ceil_bool,
        r_sb_recon, r_sb_bool, r_sa_recon, r_sa_bool, r_df_recon, r_df_bool,
    ]);

    // ---- Absorb evaluations -> alpha (Step 4 analogue).
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
    for val_group in [&e_bid, &e_ask, &e_ceil, &e_sb, &e_sa, &e_df] {
        for val in val_group {
            transcript.append_message(format!("{:?}", val));
        }
    }
    let alpha = transcript.append_and_digest::<Bn254>("alpha".to_string());

    let mut alpha_pow = F::one();
    let mut batched   = F::zero();
    for ri in &r {
        batched   += alpha_pow * ri;
        alpha_pow *= alpha;
    }
    let batch_ok = batched.is_zero();
    let transcript_residues_ms = t5.elapsed().as_secs_f64() * 1000.0;

    // ---- Phase 3: open everything at zeta (+3 shifted, +cliff) and
    // pairing-verify. Mechanical mirror of compute_all_opening_proofs,
    // Group 2 replaced by the 204-poly aggregated set.
    let t6 = Instant::now();
    let polys_z: Vec<&DensePolynomial<F>> = vec![
        &polys.b, &polys.a, &polys.acc_b, &polys.acc_a, &polys.min_x,
        &polys.surp_b, &polys.surp_a, &polys.delta, &polys.chk_d,
        &quots.q_acc_a_init, &quots.q_acc_a_rec, &quots.q_acc_b_init, &quots.q_acc_b_rec,
        &quots.q_kl, &quots.q_delta_def, &quots.q_chkd_bool, &quots.q_chkd_correct,
        &quots.q_chkd_contain, &quots.q_plateau_left, &quots.q_plateau_right, &quots.q_valley_pin,
        &quots.q_mask_v_contain, &quots.q_sd_def,
    ];
    let rands_z: Vec<&crate::KzgRand> = vec![
        &w_rands[0], &w_rands[1], &w_rands[2], &w_rands[3], &w_rands[4],
        &w_rands[5], &w_rands[6], &w_rands[7], &w_rands[8],
        &q_rands[0], &q_rands[1], &q_rands[2], &q_rands[3], &q_rands[4], &q_rands[5],
        &q_rands[6], &q_rands[7], &q_rands[8], &q_rands[9], &q_rands[10], &q_rands[11],
        &q_rands[12], &q_rands[13],
    ];
    let (w_zeta, evals_z, gamma_z) =
        batch_open(&scheme.powers, &polys_z, &rands_z, zeta, false, rng);

    let polys_wz: Vec<&DensePolynomial<F>> = vec![&polys.acc_a, &polys.a, &polys.acc_b];
    let rands_wz: Vec<&crate::KzgRand> = vec![&w_rands[3], &w_rands[1], &w_rands[2]];
    let (w_omega_zeta, evals_wz, gamma_wz) =
        batch_open(&scheme.powers, &polys_wz, &rands_wz, omega_zeta, false, rng);
    let open_main_ms = t6.elapsed().as_secs_f64() * 1000.0;

    let t7 = Instant::now();
    let (bg_polys, bg_rands_refs) = agg_bit_gadget_set_polys_and_rands(&agg_gadgets, &agg_bg_rands);
    let num_bit_gadget_polys = bg_polys.len();
    let (w_bg, evals_bg, gamma_bg) =
        crate::parallel_batch_open(&scheme.powers, &bg_polys, &bg_rands_refs, zeta, false, rng);
    let open_bitgadget_ms = t7.elapsed().as_secs_f64() * 1000.0;

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

    commitments.push(agg_bgcomms.all_commitments());
    witnesses.push(w_bg);
    points.push(zeta);
    open_evals.push(evals_bg);
    gammas.push(gamma_bg);

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

    let opr = OpeningsResult {
        proof: BatchCheckProof { commitments, witnesses, points, open_evals, gammas },
        min_at_c_minus_1,
        min_at_d_plus_1,
    };
    let verify_t0 = Instant::now();
    let opening_ok = verify_all_openings(&scheme, &opr, rng);
    let opening_verify_ms = verify_t0.elapsed().as_secs_f64() * 1000.0;

    // ---- Range proofs, unchanged from full_pipeline.
    let t8 = Instant::now();
    let v_max_proof = prove_range16(&scheme, book.v_max, rng);
    let v_max_ok    = verify_range16_bound(&scheme, &v_max_proof, book.v_max, rng);
    let (slack_l_proof, slack_r_proof) =
        prove_cliff_slack(&scheme, book, opr.min_at_c_minus_1, opr.min_at_d_plus_1, rng);
    let slack_l_ok = slack_l_proof.as_ref()
        .map(|p| verify_range16_bound(&scheme, p, book.slack_l, rng))
        .unwrap_or(true);
    let slack_r_ok = slack_r_proof.as_ref()
        .map(|p| verify_range16_bound(&scheme, p, book.slack_r, rng))
        .unwrap_or(true);
    let range_ok = v_max_ok && slack_l_ok && slack_r_ok;
    let range_proofs_ms = t8.elapsed().as_secs_f64() * 1000.0;

    let gadgets_self_check = agg_gadgets.all_pass();

    let num_total_committed_polys = 23 + num_bit_gadget_polys;

    // ---- Real proof-size accounting, same method as main.rs's `count()`:
    // G1 = sum(commitments per group) + 1 witness/group; Fr = 2*sum(open
    // evals per group) + 1 gamma/group. Covers the opening proof (opr) plus
    // all applicable range proofs, exactly like main.rs's printed
    // "PROOF SIZE" section.
    fn count_proof(p: &BatchCheckProof<Bn254>) -> (usize, usize) {
        let g1 = p.commitments.iter().map(|v| v.len()).sum::<usize>() + p.witnesses.len();
        let fr = p.open_evals.iter().map(|v| v.len()).sum::<usize>() * 2 + p.gammas.len();
        (g1, fr)
    }
    let (mut proof_g1, mut proof_fr) = count_proof(&opr.proof);
    let (g1r, frr) = count_proof(&v_max_proof);
    proof_g1 += g1r; proof_fr += frr;
    if let Some(p) = &slack_l_proof { let (a, b) = count_proof(p); proof_g1 += a; proof_fr += b; }
    if let Some(p) = &slack_r_proof { let (a, b) = count_proof(p); proof_g1 += a; proof_fr += b; }

    AggPipelineResult {
        constraint_result: cresult,
        quotient_check:    q_check,
        batch_ok,
        opening_ok,
        range_ok,
        gadgets_self_check,
        v_max: book.v_max,
        n,
        domain_size,
        num_bit_gadget_polys,
        num_total_committed_polys,
        proof_g1_elements: proof_g1,
        proof_fr_elements: proof_fr,
        opening_verify_ms,
        build_ms,
        commit_main_ms,
        quotients_ms,
        verify_all_ms,
        bitgadget_build_commit_ms,
        transcript_residues_ms,
        open_main_ms,
        open_bitgadget_ms,
        range_proofs_ms,
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agg_full_pipeline_honest_path_21tick() {
        let book = OrderBook::hardcoded_21tick();
        let mut rng = ark_std::test_rng();
        let result = agg_full_pipeline(&book, &mut rng);
        assert!(result.constraint_result.all_pass(), "constraint_result failed");
        assert!(result.quotient_check.all_zero(), "quotient_check failed");
        assert!(result.batch_ok, "alpha-folded residue check failed");
        assert!(result.opening_ok, "batch_check (pairing) failed");
        assert!(result.range_ok, "range proofs failed");
        assert!(result.gadgets_self_check, "bit-gadget self-check failed");
        assert!(result.all_pass(), "AggPipelineResult::all_pass() failed");
        assert_eq!(result.num_bit_gadget_polys, 204);
        assert_eq!(result.num_total_committed_polys, 227);
    }

    #[test]
    fn agg_full_pipeline_honest_path_100tick_csv() {
        let book = OrderBook::from_csv(concat!(env!("CARGO_MANIFEST_DIR"), "/test_data/order_book_100_log-normal.csv"));
        let mut rng = ark_std::test_rng();
        let result = agg_full_pipeline(&book, &mut rng);
        assert!(result.all_pass(), "AggPipelineResult::all_pass() failed on 100-tick CSV");
        assert_eq!(result.num_bit_gadget_polys, 204);
    }

    #[test]
    fn agg_full_pipeline_honest_path_1000tick_csv() {
        let book = OrderBook::from_csv(concat!(env!("CARGO_MANIFEST_DIR"), "/test_data/order_book_1000_log-normal.csv"));
        let mut rng = ark_std::test_rng();
        let result = agg_full_pipeline(&book, &mut rng);
        assert!(result.all_pass(), "AggPipelineResult::all_pass() failed on 1000-tick CSV");
        assert_eq!(result.num_bit_gadget_polys, 204);
    }

    /// Confirms the CURRENT RLC-aggregated pipeline (not just the
    /// pre-aggregation baseline validated separately in
    /// ~/zk_fba_real_data/RESULTS.md on 2026-08-26) also passes on real
    /// market data. These two CSVs are real AAPL mbp-10 quote/trade data
    /// from Databento, mapped into the same 20-column order-book format via
    /// ~/zk_fba_real_data/data/build_real_csv.py -- not synthetic
    /// log-normal data. Same N=100 as the synthetic 100-tick case above, so
    /// this isolates "does real data work" from "does N=100 work" (already
    /// covered by agg_full_pipeline_honest_path_100tick_csv).
    #[test]
    fn agg_full_pipeline_honest_path_real_quotes_100tick_csv() {
        let book = OrderBook::from_csv(concat!(env!("CARGO_MANIFEST_DIR"), "/test_data/order_book_real_quotes_100.csv"));
        let mut rng = ark_std::test_rng();
        let result = agg_full_pipeline(&book, &mut rng);
        assert!(result.all_pass(), "AggPipelineResult::all_pass() failed on real-quotes CSV");
        assert_eq!(result.num_bit_gadget_polys, 204);
    }

    #[test]
    fn agg_full_pipeline_honest_path_real_trades_100tick_csv() {
        let book = OrderBook::from_csv(concat!(env!("CARGO_MANIFEST_DIR"), "/test_data/order_book_real_trades_100.csv"));
        let mut rng = ark_std::test_rng();
        let result = agg_full_pipeline(&book, &mut rng);
        assert!(result.all_pass(), "AggPipelineResult::all_pass() failed on real-trades CSV");
        assert_eq!(result.num_bit_gadget_polys, 204);
    }

    /// End-to-end negative control: swap one bit-gadget commitment for an
    /// unrelated (but validly-formed) one from a *different instance* right
    /// before opening, without touching the opened evaluation. Mirrors
    /// `agg_bit_gadget::agg_bit_gadget_rejects_swapped_commitment` and
    /// lib.rs's own `bit_gadget_opening_rejects_swapped_commitment`, but at
    /// the full-pipeline level (through `agg_full_pipeline`'s own
    /// `OpeningsResult`/`verify_all_openings` call, not the component-level
    /// helper) -- confirms the real pairing check, not just the redundant
    /// plaintext self-check, is what's actually catching a forged proof.
    #[test]
    fn agg_full_pipeline_rejects_swapped_bit_gadget_commitment() {
        let book = OrderBook::hardcoded_21tick();
        let mut rng = ark_std::test_rng();
        let domain = build_domain(book.domain_size);
        let polys = build_all_polys(&book, &domain);
        let mask_p = mask_p_poly(&book, &domain);
        let scheme = KzgScheme::new(book.domain_size - 1, &mut rng);

        let (wcomms, w_rands) = commit_all(&scheme, &polys, &mut rng);
        let (quots, _q_check) = compute_quotients(&book, &polys, &mask_p, &domain, book.n);
        let (qcomms, q_rands) = commit_quotients(&scheme, &quots, &mut rng);

        let (agg_gadgets, mut agg_bgcomms, agg_bg_rands) =
            build_and_commit_all_agg_bit_gadgets(&scheme, &book, &polys, &mask_p, &domain, &mut rng);

        // Tamper: swap bid_range's first bit commitment for ask_range's
        // first bit commitment. Both are validly-formed KZG commitments to
        // real polynomials -- just not the one whose evaluation was opened.
        let swapped = agg_bgcomms.ask_range.c_bits[0].clone();
        agg_bgcomms.bid_range.c_bits[0] = swapped;

        // Re-derive zeta the same way agg_full_pipeline does, over the now-
        // tampered commitment set (an honest re-run of the same protocol
        // steps, just with a forged commitment substituted in).
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
        absorbed.extend(agg_bgcomms.all_affines());
        transcript.append_affines::<Bn254>(&absorbed);
        let zeta = transcript.append_and_digest::<Bn254>("zeta".to_string());

        let (bg_polys, bg_rands_refs) =
            agg_bit_gadget_set_polys_and_rands(&agg_gadgets, &agg_bg_rands);
        let (w_bg, evals_bg, gamma_bg) =
            crate::parallel_batch_open(&scheme.powers, &bg_polys, &bg_rands_refs, zeta, false, &mut rng);

        let opr = OpeningsResult {
            proof: BatchCheckProof {
                commitments: vec![agg_bgcomms.all_commitments()],
                witnesses:   vec![w_bg],
                points:      vec![zeta],
                open_evals:  vec![evals_bg],
                gammas:      vec![gamma_bg],
            },
            min_at_c_minus_1: None,
            min_at_d_plus_1:  None,
        };
        let ok = verify_all_openings(&scheme, &opr, &mut rng);
        assert!(!ok, "batch_check must reject a swapped bit-gadget commitment");
    }
}
