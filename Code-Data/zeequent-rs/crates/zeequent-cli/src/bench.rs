//! `zeequent bench`: time every operation of a batch.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::time::Duration;

use ark_std::rand::{rngs::StdRng, Rng, SeedableRng};
use zeequent_core::{
    prove, prove_no_cross, prove_updates, required_srs_degree, required_srs_degree_updates,
    update_domain_size, verify, verify_batch, verify_book_opening, verify_no_cross,
    verify_update, verify_updates, Board, BookState, Proof, PublishedBatch, UpdateProof,
};
use zeequent_kzg::{DomainKey, Srs};
use zeequent_types::{Order, OrderKind, OrderSide, Outcome, Params};

use crate::{list, ms, option, synthetic_orders, threads, timed};

/// Order flow that does not cross: bids below the middle, asks at or above it.
fn quiet_orders(n_ticks: usize, count: usize, max_qty: u64, rng: &mut StdRng) -> Vec<Order> {
    let mid = (n_ticks / 2).max(1);
    (0..count)
        .map(|_| {
            let (side, tick) = if rng.gen_bool(0.5) {
                (OrderSide::Buy, rng.gen_range(0..mid))
            } else {
                (OrderSide::Sell, rng.gen_range(mid..n_ticks))
            };
            Order { side, kind: OrderKind::Limit { tick }, qty: rng.gen_range(1..=max_qty) }
        })
        .collect()
}

#[derive(Default)]
struct Samples {
    times: BTreeMap<&'static str, Vec<Duration>>,
    sizes: BTreeMap<&'static str, usize>,
}

impl Samples {
    fn time<T>(&mut self, metric: &'static str, f: impl FnOnce() -> T) -> T {
        let (value, elapsed) = timed(f);
        self.times.entry(metric).or_default().push(elapsed);
        value
    }
}

/// Median, minimum, quartiles and maximum, in milliseconds.
fn summary(samples: &[Duration]) -> [f64; 5] {
    let mut sorted = samples.to_vec();
    sorted.sort();
    let at = |q: usize| ms(sorted[(sorted.len() - 1) * q / 4]);
    [ms(sorted[sorted.len() / 2]), at(0), at(1), at(3), at(4)]
}

/// `bench [--ticks ..] [--kin ..] [--orders N] [--runs R] [--csv FILE] [--raw FILE]`
pub fn run(args: &[String]) {
    let ticks: Vec<usize> = list(args, "--ticks", &[100, 1000]);
    let widths: Vec<u32> = list(args, "--kin", &[16, 32]);
    let num_orders: usize = option(args, "--orders").and_then(|v| v.parse().ok()).unwrap_or(250);
    let runs: usize = option(args, "--runs").and_then(|v| v.parse().ok()).unwrap_or(5).max(1);

    println!("threads = {}, runs = {runs}, orders per batch = {num_orders}", threads());
    println!("SRS: local insecure setup. Median times in milliseconds.\n");
    println!("| N | k_in | step domain | prove | verify+decode | proof B | upd prove | upd verify+decode | upd B | no-cross prove | no-cross B | whole batch check | order update |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|---|");

    let header = "n_ticks,n,k_in,k,threads,orders,step_domain,runs";
    let mut csv = format!("{header},metric,median_ms,min_ms,p25_ms,p75_ms,max_ms\n");
    let mut raw = format!("{header},metric,run,ms\n");

    for &n_ticks in &ticks {
        for &k_in in &widths {
            let Ok(params) = Params::new(n_ticks, k_in) else {
                eprintln!("skipping N = {n_ticks}, k_in = {k_in}");
                continue;
            };
            let mut rng = StdRng::seed_from_u64(1);
            let step_domain = update_domain_size(params.n, 4 * num_orders);
            let degree = required_srs_degree(&params).max(required_srs_degree_updates(step_domain));
            let (srs, key) =
                Srs::insecure_setup_for_domain(degree, params.n, &mut rng).expect("domain exists");
            let update_key = if step_domain == params.n {
                key.clone()
            } else {
                DomainKey::lagrange_only(step_domain, &srs).expect("domain exists")
            };
            let vk = srs.verifier_key();
            let max_qty = ((params.input_bound() - 1) / num_orders.max(1) as u64).clamp(1, 5000);
            let mut s = Samples::default();

            for _ in 0..runs {
                // A crossing batch.
                let (mut state, opening) =
                    s.time("book_open", || BookState::open(&params, &srs, &key, &mut rng).unwrap());
                assert!(s.time("book_open_check", || verify_book_opening(&opening, &vk, &key)));

                let orders = synthetic_orders(n_ticks, num_orders, max_qty, &mut rng);
                let mut board = Board::new();
                let mut receipts = Vec::with_capacity(orders.len());
                let mut elapsed = Duration::ZERO;
                for order in &orders {
                    let (receipt, t) = timed(|| state.apply(order, &vk, &key, &mut rng).unwrap());
                    elapsed += t;
                    receipts.push(receipt);
                    board.append_order(&opening, &state.commitments());
                }
                s.times.entry("order_update").or_default().push(elapsed / num_orders.max(1) as u32);
                assert!(s.time("receipt_check", || verify_update(&receipts[0], &vk, &key)));

                let (outcome, witness) = s.time("clear", || state.clear().unwrap());
                let Outcome::Cross(receipt) = outcome else {
                    panic!("the synthetic book did not cross");
                };
                let proof = s.time("prove", || {
                    prove(&params, &receipt, &witness, &state.columns, &srs, &key, &mut rng).unwrap()
                });
                let bytes = proof.to_bytes();
                let book = state.commitments();
                let decoded = s.time("decode", || Proof::from_bytes(&bytes).unwrap());
                let result = s.time("verify", || verify(&params, &receipt, &book, &decoded, &vk));
                assert_eq!(result, Ok(()));

                let updates = s.time("updates_prove", || {
                    prove_updates(&params, &state.history, &srs, &key, &update_key, &mut rng)
                        .unwrap()
                });
                let update_bytes = updates.to_bytes();
                let decoded_updates =
                    s.time("updates_decode", || UpdateProof::from_bytes(&update_bytes).unwrap());
                let deltas = board.deltas(&opening);
                let result = s.time("updates_verify", || {
                    verify_updates(&params, &deltas, step_domain, &decoded_updates, &vk, &key)
                });
                assert_eq!(result, Ok(()));

                let batch = PublishedBatch {
                    opening: &opening,
                    board: &board,
                    outcome: &Outcome::Cross(receipt),
                    proof: &decoded,
                    updates: &decoded_updates,
                };
                let result = s.time("batch_check", || {
                    verify_batch(&params, step_domain, &batch, &vk, &key)
                });
                assert!(result.is_ok());

                s.sizes.insert("commitments", proof.num_commitments());
                s.sizes.insert("proof_bytes", bytes.len());
                s.sizes.insert("updates_commitments", updates.proof.num_commitments() + 1);
                s.sizes.insert("updates_proof_bytes", update_bytes.len());
                s.sizes.insert("log_bytes_per_order", board.to_bytes().len() / num_orders.max(1));

                // A batch that does not cross.
                let (mut quiet, _) = BookState::open(&params, &srs, &key, &mut rng).unwrap();
                for order in quiet_orders(n_ticks, num_orders, max_qty, &mut rng) {
                    quiet.apply(&order, &vk, &key, &mut rng).unwrap();
                }
                let (outcome, witness) = quiet.clear().unwrap();
                assert_eq!(outcome, Outcome::NoCross);
                let proof = s.time("nocross_prove", || {
                    prove_no_cross(&params, &witness, &quiet.columns, &srs, &key, &mut rng).unwrap()
                });
                let bytes = proof.to_bytes();
                let decoded = s.time("nocross_decode", || Proof::from_bytes(&bytes).unwrap());
                let book = quiet.commitments();
                let result =
                    s.time("nocross_verify", || verify_no_cross(&params, &book, &decoded, &vk));
                assert_eq!(result, Ok(()));
                s.sizes.insert("nocross_commitments", proof.num_commitments());
                s.sizes.insert("nocross_proof_bytes", bytes.len());
            }

            let prefix = format!(
                "{n_ticks},{},{k_in},{},{},{num_orders},{step_domain},{runs}",
                params.n,
                params.k,
                threads()
            );
            for (metric, samples) in &s.times {
                let [median, min, p25, p75, max] = summary(samples);
                writeln!(csv, "{prefix},{metric},{median:.4},{min:.4},{p25:.4},{p75:.4},{max:.4}")
                    .unwrap();
                for (run, sample) in samples.iter().enumerate() {
                    writeln!(raw, "{prefix},{metric},{run},{:.4}", ms(*sample)).unwrap();
                }
            }
            for (metric, size) in &s.sizes {
                writeln!(csv, "{prefix},{metric},{size},{size},{size},{size},{size}").unwrap();
            }
            let median = |metric: &str| summary(&s.times[metric])[0];
            println!(
                "| {n_ticks} | {k_in} | {step_domain} | {:.1} | {:.1} | {} | {:.1} | {:.1} | {} | {:.1} | {} | {:.1} | {:.3} |",
                median("prove"),
                median("verify") + median("decode"),
                s.sizes["proof_bytes"],
                median("updates_prove"),
                median("updates_verify") + median("updates_decode"),
                s.sizes["updates_proof_bytes"],
                median("nocross_prove"),
                s.sizes["nocross_proof_bytes"],
                median("batch_check"),
                median("order_update"),
            );
        }
    }

    for (name, content) in [("--csv", &csv), ("--raw", &raw)] {
        if let Some(path) = option(args, name) {
            match std::fs::write(path, content) {
                Ok(()) => println!("wrote {path}"),
                Err(e) => eprintln!("could not write {path}: {e}"),
            }
        }
    }
}
