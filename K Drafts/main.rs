//K-FBA: Main binary -- full five-layer pipeline with per-phase timing.
//Usage:  cargo run --release

use ark_ff::Zero;
use ark_poly::{EvaluationDomain, Polynomial};
use ark_serialize::CanonicalSerialize;
use std::time::{Duration, Instant};
use zk_fba::{
    build_all_polys, build_domain, commit_all, commit_quotients,
    compute_all_arrays, compute_all_opening_proofs, compute_quotients,
    fiat_shamir_prove, random_tau, to_u64, verify_all, verify_all_openings,
    KZGSetup, ASKS_RAW, BIDS_RAW, DOMAIN_SIZE, N, PRICES,
};

//------------------------------------------------------
//Formatting helpers

fn sep(c: char, n: usize) -> String { std::iter::repeat(c).take(n).collect() }

fn commit_hex<S: CanonicalSerialize>(pt: &S) -> String {
    let mut b = Vec::new();
    pt.serialize_compressed(&mut b).unwrap();
    let p: String = b[..8].iter().map(|x| format!("{x:02x}")).collect();
    format!("{p}... ({} B)", b.len())
}

fn field_hex<S: CanonicalSerialize>(f: &S) -> String {
    let mut b = Vec::new();
    f.serialize_compressed(&mut b).unwrap();
    // field elements are stored little-endian; display big-endian for readability
    let p: String = b.iter().rev().take(8).map(|x| format!("{x:02x}")).collect();
    format!("{p}...")
}

fn ok(b: bool) -> &'static str { if b { "PASS" } else { "FAIL" } }

fn dur(d: Duration) -> String {
    if      d.as_secs()  > 0  { format!("{:.3} s",  d.as_secs_f64()) }
    else if d.as_millis()> 0  { format!("{:.3} ms", d.as_secs_f64() * 1e3) }
    else if d.as_micros()> 0  { format!("{:.2} us", d.as_secs_f64() * 1e6) }
    else                      { format!("{} ns",    d.as_nanos()) }
}

const W: usize = 70;

//------------------------------------------------------
//Main

fn main() {
    let t_total = Instant::now();

    println!("\n{}", sep('=', W));
    println!("  ZK-FBA | KZG Commitments + Quotient Polynomials | BN254");
    println!("{}", sep('=', W));

    //------------------------------------------------------
    //Layer 1: Arrays
    let t = Instant::now();
    let (bids, asks, bid_depth, ask_depth, min_arr) = compute_all_arrays();
    let d1 = t.elapsed();

    println!("\n{} LAYER 1  Array Computation {}", sep('-',3), sep('-',W-31));
    println!("{:>6}  {:>8}  {:>8}  {:>10}  {:>10}  {:>10}",
        "Price","Bids++","Asks++","Bid Depth","Ask Depth","Min(MCV)");
    println!("{}", sep('-', W));
    for i in 0..N {
        println!("{:>6}  {:>8}  {:>8}  {:>10}  {:>10}  {:>10}",
            PRICES[i], BIDS_RAW[i], ASKS_RAW[i],
            to_u64(bid_depth[i]), to_u64(ask_depth[i]), to_u64(min_arr[i]));
    }
    println!("{}", sep('-', W));
    let mcv = to_u64(*min_arr.iter().max_by_key(|&&x| to_u64(x)).unwrap());
    println!("  MCV = {mcv}  (global max of Min array)       time:  {}", dur(d1));

    //------------------------------------------------------
    //Layer 2: polynomials
    let t = Instant::now();
    let domain = build_domain();
    let polys  = build_all_polys(&bids, &asks, &bid_depth, &ask_depth, &min_arr, &domain);
    let d2 = t.elapsed();

    println!("\n{} LAYER 2  Polynomial Interpolation {}", sep('-',3), sep('-',W-38));
    println!("  Domain |H| = {}  (2^5),  degree <= {},  5 polynomials   time:  {}",
        domain.size(), domain.size()-1, dur(d2));

    //------------------------------------------------------
    //Layer 3a: KZG setup
    let t = Instant::now();
    let mut rng = ark_std::test_rng();
    let tau   = random_tau(&mut rng);
    let setup = KZGSetup::new(DOMAIN_SIZE - 1, tau);
    let d3a = t.elapsed();

    //------------------------------------------------------
    //Layer 3b: Commit witnesses
    let t = Instant::now();
    let wcomms = commit_all(&setup, &polys);
    let d3b = t.elapsed();

    println!("\n{} LAYER 3a/b  KZG Setup & Witness Commits {}", sep('-',3), sep('-',W-44));
    println!("  SRS: {} G1 points  [G1,tau*G1,...,tau^31*G1]   setup time:  {}",
        setup.srs_g1.len(), dur(d3a));
    println!("  C_Bid        {}", commit_hex(&wcomms.c_bid));
    println!("  C_Ask        {}", commit_hex(&wcomms.c_ask));
    println!("  C_Acc_B      {}", commit_hex(&wcomms.c_acc_b));
    println!("  C_Acc_A      {}", commit_hex(&wcomms.c_acc_a));
    println!("  C_Min        {}  commit time:  {}", commit_hex(&wcomms.c_min), dur(d3b));

    //------------------------------------------------------
    //Layer 3c: quotient polynomials
    let t = Instant::now();
    let (quots, q_check) = compute_quotients(&polys, &domain);
    let d3c = t.elapsed();

    println!("\n{} LAYER 3c  Quotient Polynomials {}", sep('-',3), sep('-',W-35));
    println!("  Each constraint V(X)=0 on H  ->  Q(X)=V(X)/D(X),  remainder must be 0.");
    println!("  Zero remainder == the constraint holds at every evaluation point.");
    println!();
    println!("  {:<15}  {:<12}  {:>6}  {:>8}  {:>6}",
        "Constraint", "Divisor D(X)", "deg(Q)", "Remainder", "Check");
    println!("  {}", sep('-', 58));
    println!("  {:<15}  {:<12}  {:>6}  {:>8}  {}",
        "V_AccA_init", "(X-w^0)",
        quots.q_acc_a_init.degree(),
        "0 (scalar)", ok(q_check.r_acc_a_init));
    println!("  {:<15}  {:<12}  {:>6}  {:>8}  {}",
        "V_AccA_rec", "Z_H=X^32-1",
        quots.q_acc_a_rec.degree(),
        "0 (poly)", ok(q_check.r_acc_a_rec));
    println!("  {:<15}  {:<12}  {:>6}  {:>8}  {}",
        "V_AccB_init", "(X-w^20)",
        quots.q_acc_b_init.degree(),
        "0 (scalar)", ok(q_check.r_acc_b_init));
    println!("  {:<15}  {:<12}  {:>6}  {:>8}  {}",
        "V_AccB_rec", "Z_H=X^32-1",
        quots.q_acc_b_rec.degree(),
        "0 (poly)", ok(q_check.r_acc_b_rec));
    println!("  {:<15}  {:<12}  {:>6}  {:>8}  {}",
        "V_KL (min)", "Z_H=X^32-1",
        quots.q_kl.degree(),
        "0 (poly)", ok(q_check.r_kl));
    println!("  {}", sep('-', 58));
    println!("  ALL REMAINDERS ZERO: {}              time:  {}", ok(q_check.all_zero()), dur(d3c));

    //------------------------------------------------------
    //Layer 3d: Commit quotients
    let t = Instant::now();
    let qcomms = commit_quotients(&setup, &quots);
    let d3d = t.elapsed();

    println!("\n{} LAYER 3d  Quotient KZG Commits {}", sep('-',3), sep('-',W-35));
    println!("  Max quotient degree: {}  (fits in degree-{} SRS)", quots.max_degree(), DOMAIN_SIZE-1);
    println!("  C_Q_AccA_init  {}", commit_hex(&qcomms.c_q_acc_a_init));
    println!("  C_Q_AccA_rec   {}", commit_hex(&qcomms.c_q_acc_a_rec));
    println!("  C_Q_AccB_init  {}", commit_hex(&qcomms.c_q_acc_b_init));
    println!("  C_Q_AccB_rec   {}", commit_hex(&qcomms.c_q_acc_b_rec));
    println!("  C_Q_KL         {}   time:  {}", commit_hex(&qcomms.c_q_kl), dur(d3d));

    //------------------------------------------------------
    //Layer 3e: Fiat-Shamir non-interactive proof
    let t = Instant::now();
    let fs = fiat_shamir_prove(&polys, &quots, &wcomms, &qcomms, &domain);
    let d3e = t.elapsed();

    println!("\n{} LAYER 3e  Fiat-Shamir Non-Interactive Proof {}", sep('-',3), sep('-',W-48));
    println!("  Hash function : SHA-256 (NIST standard; Poseidon preferred in ZK circuits)");
    println!("  Transcript    : 10 x 32 B compressed G1 points = 320 B hashed to 32 B");
    println!();
    println!("  Challenge zeta   {}", field_hex(&fs.zeta));
    println!("  Challenge alpha   {}", field_hex(&fs.alpha));
    println!();
    println!("  Witness evaluations at zeta:");
    println!("    Bid(zeta)      = {}", to_u64(fs.bid_at_zeta));
    println!("    Ask(zeta)      = {}", to_u64(fs.ask_at_zeta));
    println!("    Acc_B(zeta)    = {}", to_u64(fs.acc_b_at_zeta));
    println!("    Acc_A(zeta)    = {}", to_u64(fs.acc_a_at_zeta));
    println!("    Min(zeta)      = {}", to_u64(fs.min_at_zeta));
    println!("  Shifted evaluations at wzeta:");
    println!("    Acc_A(wzeta)   = {}", to_u64(fs.acc_a_at_omega_zeta));
    println!("    Ask(wzeta)     = {}", to_u64(fs.ask_at_omega_zeta));
    println!("    Acc_B(wzeta)   = {}", to_u64(fs.acc_b_at_omega_zeta));
    println!("  Z_H(zeta) = zeta^32-1 = {}  (non-zero => zeta not in H)", field_hex(&fs.zh_at_zeta));
    println!();
    println!("  Per-constraint residues r_i = V_i(zeta) - Q_i(zeta)*D_i(zeta):");
    let labels = ["r_1 V_AccA_init", "r_2 V_AccA_rec ", "r_3 V_AccB_init",
                  "r_4 V_AccB_rec ", "r_5 V_KL       "];
    for (i, ri) in fs.r.iter().enumerate() {
        println!("    {}  {}  {}", labels[i],
            if ri.is_zero() { "= 0  " } else { "!= 0 !" },
            ok(ri.is_zero()));
    }
    println!();
    println!("  Batched: sum alpha^i*r_i = 0 ?  {}              time:  {}", ok(fs.batch_ok), dur(d3e));


    //------------------------------------------------------
    //Layer 3f: KZG opening proofs
    let t = Instant::now();
    let oprfs = compute_all_opening_proofs(&setup, &polys, &quots, &fs, &domain);
    let d3f_prove = t.elapsed();

    let t = Instant::now();
    let opv = verify_all_openings(&setup, &wcomms, &qcomms, &oprfs, &fs, &domain);
    let d3f_verify = t.elapsed();

    println!("\n{} LAYER 3f  KZG Opening Proofs & Pairing Verification {}", sep('-',3), sep('-',W-55));
    println!("  pi = [(p(tau)-p(zeta))/(tau-zeta)]_G1  via synthetic division + MSM");
    println!("  Check: e(C-p(zeta)*G1, G2) = e(pi, [tau]G2-zeta*G2)");
    println!();
    println!("  {:<18}  {:<20}  {}", "Polynomial", "pi (opening proof)", "Pairing");
    println!("  {}", sep('-', 56));
    let poly_names = [
        "Bid","Ask","Acc_B","Acc_A","Min",
        "Q_AccA_init","Q_AccA_rec","Q_AccB_init","Q_AccB_rec","Q_KL",
    ];
    let pis = [
        oprfs.pi_bid, oprfs.pi_ask, oprfs.pi_acc_b, oprfs.pi_acc_a, oprfs.pi_min,
        oprfs.pi_q_acc_a_init, oprfs.pi_q_acc_a_rec,
        oprfs.pi_q_acc_b_init, oprfs.pi_q_acc_b_rec, oprfs.pi_q_kl,
    ];
    for i in 0..10 {
        println!("  {:<18}  {:<20}  {}",
            poly_names[i], commit_hex(&pis[i]), ok(opv.at_zeta[i]));
    }
    println!("  {}", sep('-', 56));
    println!("  Openings at w*zeta (shifted evaluations for recurrence constraints):");
    let shift_names = ["Acc_A(wzeta)", "Ask(wzeta)", "Acc_B(wzeta)"];
    let shift_pis   = [oprfs.pi_acc_a_shift, oprfs.pi_ask_shift, oprfs.pi_acc_b_shift];
    for i in 0..3 {
        println!("  {:<18}  {:<20}  {}",
            shift_names[i], commit_hex(&shift_pis[i]), ok(opv.at_omega_zeta[i]));
    }
    println!("  {}", sep('-', 56));
    println!("  Batched check at zeta     (2 pairings, 10 polys):  {}   prove time:  {}",
        ok(opv.batch_zeta), dur(d3f_prove));
    println!("  Batched check at w*zeta  (2 pairings,  3 polys):  {}  verify time:  {}",
        ok(opv.batch_omega_zeta), dur(d3f_verify));
    println!("  ALL OPENING PROOFS:  {}", ok(opv.all_pass()));


    //------------------------------------------------------
    //Layer 4: constraint verification
    let t = Instant::now();
    let res = verify_all(&polys, &domain);
    let d4 = t.elapsed();

    println!("\n{} LAYER 4  Vanishing-Polynomial Constraints {}", sep('-',3), sep('-',W-46));
    println!("  V_AccA_init  Acc_A(w^0)=Ask(w^0)                         {}", ok(res.acc_a_init));
    println!("  V_AccA_rec   Acc_A(wX)=Acc_A(X)+Ask(wX) for allX!=w^{{N-1}}    {}", ok(res.acc_a_recurrence));
    println!("  V_AccB_init  Acc_B(w^20)=Bid(w^20)                        {}", ok(res.acc_b_init));
    println!("  V_AccB_rec   Acc_B(X)=Acc_B(wX)+Bid(X)  for allX!=w^{{N-1}}    {}", ok(res.acc_b_recurrence));
    println!("  V_KL         (Acc_A-Min)*(Acc_B-Min)=0  for allXinH            {}", ok(res.min_exclusivity));
    println!("  ALL:  {}                                              time:  {}", ok(res.all_pass()), dur(d4));

    //------------------------------------------------------
    //Timing summary
    let d_total = t_total.elapsed();
    println!("\n{} Timing Summary {}", sep('=',3), sep('=',W-19));
    let rows = [
        ("Layer 1  Array computation",                d1),
        ("Layer 2  Polynomial interpolation (5 IFFT)", d2),
        ("Layer 3a KZG trusted setup (32 G1 muls)",   d3a),
        ("Layer 3b KZG commit witnesses (5 MSM)",      d3b),
        ("Layer 3c Quotient polynomials (5 divs)",     d3c),
        ("Layer 3d KZG commit quotients (5 MSM)",      d3d),
        ("Layer 3e Fiat-Shamir (2xSHA-256 + evals)",   d3e),
        ("Layer 3f Opening proofs  (13 MSMs)",         d3f_prove),
        ("Layer 3f Pairing verification (26->4 pair.)", d3f_verify),
        ("Layer 4  Constraint verification",            d4),
    ];
    for (label, d) in &rows {
        println!("  {label:<46} {:>10}", dur(*d));
    }
    println!("  {}", sep('-', W-2));
    println!("  {:<46} {:>10}", "TOTAL", dur(d_total));
    println!("{}", sep('=', W));

    //------------------------------------------------------
    //Run environment
    println!("\n  Environment (single cold run -- use `cargo bench` for robust numbers)");
    println!("  Binary   : cargo run --release  (opt-level=3, single-threaded)");
    println!("  Toolchain: rustc 1.95.0 / ark-{{ff,poly,ec,bn254}} v0.4 / criterion 0.5");
    println!("  Curve    : BN254 (alt_bn128), 254-bit Fr, |H|=32=2^5\n");
}
