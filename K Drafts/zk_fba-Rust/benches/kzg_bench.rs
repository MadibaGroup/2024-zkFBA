//Criterion benchmark harness for the ZK-FBA pipeline.
//
//Run:   cargo bench
//HTML:  target/criterion/index.html

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use zk_fba::{
    build_all_polys, build_domain, commit_all, commit_quotients,
    compute_all_arrays, compute_all_opening_proofs, compute_quotients,
    fiat_shamir_prove, full_pipeline, random_tau, verify_all,
    verify_all_openings, KZGSetup, DOMAIN_SIZE,
};

//Layer benchmarks

fn bench_layer1(c: &mut Criterion) {
    c.bench_function("layer1_compute_all_arrays", |b| {
        b.iter(|| black_box(compute_all_arrays()))
    });
}

fn bench_layer2(c: &mut Criterion) {
    let (bids, asks, bd, ad, mn) = compute_all_arrays();
    let domain = build_domain();
    c.bench_function("layer2_interpolate_5_polys", |b| {
        b.iter(|| black_box(build_all_polys(
            black_box(&bids), black_box(&asks),
            black_box(&bd),   black_box(&ad), black_box(&mn),
            black_box(&domain),
        )))
    });
}

fn bench_layer3a(c: &mut Criterion) {
    let mut rng = ark_std::test_rng();
    let tau = random_tau(&mut rng);
    c.bench_function("layer3a_kzg_trusted_setup", |b| {
        b.iter(|| black_box(KZGSetup::new(black_box(DOMAIN_SIZE - 1), black_box(tau))))
    });
}

fn bench_layer3b(c: &mut Criterion) {
    let (bids, asks, bd, ad, mn) = compute_all_arrays();
    let domain = build_domain();
    let polys  = build_all_polys(&bids, &asks, &bd, &ad, &mn, &domain);
    let mut rng = ark_std::test_rng();
    let setup = KZGSetup::new(DOMAIN_SIZE - 1, random_tau(&mut rng));
    c.bench_function("layer3b_kzg_commit_witnesses_5", |b| {
        b.iter(|| black_box(commit_all(black_box(&setup), black_box(&polys))))
    });
}

fn bench_layer3c(c: &mut Criterion) {
    let (bids, asks, bd, ad, mn) = compute_all_arrays();
    let domain = build_domain();
    let polys  = build_all_polys(&bids, &asks, &bd, &ad, &mn, &domain);
    c.bench_function("layer3c_compute_quotient_polys_5", |b| {
        b.iter(|| black_box(compute_quotients(black_box(&polys), black_box(&domain))))
    });
}

fn bench_layer3d(c: &mut Criterion) {
    let (bids, asks, bd, ad, mn) = compute_all_arrays();
    let domain = build_domain();
    let polys  = build_all_polys(&bids, &asks, &bd, &ad, &mn, &domain);
    let mut rng = ark_std::test_rng();
    let setup = KZGSetup::new(DOMAIN_SIZE - 1, random_tau(&mut rng));
    let (quots, _) = compute_quotients(&polys, &domain);
    c.bench_function("layer3d_kzg_commit_quotients_5", |b| {
        b.iter(|| black_box(commit_quotients(black_box(&setup), black_box(&quots))))
    });
}

fn bench_layer4(c: &mut Criterion) {
    let (bids, asks, bd, ad, mn) = compute_all_arrays();
    let domain = build_domain();
    let polys  = build_all_polys(&bids, &asks, &bd, &ad, &mn, &domain);
    c.bench_function("layer4_verify_constraints", |b| {
        b.iter(|| black_box(verify_all(black_box(&polys), black_box(&domain))))
    });
}

fn bench_full(c: &mut Criterion) {
    let mut rng = ark_std::test_rng();
    c.bench_function("full_pipeline_end_to_end", |b| {
        b.iter(|| {
            let tau = random_tau(&mut rng);
            black_box(full_pipeline(black_box(tau)))
        })
    });
}

//Parametric: single-poly benchmarks

fn bench_commit_single(c: &mut Criterion) {
    let (bids, asks, bd, ad, mn) = compute_all_arrays();
    let domain = build_domain();
    let polys  = build_all_polys(&bids, &asks, &bd, &ad, &mn, &domain);
    let mut rng = ark_std::test_rng();
    let setup  = KZGSetup::new(DOMAIN_SIZE - 1, random_tau(&mut rng));

    let witness_list = [
        ("w_bid",   &polys.bid),
        ("w_ask",   &polys.ask),
        ("w_acc_b", &polys.acc_b),
        ("w_acc_a", &polys.acc_a),
        ("w_min",   &polys.min),
    ];
    let mut g = c.benchmark_group("kzg_commit_single_witness_poly");
    for (name, poly) in &witness_list {
        g.bench_with_input(BenchmarkId::from_parameter(name), name, |b, _| {
            b.iter(|| black_box(setup.commit(black_box(poly))))
        });
    }
    g.finish();
}

fn bench_quotient_single(c: &mut Criterion) {
    let (bids, asks, bd, ad, mn) = compute_all_arrays();
    let domain = build_domain();
    let polys  = build_all_polys(&bids, &asks, &bd, &ad, &mn, &domain);
    let mut rng = ark_std::test_rng();
    let setup  = KZGSetup::new(DOMAIN_SIZE - 1, random_tau(&mut rng));
    let (quots, _) = compute_quotients(&polys, &domain);

    let quotient_list = [
        ("q_acc_a_init", &quots.q_acc_a_init),
        ("q_acc_a_rec",  &quots.q_acc_a_rec),
        ("q_acc_b_init", &quots.q_acc_b_init),
        ("q_acc_b_rec",  &quots.q_acc_b_rec),
        ("q_kl",         &quots.q_kl),
    ];
    let mut g = c.benchmark_group("kzg_commit_single_quotient_poly");
    for (name, poly) in &quotient_list {
        g.bench_with_input(BenchmarkId::from_parameter(name), name, |b, _| {
            b.iter(|| black_box(setup.commit(black_box(poly))))
        });
    }
    g.finish();
}

fn bench_layer3f_prove(c: &mut Criterion) {
    let (bids, asks, bd, ad, mn) = compute_all_arrays();
    let domain = build_domain();
    let polys  = build_all_polys(&bids, &asks, &bd, &ad, &mn, &domain);
    let mut rng = ark_std::test_rng();
    let setup  = KZGSetup::new(DOMAIN_SIZE - 1, random_tau(&mut rng));
    let wcomms = commit_all(&setup, &polys);
    let (quots, _) = compute_quotients(&polys, &domain);
    let qcomms = commit_quotients(&setup, &quots);
    let fs = fiat_shamir_prove(&polys, &quots, &wcomms, &qcomms, &domain);
    c.bench_function("layer3f_opening_proofs_13_msms", |b| {
        b.iter(|| black_box(compute_all_opening_proofs(
            black_box(&setup), black_box(&polys), black_box(&quots),
            black_box(&fs),    black_box(&domain),
        )))
    });
}

fn bench_layer3f_verify(c: &mut Criterion) {
    let (bids, asks, bd, ad, mn) = compute_all_arrays();
    let domain = build_domain();
    let polys  = build_all_polys(&bids, &asks, &bd, &ad, &mn, &domain);
    let mut rng = ark_std::test_rng();
    let setup  = KZGSetup::new(DOMAIN_SIZE - 1, random_tau(&mut rng));
    let wcomms = commit_all(&setup, &polys);
    let (quots, _) = compute_quotients(&polys, &domain);
    let qcomms = commit_quotients(&setup, &quots);
    let fs  = fiat_shamir_prove(&polys, &quots, &wcomms, &qcomms, &domain);
    let opr = compute_all_opening_proofs(&setup, &polys, &quots, &fs, &domain);
    c.bench_function("layer3f_pairing_verify_all", |b| {
        b.iter(|| black_box(verify_all_openings(
            black_box(&setup),  black_box(&wcomms), black_box(&qcomms),
            black_box(&opr),    black_box(&fs),     black_box(&domain),
        )))
    });
}

fn bench_fiat_shamir(c: &mut Criterion) {
    let (bids, asks, bd, ad, mn) = compute_all_arrays();
    let domain = build_domain();
    let polys  = build_all_polys(&bids, &asks, &bd, &ad, &mn, &domain);
    let mut rng = ark_std::test_rng();
    let setup  = KZGSetup::new(DOMAIN_SIZE - 1, random_tau(&mut rng));
    let wcomms = commit_all(&setup, &polys);
    let (quots, _) = compute_quotients(&polys, &domain);
    let qcomms = commit_quotients(&setup, &quots);
    c.bench_function("layer3e_fiat_shamir_prove", |b| {
        b.iter(|| black_box(fiat_shamir_prove(
            black_box(&polys),  black_box(&quots),
            black_box(&wcomms), black_box(&qcomms),
            black_box(&domain),
        )))
    });
}

//Register all benchmarks

criterion_group!(
    benches,
    bench_layer1,
    bench_layer2,
    bench_layer3a,
    bench_layer3b,
    bench_layer3c,
    bench_layer3d,
    bench_fiat_shamir,
    bench_layer3f_prove,
    bench_layer3f_verify,
    bench_layer4,
    bench_full,
    bench_commit_single,
    bench_quotient_single,
);
criterion_main!(benches);
