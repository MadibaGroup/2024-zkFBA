//! One-shot timing of a whole batch.

use std::time::Instant;

use ark_std::rand::{rngs::StdRng, Rng, SeedableRng};
use zeequent_core::{
    prove, required_srs_degree, verify, verify_book_opening, verify_update, BookState,
};
use zeequent_kzg::Srs;
use zeequent_types::{Order, OrderKind, OrderSide, Outcome, Params};

fn main() {
    let mut args = std::env::args().skip(1);
    let n_ticks: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(1000);
    let k_in: u32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(32);
    let num_orders: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(2200);
    let params = Params::new(n_ticks, k_in).expect("valid parameters");
    let mut rng = StdRng::seed_from_u64(1);

    let start = Instant::now();
    let (srs, key) =
        Srs::insecure_setup_for_domain(required_srs_degree(&params), params.n, &mut rng)
            .expect("domain exists");
    let vk = srs.verifier_key();
    let setup_time = start.elapsed();

    let start = Instant::now();
    let (mut state, opening) = BookState::open(&params, &srs, &key, &mut rng).unwrap();
    let open_time = start.elapsed();
    let start = Instant::now();
    assert!(verify_book_opening(&opening, &vk, &key));
    let open_verify_time = start.elapsed();

    // Synthetic flow that crosses.
    let mid = n_ticks / 2;
    let spread = (n_ticks / 10).max(2);
    let orders: Vec<Order> = (0..num_orders)
        .map(|i| {
            let side = if rng.gen_bool(0.5) { OrderSide::Buy } else { OrderSide::Sell };
            let offset = rng.gen_range(0..spread * 2);
            let tick = match side {
                OrderSide::Buy => (mid + spread / 2).saturating_sub(offset),
                OrderSide::Sell => (mid.saturating_sub(spread / 2) + offset).min(n_ticks - 1),
            };
            let kind = if i % 11 == 0 { OrderKind::Io { tick } } else { OrderKind::Limit { tick } };
            Order { side, kind, qty: rng.gen_range(1..5000) }
        })
        .collect();

    let start = Instant::now();
    let receipts: Vec<_> = orders
        .iter()
        .map(|order| state.apply(order, &vk, &key, &mut rng).unwrap())
        .collect();
    let update_time = start.elapsed() / num_orders.max(1) as u32;
    let start = Instant::now();
    assert!(verify_update(&receipts[0], &vk, &key));
    let update_verify_time = start.elapsed();

    let start = Instant::now();
    let (outcome, witness) = state.clear().unwrap();
    let clear_time = start.elapsed();
    let receipt = match outcome {
        Outcome::Cross(receipt) => receipt,
        Outcome::NoCross => panic!("the synthetic book did not cross"),
    };

    let start = Instant::now();
    let proof = prove(&params, &receipt, &witness, &state.columns, &srs, &key, &mut rng).unwrap();
    let prove_time = start.elapsed();

    let book = state.commitments();
    let start = Instant::now();
    let result = verify(&params, &receipt, &book, &proof, &vk);
    let verify_time = start.elapsed();

    println!("ticks N = {n_ticks}, domain n = {}, k_in = {k_in}, k = {}", params.n, params.k);
    println!("orders = {num_orders}");
    println!("receipt: {receipt:?}");
    println!("SRS + domain key (test)  : {setup_time:?}");
    println!("open empty book          : {open_time:?}");
    println!("verify book opening      : {open_verify_time:?}");
    println!("apply one order (mean)   : {update_time:?}");
    println!("verify one update receipt: {update_verify_time:?}");
    println!("clearing + witness       : {clear_time:?}");
    println!("prove                    : {prove_time:?}");
    println!("verify                   : {verify_time:?}  -> {result:?}");
    println!("commitments              : {}", proof.num_commitments());
    println!("opening points           : 2");
    println!("proof size (computed)    : {} bytes", proof.size_bytes());
}
