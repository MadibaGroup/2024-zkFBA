//! `zeequent`: command-line driver.

use std::time::{Duration, Instant};

use ark_std::rand::{rngs::StdRng, Rng, SeedableRng};
use zeequent_core::{
    fill, prove, prove_no_cross, required_srs_degree, verify, verify_book_opening,
    verify_no_cross, verify_update, BookState, Proof,
};
use zeequent_kzg::Srs;
use zeequent_types::{Order, OrderKind, OrderSide, Outcome, Params};

mod bench;
mod data;
mod noir;
mod ties;
mod workflow;

const USAGE: &str = "usage:
  zeequent demo
  zeequent bench [--ticks 100,1000] [--kin 16,32] [--orders 250] [--runs 5] [--csv FILE] [--raw FILE]
  zeequent data --files A.csv,B.csv [--kin 16] [--runs 3] [--csv FILE]
  zeequent ties [--trades RAW.csv] [--intervals 1,5,10] [--books A.csv,B.csv] [--trials 2000] [--csv FILE]
  zeequent export-noir --file BOOK.csv --out DIR

  zeequent setup --ticks N --kin K [--max-orders M] (--ptau FILE | --insecure) --out SETUP
  zeequent open --setup SETUP --dir DIR
  zeequent check-opening --setup SETUP --dir DIR
  zeequent order --setup SETUP --dir DIR --side buy|sell [--type limit|market|io] [--tick T] --qty Q
  zeequent check-receipt --setup SETUP --receipt FILE [--dir DIR]
  zeequent close --setup SETUP --dir DIR
  zeequent verify --setup SETUP --dir DIR
  zeequent fill --setup SETUP --dir DIR --side buy|sell [--type limit|market|io] [--tick T] --qty Q";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or(&[]);
    let result = match args.first().map(String::as_str) {
        Some("demo") => {
            demo();
            Ok(())
        }
        Some("bench") => {
            bench::run(rest);
            Ok(())
        }
        Some("data") => data::run(rest),
        Some("ties") => ties::run(rest),
        Some("export-noir") => noir::run(rest),
        Some("setup") => workflow::setup(rest),
        Some("open") => workflow::open(rest),
        Some("check-opening") => workflow::check_opening(rest),
        Some("order") => workflow::order(rest),
        Some("check-receipt") => workflow::check_receipt(rest),
        Some("close") => workflow::close(rest),
        Some("verify") => workflow::verify_batch(rest),
        Some("fill") => workflow::fill_for(rest),
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    };
    if let Err(message) = result {
        eprintln!("error: {message}");
        std::process::exit(1);
    }
}

/// Value of `--name` in `args`, if present.
pub(crate) fn option<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    let position = args.iter().position(|a| a == name)?;
    args.get(position + 1).map(String::as_str)
}

pub(crate) fn list<T: std::str::FromStr + Clone>(args: &[String], name: &str, default: &[T]) -> Vec<T> {
    match option(args, name) {
        Some(text) => text.split(',').filter_map(|part| part.trim().parse().ok()).collect(),
        None => default.to_vec(),
    }
}

/// Synthetic order flow that crosses.
pub(crate) fn synthetic_orders(n_ticks: usize, count: usize, max_qty: u64, rng: &mut StdRng) -> Vec<Order> {
    let mid = n_ticks / 2;
    let spread = (n_ticks / 10).max(2);
    (0..count)
        .map(|i| {
            let side = if rng.gen_bool(0.5) { OrderSide::Buy } else { OrderSide::Sell };
            let offset = rng.gen_range(0..spread * 2);
            let tick = match side {
                OrderSide::Buy => (mid + spread / 2).saturating_sub(offset),
                OrderSide::Sell => (mid.saturating_sub(spread / 2) + offset).min(n_ticks - 1),
            };
            let kind = if i % 11 == 0 { OrderKind::Io { tick } } else { OrderKind::Limit { tick } };
            Order { side, kind, qty: rng.gen_range(1..=max_qty) }
        })
        .collect()
}

pub(crate) fn timed<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let start = Instant::now();
    let value = f();
    (value, start.elapsed())
}

pub(crate) fn median(samples: &mut [Duration]) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

pub(crate) fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

/// Number of worker threads in use.
pub(crate) fn threads() -> usize {
    if !cfg!(feature = "parallel") {
        return 1;
    }
    std::env::var("RAYON_NUM_THREADS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|n| *n > 0)
        .unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1))
}

fn demo() {
    let mut rng = StdRng::seed_from_u64(7);
    let params = Params::new(8, 8).unwrap();
    let (srs, key) =
        Srs::insecure_setup_for_domain(required_srs_degree(&params), params.n, &mut rng).unwrap();
    let vk = srs.verifier_key();

    println!("Batch 1: 8 price ticks.");
    let (mut state, opening) = BookState::open(&params, &srs, &key, &mut rng).unwrap();
    println!("  book opened; empty-book proof valid: {}", verify_book_opening(&opening, &vk, &key));

    let limit = |side, tick, qty| Order { side, kind: OrderKind::Limit { tick }, qty };
    let orders = [
        ("buy 10 @ tick 5", limit(OrderSide::Buy, 5, 10)),
        ("sell 6 @ tick 2", limit(OrderSide::Sell, 2, 6)),
        ("IO sell 3 @ tick 4", Order { side: OrderSide::Sell, kind: OrderKind::Io { tick: 4 }, qty: 3 }),
    ];
    for (label, order) in &orders {
        let receipt = state.apply(order, &vk, &key, &mut rng).unwrap();
        println!("  {label:<20} update receipt valid: {}", verify_update(&receipt, &vk, &key));
    }

    let (outcome, witness) = state.clear().unwrap();
    let Outcome::Cross(receipt) = outcome else { panic!("expected a cross") };
    println!("  clearing receipt: {receipt:?}");
    println!(
        "  admitted IO = {}, residual = {}, traded volume = {}",
        receipt.admitted_io(),
        receipt.residual(),
        receipt.v_exec()
    );
    let proof = prove(&params, &receipt, &witness, &state.columns, &srs, &key, &mut rng).unwrap();
    let bytes = proof.to_bytes();
    let decoded = Proof::from_bytes(&bytes).unwrap();
    println!(
        "  proof: {} commitments, {} bytes; verifies: {:?}",
        proof.num_commitments(),
        bytes.len(),
        verify(&params, &receipt, &state.commitments(), &decoded, &vk)
    );
    for (label, order) in &orders {
        println!("  fill for {label:<20}: {}", fill(&receipt, params.n_ticks, order));
    }

    println!("Batch 2: no cross.");
    let (mut state, _) = BookState::open(&params, &srs, &key, &mut rng).unwrap();
    state.apply(&limit(OrderSide::Buy, 2, 5), &vk, &key, &mut rng).unwrap();
    state.apply(&limit(OrderSide::Sell, 5, 5), &vk, &key, &mut rng).unwrap();
    let (outcome, witness) = state.clear().unwrap();
    println!("  outcome: {outcome:?}");
    let proof = prove_no_cross(&params, &witness, &state.columns, &srs, &key, &mut rng).unwrap();
    println!(
        "  no-cross proof: {} commitments, {} bytes; verifies: {:?}",
        proof.num_commitments(),
        proof.size_bytes(),
        verify_no_cross(&params, &state.commitments(), &proof, &vk)
    );
}
