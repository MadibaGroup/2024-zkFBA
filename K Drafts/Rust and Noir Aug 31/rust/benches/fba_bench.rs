use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use zk_fba_csv_full::{
    build_all_polys, build_domain, commit_all, commit_quotients,
    compute_quotients, mask_p_poly, verify_all, full_pipeline,
    KzgScheme, OrderBook,
};

const CSV_100:  &str = "/Users/kimiaesmaili/Downloads/order_book_100_log-normal.csv";
const CSV_1000: &str = "/Users/kimiaesmaili/Downloads/order_book_1000_log-normal.csv";

// ── Helper: build and warm-up all pre-proof state for a given book ───────────

struct BenchState {
    book:    OrderBook,
    scheme:  KzgScheme,
}

fn make_state(book: OrderBook, rng: &mut impl ark_std::rand::Rng) -> BenchState {
    let domain_size = book.domain_size;
    let scheme = KzgScheme::new(domain_size - 1, rng);
    BenchState { book, scheme }
}

// ── Individual layer benchmarks ───────────────────────────────────────────────

fn bench_layer2(c: &mut Criterion) {
    let mut group = c.benchmark_group("layer2_interpolation");
    let mut rng   = ark_std::test_rng();

    for (id, book) in [
        ("21tick",  OrderBook::hardcoded_21tick()),
        ("100tick", OrderBook::from_csv(CSV_100)),
        ("1000tick",OrderBook::from_csv(CSV_1000)),
    ] {
        group.bench_with_input(BenchmarkId::new("IFFT_5polys", id), &book, |b, book| {
            let domain = build_domain(book.domain_size);
            b.iter(|| black_box(build_all_polys(black_box(book), black_box(&domain))))
        });
    }
    group.finish();
}

fn bench_layer3a(c: &mut Criterion) {
    let mut group = c.benchmark_group("layer3a_kzg_setup");
    let mut rng   = ark_std::test_rng();

    for (id, domain_size) in [("21tick", 32usize), ("100tick", 128), ("1000tick", 1024)] {
        group.bench_with_input(BenchmarkId::new("KZG_setup", id), &domain_size, |b, &ds| {
            b.iter(|| {
                let mut r = ark_std::test_rng();
                black_box(KzgScheme::new(black_box(ds - 1), &mut r))
            })
        });
    }
    group.finish();
}

fn bench_layer3b(c: &mut Criterion) {
    let mut group = c.benchmark_group("layer3b_witness_commits");
    let mut rng   = ark_std::test_rng();

    for (id, book) in [
        ("21tick",  OrderBook::hardcoded_21tick()),
        ("100tick", OrderBook::from_csv(CSV_100)),
        ("1000tick",OrderBook::from_csv(CSV_1000)),
    ] {
        let domain = build_domain(book.domain_size);
        let polys  = build_all_polys(&book, &domain);
        let scheme = KzgScheme::new(book.domain_size - 1, &mut rng);
        group.bench_with_input(BenchmarkId::new("commit_5MSM", id), &(), |b, _| {
            b.iter(|| {
                let mut r = ark_std::test_rng();
                black_box(commit_all(black_box(&scheme), black_box(&polys), &mut r))
            })
        });
    }
    group.finish();
}

fn bench_layer3c(c: &mut Criterion) {
    let mut group = c.benchmark_group("layer3c_quotient_polys");
    let mut rng   = ark_std::test_rng();

    for (id, book) in [
        ("21tick",  OrderBook::hardcoded_21tick()),
        ("100tick", OrderBook::from_csv(CSV_100)),
        ("1000tick",OrderBook::from_csv(CSV_1000)),
    ] {
        let domain = build_domain(book.domain_size);
        let polys  = build_all_polys(&book, &domain);
        let mask_p = mask_p_poly(&book, &domain);
        let n      = book.n;
        group.bench_with_input(BenchmarkId::new("quotient_div", id), &(), |b, _| {
            b.iter(|| black_box(compute_quotients(
                black_box(&book), black_box(&polys), black_box(&mask_p), black_box(&domain), black_box(n),
            )))
        });
    }
    group.finish();
}

fn bench_layer4(c: &mut Criterion) {
    let mut group = c.benchmark_group("layer4_constraint_verify");
    let mut rng   = ark_std::test_rng();

    for (id, book) in [
        ("21tick",  OrderBook::hardcoded_21tick()),
        ("100tick", OrderBook::from_csv(CSV_100)),
        ("1000tick",OrderBook::from_csv(CSV_1000)),
    ] {
        let domain = build_domain(book.domain_size);
        let polys  = build_all_polys(&book, &domain);
        let mask_p = mask_p_poly(&book, &domain);
        let n      = book.n;
        group.bench_with_input(BenchmarkId::new("verify_all", id), &(), |b, _| {
            b.iter(|| black_box(verify_all(
                black_box(&book), black_box(&polys), black_box(&mask_p), black_box(&domain), black_box(n),
            )))
        });
    }
    group.finish();
}

fn bench_full_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_pipeline");
    // Longer measurement time for 1000-tick (expensive)
    group.measurement_time(std::time::Duration::from_secs(30));

    for (id, book) in [
        ("21tick",  OrderBook::hardcoded_21tick()),
        ("100tick", OrderBook::from_csv(CSV_100)),
        ("1000tick",OrderBook::from_csv(CSV_1000)),
    ] {
        group.bench_with_input(BenchmarkId::new("end_to_end", id), &book, |b, book| {
            b.iter(|| {
                let mut r = ark_std::test_rng();
                black_box(full_pipeline(black_box(book), &mut r))
            })
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_layer2,
    bench_layer3a,
    bench_layer3b,
    bench_layer3c,
    bench_layer4,
    bench_full_pipeline,
);
criterion_main!(benches);
