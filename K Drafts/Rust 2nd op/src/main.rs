use std::time::Instant;
use ark_ff::Zero;
use ark_serialize::CanonicalSerialize;
use zk_fba_csv_full::{
    build_all_polys, build_domain, commit_all, commit_quotients,
    compute_quotients, compute_all_opening_proofs,
    fiat_shamir_prove, mask_p_poly, verify_all, verify_all_openings,
    prove_range16, verify_range16_bound, prove_cliff_slack, to_u64,
    build_bit_gadget_set, commit_all_bit_gadgets,
    KzgScheme, OrderBook,
};

fn hex8(g1: &ark_bn254::G1Affine) -> String {
    let mut b = Vec::new();
    g1.serialize_compressed(&mut b).unwrap();
    b.iter().take(4).map(|x| format!("{:02x}", x)).collect()
}

// ============================================================================
// Run the full pipeline for one order book and print per-layer timing.
// Returns (total_ms, v_max, all_pass).
// ============================================================================
fn run_book(label: &str, book: &OrderBook, rng: &mut impl ark_std::rand::Rng)
    -> (f64, u64, bool)
{
    let sep = "=".repeat(70);
    let n           = book.n;
    let domain_size = book.domain_size;

    println!("\n{sep}");
    println!("  ZK-FBA CSV | {label}  |  n={n}  domain={}  V_max={}", domain_size, book.v_max);
    println!("{sep}");

    // -- Layer 1: print first 5 and last 5 rows --------------------------------
    println!("\n--- LAYER 1  Order Book (n={n}, showing first/last 5 rows) ---");
    println!(" {:>6}  {:>8}  {:>8}  {:>10}  {:>10}  {:>10}",
        "Price", "B(X)", "A(X)", "AccB(X)", "AccA(X)", "Min(X)");
    println!("{}", "-".repeat(60));
    let show: Vec<usize> = if n <= 10 {
        (0..n).collect()
    } else {
        let mut v: Vec<usize> = (0..5).collect();
        v.push(usize::MAX); // sentinel for "..."
        v.extend(n-5..n);
        v
    };
    for &i in &show {
        if i == usize::MAX { println!("  ..."); continue; }
        println!(" {:>6}  {:>8}  {:>8}  {:>10}  {:>10}  {:>10}",
            book.prices[i],
            to_u64(book.b[i]),
            to_u64(book.a[i]),
            to_u64(book.acc_b[i]),
            to_u64(book.acc_a[i]),
            to_u64(book.min_x[i]));
    }
    println!("{}", "-".repeat(60));
    println!("  V_max = {}  (global max of Min(X))", book.v_max);
    println!("  Plateau [c, d] = [{}, {}]   V_min_delta = {}   p* = {}",
        book.c, book.d, book.v_min_delta, book.p_star);
    println!("  Cliff:  left={} (slack_l={})   right={} (slack_r={})",
        book.has_left_cliff, book.slack_l, book.has_right_cliff, book.slack_r);

    // -- Layer 2 ---------------------------------------------------------------
    println!("\n--- LAYER 2  Polynomial Interpolation ---");
    let t = Instant::now();
    let domain = build_domain(domain_size);
    let polys  = build_all_polys(book, &domain);
    let mask_p = mask_p_poly(book, &domain);
    let l2 = t.elapsed();
    println!("  domain |H| = {domain_size}  (log2={}),  9 witness polys + Mask_P   time: {:.3?}",
        domain_size.trailing_zeros(), l2);

    // -- Layer 3a --------------------------------------------------------------
    println!("\n--- LAYER 3a  KZG Trusted Setup ---");
    let t = Instant::now();
    let scheme = KzgScheme::new(domain_size - 1, rng);
    let l3a = t.elapsed();
    println!("  SRS: {} G1 points  (degree {})   time: {:.3?}",
        scheme.powers.powers_of_g.len(), domain_size - 1, l3a);

    // -- Layer 3b --------------------------------------------------------------
    println!("\n--- LAYER 3b  Witness Commitments (9 MSM) ---");
    let t = Instant::now();
    let (wcomms, w_rands) = commit_all(&scheme, &polys, rng);
    let l3b = t.elapsed();
    println!("  C_B   {}...  C_A   {}...", hex8(&wcomms.c_b.0), hex8(&wcomms.c_a.0));
    println!("  C_AccB {}...  C_AccA {}...  C_Min {}...",
        hex8(&wcomms.c_acc_b.0), hex8(&wcomms.c_acc_a.0), hex8(&wcomms.c_min_x.0));
    println!("  time: {:.3?}", l3b);

    // -- Layer 3c --------------------------------------------------------------
    println!("\n--- LAYER 3c  Quotient Polynomials ---");
    let t = Instant::now();
    let (quots, q_check) = compute_quotients(book, &polys, &mask_p, &domain, n);
    let l3c = t.elapsed();
    println!("  max quotient degree: {}   all_zero: {}   time: {:.3?}",
        quots.max_degree(),
        if q_check.all_zero() { "PASS" } else { "FAIL !!!" },
        l3c);

    // -- Layer 3d --------------------------------------------------------------
    println!("\n--- LAYER 3d  Quotient Commitments (14 MSM) ---");
    let t = Instant::now();
    let (qcomms, q_rands) = commit_quotients(&scheme, &quots, rng);
    let l3d = t.elapsed();
    println!("  C_Q_AccA_init {}...  C_Q_AccA_rec {}...",
        hex8(&qcomms.c_q_acc_a_init.0), hex8(&qcomms.c_q_acc_a_rec.0));
    println!("  time: {:.3?}", l3d);

    // -- Layer 4b (build + commit happen here, before Fiat-Shamir, so their
    // commitments are absorbed into the challenge -- see AllBitGadgetCommitments
    // in lib.rs. Printed later, in its usual "Layer 4b" position below.) -----
    let t = Instant::now();
    let bit_gadgets = build_bit_gadget_set(book, &polys, &mask_p, &domain);
    let l4b_build = t.elapsed();
    let t = Instant::now();
    let (bg_comms, bg_rands) = commit_all_bit_gadgets(&scheme, &bit_gadgets, rng);
    let l4b_commit = t.elapsed();

    // -- Layer 3e --------------------------------------------------------------
    println!("\n--- LAYER 3e  Fiat-Shamir ---");
    let t = Instant::now();
    let fs = fiat_shamir_prove(book, &polys, &mask_p, &quots, &wcomms, &qcomms, &bit_gadgets, &bg_comms, &domain, n);
    let l3e = t.elapsed();
    println!("  batch_ok (Σ α^i·r_i = 0): {}   time: {:.3?}",
        if fs.batch_ok { "PASS" } else { "FAIL !!!" }, l3e);
    for (i, ri) in fs.r.iter().take(14).enumerate() {
        println!("    r[{}] = {}   {}", i+1, to_u64(*ri),
            if ri.is_zero() { "PASS" } else { "FAIL !!!" });
    }
    let bit_residues_ok = fs.r.iter().skip(14).all(|ri| ri.is_zero());
    println!("    bit-gadget residues r[15..{}] (198, #1/#2/#8/#15/#16/#23): {}",
        fs.r.len(), if bit_residues_ok { "PASS" } else { "FAIL !!!" });

    // -- Layer 3f --------------------------------------------------------------
    println!("\n--- LAYER 3f  KZG Batch Opening (GWC19, 4 pairings) ---");
    let t = Instant::now();
    let opr = compute_all_opening_proofs(
        &scheme, book, &polys, &quots, &wcomms, &qcomms,
        &w_rands, &q_rands, &fs, &domain,
        &bit_gadgets, &bg_comms, &bg_rands, rng,
    );
    let l3f_prove = t.elapsed();
    let t = Instant::now();
    let opening_ok = verify_all_openings(&scheme, &opr, rng);
    let l3f_verify = t.elapsed();
    println!("  openings: {}   prove: {:.3?}   verify: {:.3?}",
        if opening_ok { "PASS" } else { "FAIL !!!" }, l3f_prove, l3f_verify);

    // -- Layer 3g --------------------------------------------------------------
    println!("\n--- LAYER 3g  V_max Range Proof (V_max < 2^16) ---");
    let t = Instant::now();
    let range_proof = prove_range16(&scheme, book.v_max, rng);
    let l3g_prove = t.elapsed();
    let t = Instant::now();
    let v_max_ok = verify_range16_bound(&scheme, &range_proof, book.v_max, rng);
    let l3g_verify = t.elapsed();
    println!("  V_max={} < 65536, bound to disclosed V_max: {}   prove: {:.3?}   verify: {:.3?}",
        book.v_max,
        if v_max_ok { "PASS" } else { "FAIL !!!" }, l3g_prove, l3g_verify);

    // Negative control: a range proof for an unrelated value must fail the
    // binding check even though the internal range algebra is unaffected.
    let forged_proof = prove_range16(&scheme, book.v_max.wrapping_add(1) % (1u64 << 16), rng);
    let forged_ok = verify_range16_bound(&scheme, &forged_proof, book.v_max, rng);
    println!("  negative control (proof for V_max+1 checked against V_max): {}",
        if !forged_ok { "correctly FAILED" } else { "UNSOUND -- PASSED !!!" });

    // -- Layer 3h ----------------------------------------------------------
    println!("\n--- LAYER 3h  Cliff Slack Range Proofs (#31/#32) ---");
    let t = Instant::now();
    let (slack_l_proof, slack_r_proof) =
        prove_cliff_slack(&scheme, book, opr.min_at_c_minus_1, opr.min_at_d_plus_1, rng);
    let l3h_prove = t.elapsed();
    let t = Instant::now();
    let slack_l_ok = slack_l_proof.as_ref()
        .map(|p| verify_range16_bound(&scheme, p, book.slack_l, rng))
        .unwrap_or(true);
    let slack_r_ok = slack_r_proof.as_ref()
        .map(|p| verify_range16_bound(&scheme, p, book.slack_r, rng))
        .unwrap_or(true);
    let l3h_verify = t.elapsed();
    println!("  left cliff (slack_l={}): {}   right cliff (slack_r={}): {}   prove: {:.3?}   verify: {:.3?}",
        book.slack_l, if slack_l_ok { "PASS" } else { "FAIL !!!" },
        book.slack_r, if slack_r_ok { "PASS" } else { "FAIL !!!" },
        l3h_prove, l3h_verify);
    let range_ok = v_max_ok && slack_l_ok && slack_r_ok;

    // -- Proof-size accounting (real element counts via the actual proof
    // structs, not an estimate) -------------------------------------------
    {
        fn count(p: &gadgets::utils::BatchCheckProof<ark_bn254::Bn254>) -> (usize, usize) {
            let g1 = p.commitments.iter().map(|v| v.len()).sum::<usize>() + p.witnesses.len();
            let fr = p.open_evals.iter().map(|v| v.len()).sum::<usize>() * 2 + p.gammas.len();
            (g1, fr)
        }
        let (mut g1, mut fr) = count(&opr.proof);
        let (g1r, frr) = count(&range_proof);
        g1 += g1r; fr += frr;
        if let Some(p) = &slack_l_proof { let (a, b) = count(p); g1 += a; fr += b; }
        if let Some(p) = &slack_r_proof { let (a, b) = count(p); g1 += a; fr += b; }
        let bytes = (g1 + fr) * 32;
        println!("\n--- PROOF SIZE (real element count, excludes public opening points) ---");
        println!("  G1 points: {}   Fr elements: {}   total elements: {}   bytes (32B each): {}",
            g1, fr, g1 + fr, bytes);
    }

    // -- Layer 4 ---------------------------------------------------------------
    println!("\n--- LAYER 4  Algebraic Constraint Verification ---");
    let t = Instant::now();
    let cr = verify_all(book, &polys, &mask_p, &domain, n);
    let l4 = t.elapsed();
    println!("  #3  AccB init        {}   #5  AccB rec         {}",
        if cr.acc_b_init { "PASS" } else { "FAIL" },
        if cr.acc_b_recurrence { "PASS" } else { "FAIL" });
    println!("  #4  AccA init        {}   #6  AccA rec         {}",
        if cr.acc_a_init { "PASS" } else { "FAIL" },
        if cr.acc_a_recurrence { "PASS" } else { "FAIL" });
    println!("  #7  Min excl.        {}   #17 Delta def        {}",
        if cr.min_exclusivity { "PASS" } else { "FAIL" },
        if cr.delta_definition { "PASS" } else { "FAIL" });
    println!("  #18 ChkD boolean     {}   #19 ChkD correct     {}",
        if cr.chkd_booleanity { "PASS" } else { "FAIL" },
        if cr.chkd_correctness { "PASS" } else { "FAIL" });
    println!("  #20 ChkD contain     {}   #11 Plateau left     {}",
        if cr.chkd_containment { "PASS" } else { "FAIL" },
        if cr.plateau_left { "PASS" } else { "FAIL" });
    println!("  #12 Plateau right    {}   #24 Valley pin       {}   time: {:.3?}",
        if cr.plateau_right { "PASS" } else { "FAIL" },
        if cr.valley_pin { "PASS" } else { "FAIL" }, l4);
    println!("  #13 InMCV inside     {}   #14 InMCV outside    {}",
        if cr.inmcv_inside { "PASS" } else { "FAIL" },
        if cr.inmcv_outside { "PASS" } else { "FAIL" });
    println!("  #21 MaskV boolean    {}   #22 MaskV contain    {}",
        if cr.mask_v_booleanity { "PASS" } else { "FAIL" },
        if cr.mask_v_containment { "PASS" } else { "FAIL" });
    println!("  #25 SD definition    {}   #26 SD membership    {}",
        if cr.sd_definition { "PASS" } else { "FAIL" },
        if cr.sd_membership { "PASS" } else { "FAIL" });
    println!("  #27 MaskC boolean    {}   #28 MaskC contain    {}",
        if cr.mask_c_booleanity { "PASS" } else { "FAIL" },
        if cr.mask_c_containment { "PASS" } else { "FAIL" });
    println!("  #9  MaskP boolean    {}",
        if cr.mask_p_booleanity { "PASS" } else { "FAIL" });

    let p_star_in_plateau = book.c <= book.p_star && book.p_star <= book.d;
    println!("  p* containment (public-side sanity check, distinct from #22): {}",
        if p_star_in_plateau { "PASS" } else { "FAIL" });

    // -- Layer 4b (printed here; built/committed earlier, before Layer 3e, so
    // the commitments could be absorbed into the Fiat-Shamir challenge) ----
    println!("\n--- LAYER 4b  Bit-Decomposition Gadget (#1, #2, #8, #15, #16, #23) ---");
    println!("  #1  Bid range check        {}   #2  Ask range check  {}",
        if bit_gadgets.bid_range.all_pass() { "PASS" } else { "FAIL !!!" },
        if bit_gadgets.ask_range.all_pass() { "PASS" } else { "FAIL !!!" });
    println!("  #8  Ceiling (V_max - Min)  {}   #15 SurpB non-neg    {}",
        if bit_gadgets.ceiling.all_pass() { "PASS" } else { "FAIL !!!" },
        if bit_gadgets.surp_b_nn.all_pass() { "PASS" } else { "FAIL !!!" });
    println!("  #16 SurpA non-neg          {}   #23 Delta floor      {}   build: {:.3?}  commit: {:.3?}",
        if bit_gadgets.surp_a_nn.all_pass() { "PASS" } else { "FAIL !!!" },
        if bit_gadgets.delta_floor.all_pass() { "PASS" } else { "FAIL !!!" },
        l4b_build, l4b_commit);
    let bit_gadgets_ok = bit_gadgets.all_pass();

    let all_pass = cr.all_pass() && q_check.all_zero() && fs.batch_ok
                && opening_ok && range_ok && p_star_in_plateau && bit_gadgets_ok;

    // -- Summary ---------------------------------------------------------------
    let total = l2 + l3a + l3b + l3c + l3d + l3e + l3f_prove + l3f_verify
              + l3g_prove + l3g_verify + l3h_prove + l3h_verify + l4
              + l4b_build + l4b_commit;
    let total_ms = total.as_secs_f64() * 1000.0;

    println!("\n=== Timing Summary  [{label}] ===");
    println!("  Layer 2  Interpolation (9 IFFT)          {:.3?}", l2);
    println!("  Layer 3a KZG setup                       {:.3?}", l3a);
    println!("  Layer 3b Commit witnesses (9 MSM)        {:.3?}", l3b);
    println!("  Layer 3c Quotient polynomials            {:.3?}", l3c);
    println!("  Layer 3d Commit quotients (14 MSM)       {:.3?}", l3d);
    println!("  Layer 3e Fiat-Shamir                     {:.3?}", l3e);
    println!("  Layer 3f batch_open (prover)             {:.3?}", l3f_prove);
    println!("  Layer 3f batch_check (4 pairings)        {:.3?}", l3f_verify);
    println!("  Layer 3g V_max range::prove               {:.3?}", l3g_prove);
    println!("  Layer 3g V_max range::verify               {:.3?}", l3g_verify);
    println!("  Layer 3h cliff slack range::prove         {:.3?}", l3h_prove);
    println!("  Layer 3h cliff slack range::verify        {:.3?}", l3h_verify);
    println!("  Layer 4  Constraint verify               {:.3?}", l4);
    println!("  Layer 4b Bit-decomp gadgets (build)       {:.3?}", l4b_build);
    println!("  Layer 4b Bit-decomp gadgets (commit)      {:.3?}", l4b_commit);
    println!("  -------------------------------------------------------");
    println!("  TOTAL                                    {:.1} ms", total_ms);
    println!("  ALL PASS: {}", if all_pass { "YES \u{2713}" } else { "NO \u{2717}" });

    (total_ms, book.v_max, all_pass)
}

fn main() {
    let mut rng = ark_std::test_rng();

    // ── Run 1: 21-tick hardcoded baseline ───────────────────────────────────
    let book21 = OrderBook::hardcoded_21tick();
    let (t21, v_max21, ok21) = run_book("21-tick hardcoded baseline", &book21, &mut rng);

    // ── Run 2: 100-tick CSV ─────────────────────────────────────────────────
    let csv100 = "/Users/kimiaesmaili/Downloads/order_book_100_log-normal.csv";
    let book100 = OrderBook::from_csv(csv100);
    let (t100, v_max100, ok100) = run_book("100-tick log-normal CSV", &book100, &mut rng);

    // ── Run 3: 1000-tick CSV ────────────────────────────────────────────────
    let csv1000 = "/Users/kimiaesmaili/Downloads/order_book_1000_log-normal.csv";
    let book1000 = OrderBook::from_csv(csv1000);
    let (t1000, v_max1000, ok1000) = run_book("1000-tick log-normal CSV", &book1000, &mut rng);

    // ── Comparison table ─────────────────────────────────────────────────────
    println!("\n\n{}", "=".repeat(70));
    println!("  CROSS-SIZE COMPARISON");
    println!("{}", "=".repeat(70));
    println!(
        "  {:<32}  {:>6}  {:>10}  {:>9}  {:>8}",
        "Dataset", "n", "domain_size", "V_max", "Total ms"
    );
    println!("  {}", "-".repeat(66));
    println!(
        "  {:<32}  {:>6}  {:>10}  {:>9}  {:>8.1}  {}",
        "21-tick hardcoded baseline", 21, 32, v_max21, t21,
        if ok21 { "PASS" } else { "FAIL" }
    );
    println!(
        "  {:<32}  {:>6}  {:>10}  {:>9}  {:>8.1}  {}",
        "100-tick log-normal CSV", 100, 128, v_max100, t100,
        if ok100 { "PASS" } else { "FAIL" }
    );
    println!(
        "  {:<32}  {:>6}  {:>10}  {:>9}  {:>8.1}  {}",
        "1000-tick log-normal CSV", 1000, 1024, v_max1000, t1000,
        if ok1000 { "PASS" } else { "FAIL" }
    );
    println!("  {}", "-".repeat(66));

    // Scaling factors
    if t21 > 0.0 {
        println!(
            "\n  Scaling (relative to 21-tick baseline):
    100-tick  : {:.1}x slower   (domain {}x larger: 128 vs 32)
    1000-tick : {:.1}x slower   (domain {}x larger: 1024 vs 32)",
            t100 / t21,   128 / 32,
            t1000 / t21, 1024 / 32
        );
    }

    println!("\n  Notes:");
    println!("  - This binary currently exercises 32/33 constraints (#1-33 except #10,");
    println!("    Mask_P position via shuffle/permutation -- N/A here since Mask_P is");
    println!("    kept public/verifier-computable, per the doc's own Section 6/11");
    println!("    footnote, rather than committed). See protocol_constraints.md numbering.");
    println!("  - KZG verifier cost is O(1): always 4 pairings per opening group.");
    println!("  - Prover MSMs scale with domain_size.");
    println!("{}", "=".repeat(70));
}
