//! Tests for the reference clearing rule and the integer constraint checker.

use zeequent_core::{check, clear, fill, Violation, Witness};
use zeequent_types::{Book, Order, OrderKind, OrderSide, Outcome, Params, Receipt, Side};

const N: usize = 8;

fn params() -> Params {
    Params::new(N, 8).unwrap()
}

fn limit(side: OrderSide, tick: usize, qty: u64) -> Order {
    Order {
        side,
        kind: OrderKind::Limit { tick },
        qty,
    }
}

fn io(side: OrderSide, tick: usize, qty: u64) -> Order {
    Order {
        side,
        kind: OrderKind::Io { tick },
        qty,
    }
}

fn book(orders: &[Order]) -> Book {
    let mut book = Book::new(N);
    for order in orders {
        book.add(order).unwrap();
    }
    book
}

fn cross(book: &Book) -> (Receipt, Witness) {
    let (outcome, witness) = clear(book, &params()).unwrap();
    assert_eq!(check(&params(), &outcome, &witness), Ok(()));
    match outcome {
        Outcome::Cross(receipt) => (receipt, witness),
        Outcome::NoCross => panic!("expected a cross"),
    }
}

fn violated(receipt: Receipt, witness: &Witness) -> &'static str {
    match check(&params(), &Outcome::Cross(receipt), witness) {
        Err(Violation { constraint, .. }) => constraint,
        Ok(()) => "none",
    }
}

#[test]
fn params_widths() {
    let p = params();
    assert_eq!((p.n, p.k_in, p.k), (8, 8, 11));
    assert_eq!(Params::new(1000, 32).unwrap().n, 1024);
    assert_eq!(Params::new(1000, 32).unwrap().k, 42);
}

#[test]
fn unique_maximizer() {
    // Bids: 5 @ 6, 5 @ 4.
    let b = book(&[
        limit(OrderSide::Buy, 6, 5),
        limit(OrderSide::Buy, 4, 5),
        limit(OrderSide::Sell, 2, 4),
        limit(OrderSide::Sell, 4, 4),
    ]);
    let (r, _) = cross(&b);
    // Tick 4: demand 10, supply 8.
    assert_eq!((r.v_max, r.c, r.d, r.p_star), (8, 4, 4, 4));
    assert_eq!((r.v_min_delta, r.side, r.p_at_price), (2, Side::DemandLong, 5));
}

#[test]
fn demand_long_plateau_picks_the_at_price_tick() {
    // The example from fixedConstraints.md, Section 7.
    let b = book(&[limit(OrderSide::Buy, 5, 10), limit(OrderSide::Sell, 2, 6)]);
    let (r, w) = cross(&b);
    assert_eq!((r.v_max, r.c, r.d), (6, 2, 5));
    assert_eq!((r.p_star, r.v_min_delta, r.side, r.p_at_price), (5, 4, Side::DemandLong, 10));
    assert_eq!((w.slack[1], w.slack[6]), (5, 5));
    assert_eq!((r.residual(), r.v_exec()), (4, 6));

    // At-price bids are rationed 6/10; every ask fills.
    assert_eq!(fill(&r, N, &limit(OrderSide::Buy, 5, 10)), 6);
    assert_eq!(fill(&r, N, &limit(OrderSide::Sell, 2, 6)), 6);

    // "Lowest tick" (p* = 2) is rejected, whatever P the exchange claims.
    let lowest = Receipt { p_star: 2, p_at_price: 0, ..r };
    assert_eq!(violated(lowest, &w), "scalar");
    let lowest_lying = Receipt { p_star: 2, ..r };
    assert_eq!(violated(lowest_lying, &w), "F32");
}

#[test]
fn supply_long_plateau_picks_the_lowest_tick() {
    let b = book(&[limit(OrderSide::Buy, 5, 6), limit(OrderSide::Sell, 2, 10)]);
    let (r, _) = cross(&b);
    assert_eq!((r.v_max, r.c, r.d), (6, 2, 5));
    assert_eq!((r.p_star, r.v_min_delta, r.side, r.p_at_price), (2, 4, Side::SupplyLong, 10));
    assert_eq!(fill(&r, N, &limit(OrderSide::Sell, 2, 10)), 6);
    assert_eq!(fill(&r, N, &limit(OrderSide::Buy, 5, 6)), 6);
}

#[test]
fn supply_long_valley_inside_the_plateau() {
    // Bids: 6 @ 5, 5 @ 3.
    let b = book(&[
        limit(OrderSide::Buy, 5, 6),
        limit(OrderSide::Buy, 3, 5),
        limit(OrderSide::Sell, 2, 6),
        limit(OrderSide::Sell, 4, 4),
    ]);
    let (r, w) = cross(&b);
    assert_eq!((r.v_max, r.c, r.d), (6, 2, 5));
    assert_eq!((r.p_star, r.v_min_delta, r.side, r.p_at_price), (4, 4, Side::SupplyLong, 4));
    // Tie-break slack at p* - 1: delta[3] - v - 1 = 5 - 4 - 1.
    assert_eq!(w.slack[3], 0);

    // Tick 5 has the same imbalance but no at-price asks.
    let higher = Receipt { p_star: 5, ..r };
    assert_ne!(violated(higher, &w), "none");
}

#[test]
fn balanced_plateau_picks_the_lowest_tick() {
    let b = book(&[limit(OrderSide::Buy, 5, 5), limit(OrderSide::Sell, 2, 5)]);
    let (r, w) = cross(&b);
    assert_eq!((r.v_max, r.c, r.d, r.p_star), (5, 2, 5, 2));
    assert_eq!((r.v_min_delta, r.p_at_price, r.j_io), (0, 0, 0));
    let higher = Receipt { p_star: 3, ..r };
    assert_eq!(violated(higher, &w), "F21");
}

#[test]
fn io_orders_reduce_the_imbalance() {
    let orders = [
        limit(OrderSide::Buy, 5, 10),
        limit(OrderSide::Sell, 2, 6),
        io(OrderSide::Sell, 4, 3), // eligible: limit <= p* = 5
        io(OrderSide::Sell, 7, 5), // not eligible: limit above p*
        io(OrderSide::Buy, 6, 9),  // wrong side: demand is already long
    ];
    let (r, w) = cross(&book(&orders));
    assert_eq!((r.p_star, r.v_min_delta, r.j_io), (5, 4, 3));
    assert_eq!((r.admitted_io(), r.residual(), r.v_exec()), (3, 1, 9));

    assert_eq!(fill(&r, N, &orders[0]), 9); // 10 * (10 - 1) / 10
    assert_eq!(fill(&r, N, &orders[1]), 6);
    assert_eq!(fill(&r, N, &orders[2]), 3);
    assert_eq!(fill(&r, N, &orders[3]), 0);
    assert_eq!(fill(&r, N, &orders[4]), 0);

    let inflated = Receipt { j_io: 8, ..r };
    assert_eq!(violated(inflated, &w), "F33");
}

#[test]
fn no_cross() {
    let b = book(&[limit(OrderSide::Buy, 2, 5), limit(OrderSide::Sell, 5, 5)]);
    let (outcome, w) = clear(&b, &params()).unwrap();
    assert_eq!(outcome, Outcome::NoCross);
    assert_eq!(check(&params(), &outcome, &w), Ok(()));

    // A crossing book cannot be passed off as "no trade".
    let crossing = book(&[limit(OrderSide::Buy, 5, 5), limit(OrderSide::Sell, 2, 5)]);
    let (_, w) = clear(&crossing, &params()).unwrap();
    assert_eq!(
        check(&params(), &Outcome::NoCross, &w).unwrap_err().constraint,
        "F14"
    );
}

#[test]
fn tampered_receipts_fail() {
    let b = book(&[limit(OrderSide::Buy, 5, 10), limit(OrderSide::Sell, 2, 6)]);
    let (r, w) = cross(&b);

    assert_eq!(violated(Receipt { v_max: r.v_max + 1, ..r }, &w), "F15");
    assert_eq!(violated(Receipt { v_max: r.v_max - 1, ..r }, &w), "F14");
    // A plateau reported one tick short on the left.
    assert_eq!(violated(Receipt { c: r.c + 1, ..r }, &w), "F17");
    // A plateau reported one tick short on the right.
    assert_eq!(violated(Receipt { d: r.d - 1, p_star: r.d - 1, ..r }, &w), "F17");
    assert_eq!(violated(Receipt { v_min_delta: r.v_min_delta - 1, ..r }, &w), "F20a");
    assert_eq!(violated(Receipt { side: Side::SupplyLong, ..r }, &w), "F20a");
    assert_eq!(violated(Receipt { p_at_price: r.p_at_price + 1, ..r }, &w), "F32");
    assert_eq!(violated(Receipt { p_star: r.d + 1, ..r }, &w), "scalar");
}

#[test]
fn tampered_witness_fails() {
    let b = book(&[limit(OrderSide::Buy, 5, 10), limit(OrderSide::Sell, 2, 6)]);
    let (r, w) = cross(&b);
    let outcome = Outcome::Cross(r);

    // The off-by-one supply recurrence from the original constraint list.
    let mut shifted = w.clone();
    shifted.acc_a[0] = shifted.a[0];
    for i in 0..shifted.a.len() - 1 {
        shifted.acc_a[i + 1] = shifted.acc_a[i] + shifted.a[i];
    }
    assert_eq!(check(&params(), &outcome, &shifted).unwrap_err().constraint, "F8");

    // No padding ticks here (N = n): test the range bound.
    let mut oversized = w.clone();
    oversized.b[5] = params().input_bound();
    assert_eq!(check(&params(), &outcome, &oversized).unwrap_err().constraint, "F1");

    // Min set to the larger depth curve.
    let mut wrong_min = w.clone();
    wrong_min.min[3] = wrong_min.acc_b[3];
    assert_eq!(check(&params(), &outcome, &wrong_min).unwrap_err().constraint, "F9");
}

#[test]
fn padding_ticks_must_be_empty() {
    // N = 5 real ticks in a domain of 8.
    let p = Params::new(5, 8).unwrap();
    let mut b = Book::new(5);
    b.add(&limit(OrderSide::Buy, 4, 7)).unwrap();
    b.add(&limit(OrderSide::Sell, 1, 7)).unwrap();
    let (outcome, w) = clear(&b, &p).unwrap();
    assert_eq!(check(&p, &outcome, &w), Ok(()));
    match outcome {
        // The plateau ends at the last real tick; the right cliff is a padding tick.
        Outcome::Cross(r) => assert_eq!((r.c, r.d, w.slack[5]), (1, 4, 6)),
        Outcome::NoCross => panic!("expected a cross"),
    }

    let mut hidden = w.clone();
    hidden.b[6] = 1;
    assert_eq!(check(&p, &outcome, &hidden).unwrap_err().constraint, "F3");
}

/// Small deterministic generator, so the test needs no dependency.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, modulus: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) % modulus
    }
}

#[test]
fn random_books_clear_and_check() {
    let n_ticks = 12;
    let p = Params::new(n_ticks, 6).unwrap();
    let mut rng = Lcg(7);
    let mut crosses = 0;

    for _ in 0..500 {
        let mut b = Book::new(n_ticks);
        for _ in 0..rng.next(10) {
            let side = if rng.next(2) == 0 { OrderSide::Buy } else { OrderSide::Sell };
            let tick = rng.next(n_ticks as u64) as usize;
            let qty = 1 + rng.next(5);
            let order = if rng.next(4) == 0 {
                Order { side, kind: OrderKind::Io { tick }, qty }
            } else {
                Order { side, kind: OrderKind::Limit { tick }, qty }
            };
            b.add(&order).unwrap();
        }

        let (outcome, w) = clear(&b, &p).unwrap();
        assert_eq!(check(&p, &outcome, &w), Ok(()), "book: {b:?}");

        // Independent recomputation of executable volume at every tick.
        let volume = |i: usize| {
            let demand: u64 = b.bids[i..].iter().sum();
            let supply: u64 = b.asks[..=i].iter().sum();
            (demand.min(supply), demand.abs_diff(supply))
        };
        let best = (0..n_ticks).map(|i| volume(i).0).max().unwrap();

        match outcome {
            Outcome::NoCross => assert_eq!(best, 0),
            Outcome::Cross(r) => {
                crosses += 1;
                assert_eq!(r.v_max, best);
                for i in 0..n_ticks {
                    assert_eq!(volume(i).0 == best, (r.c..=r.d).contains(&i));
                }
                let least = (r.c..=r.d).map(|i| volume(i).1).min().unwrap();
                assert_eq!((r.v_min_delta, volume(r.p_star).1), (least, least));
                // The whole imbalance sits at the clearing tick.
                assert!(r.v_min_delta <= r.p_at_price || r.v_min_delta == 0);
                assert!(r.v_exec() >= r.v_max && r.residual() <= r.v_min_delta);
            }
        }
    }
    assert!(crosses > 50, "generator produced too few crossing books");
}

/// The witness a cheating prover would use for `receipt`: only `Slack` is refitted.
fn witness_for(receipt: &Receipt, honest: &Witness) -> Witness {
    let n = honest.b.len();
    let mut w = honest.clone();
    w.slack = vec![0; n];
    let cliff = |i: usize| receipt.v_max.wrapping_sub(w.min[i]).wrapping_sub(1);
    let (c, d, p) = (receipt.c, receipt.d, receipt.p_star);
    let (left, right) = (c.checked_sub(1).map(cliff), (d + 1 < n).then(|| cliff(d + 1)));
    if let Some(value) = left {
        w.slack[c - 1] = value;
    }
    if let Some(value) = right {
        w.slack[d + 1] = value;
    }
    if p > c && (receipt.side == Side::SupplyLong || receipt.v_min_delta == 0) {
        w.slack[p - 1] = w.delta[p - 1].wrapping_sub(receipt.v_min_delta).wrapping_sub(1);
    }
    w
}

/// On small books, the constraints accept exactly the true receipt.
#[test]
fn only_the_true_receipt_satisfies_the_constraints() {
    let n_ticks = 6;
    let p = Params::new(n_ticks, 4).unwrap();
    let mut rng = Lcg(2027);
    let (mut crosses, mut with_plateau, mut with_tied_valley) = (0, 0, 0);

    for _ in 0..300 {
        let mut b = Book::new(n_ticks);
        for _ in 0..1 + rng.next(6) {
            let side = if rng.next(2) == 0 { OrderSide::Buy } else { OrderSide::Sell };
            let tick = rng.next(n_ticks as u64) as usize;
            let qty = 1 + rng.next(3);
            let kind = if rng.next(4) == 0 { OrderKind::Io { tick } } else { OrderKind::Limit { tick } };
            b.add(&Order { side, kind, qty }).unwrap();
        }
        let (outcome, honest) = clear(&b, &p).unwrap();
        let total: u64 = [&b.bids, &b.asks, &b.io_bids, &b.io_asks]
            .iter()
            .map(|column| column.iter().sum::<u64>())
            .sum();
        let top = total + 1;

        let mut accepted = Vec::new();
        for c in 0..n_ticks {
            for d in c..n_ticks {
                for p_star in c..=d {
                    for v_max in 0..=top {
                        for v_min_delta in 0..=top {
                            for side in [Side::DemandLong, Side::SupplyLong] {
                                let base = Receipt {
                                    v_max, c, d, p_star, v_min_delta, side,
                                    p_at_price: top, j_io: 0,
                                };
                                let w = witness_for(&base, &honest);
                                // P and J matter only to the scalar checks, F32 and F33.
                                let first = check(&p, &Outcome::Cross(base), &w);
                                let depends_on_p_or_j = match first {
                                    Ok(()) => true,
                                    Err(v) => ["scalar", "F32", "F33"].contains(&v.constraint),
                                };
                                if !depends_on_p_or_j {
                                    continue;
                                }
                                for p_at_price in 0..=top {
                                    for j_io in 0..=top {
                                        let r = Receipt { p_at_price, j_io, ..base };
                                        if check(&p, &Outcome::Cross(r), &w).is_ok() {
                                            accepted.push(r);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        match outcome {
            Outcome::NoCross => {
                assert!(accepted.is_empty(), "book {b:?} accepts {accepted:?}");
                assert_eq!(check(&p, &Outcome::NoCross, &honest), Ok(()));
            }
            Outcome::Cross(r) => {
                assert_eq!(accepted, vec![r], "book {b:?}");
                assert!(check(&p, &Outcome::NoCross, &honest).is_err());
                crosses += 1;
                if r.d > r.c {
                    with_plateau += 1;
                    let ties = (r.c..=r.d).filter(|i| honest.delta[*i] == r.v_min_delta).count();
                    with_tied_valley += (ties > 1) as u32;
                }
            }
        }
    }
    // The sample must exercise every case of the rule.
    assert!(crosses > 80 && with_plateau > 30 && with_tied_valley > 10,
        "{crosses} crosses, {with_plateau} plateaus, {with_tied_valley} tied valleys");
}

/// The receipt equals the one recomputed from the written rule by direct search.
#[test]
fn the_receipt_matches_the_written_rule() {
    let n_ticks = 10;
    let p = Params::new(n_ticks, 6).unwrap();
    let mut rng = Lcg(4754);
    let (mut crosses, mut tied) = (0, 0);

    for _ in 0..2000 {
        let mut b = Book::new(n_ticks);
        for _ in 0..1 + rng.next(9) {
            let side = if rng.next(2) == 0 { OrderSide::Buy } else { OrderSide::Sell };
            let tick = rng.next(n_ticks as u64) as usize;
            let qty = 1 + rng.next(4);
            let kind = match rng.next(8) {
                0 => OrderKind::Market,
                1 | 2 => OrderKind::Io { tick },
                _ => OrderKind::Limit { tick },
            };
            b.add(&Order { side, kind, qty }).unwrap();
        }

        // Section 1, as written.
        let demand = |i: usize| b.bids[i..].iter().sum::<u64>() as i64;
        let supply = |i: usize| b.asks[..=i].iter().sum::<u64>() as i64;
        let executable = |i: usize| demand(i).min(supply(i));
        let gap = |i: usize| supply(i) - demand(i);
        let v_max = (0..n_ticks).map(executable).max().unwrap();
        let outcome = clear(&b, &p).unwrap().0;
        if v_max == 0 {
            assert_eq!(outcome, Outcome::NoCross);
            continue;
        }
        let plateau: Vec<usize> = (0..n_ticks).filter(|i| executable(*i) == v_max).collect();
        let v = plateau.iter().map(|i| gap(*i).abs()).min().unwrap();
        let long_volume = |i: usize| if gap(i) < 0 { b.bids[i] } else { b.asks[i] } as i64;
        let candidates: Vec<usize> = plateau
            .iter()
            .copied()
            .filter(|i| gap(*i).abs() == v && (v == 0 || long_volume(*i) >= v))
            .collect();
        assert!(!candidates.is_empty(), "Lemma 6 fails for {b:?}");
        let p_star = candidates[0];
        let supply_long = gap(p_star) > 0;
        let expected = Receipt {
            v_max: v_max as u64,
            c: plateau[0],
            d: *plateau.last().unwrap(),
            p_star,
            v_min_delta: v as u64,
            side: if v > 0 && supply_long { Side::SupplyLong } else { Side::DemandLong },
            p_at_price: if v > 0 { long_volume(p_star) as u64 } else { 0 },
            j_io: match (v > 0, supply_long) {
                (false, _) => 0,
                (true, false) => b.io_asks[..=p_star].iter().sum(),
                (true, true) => b.io_bids[p_star..].iter().sum(),
            },
        };
        assert_eq!(outcome, Outcome::Cross(expected), "book {b:?}");
        crosses += 1;
        tied += (candidates.len() > 1 || plateau.iter().filter(|i| gap(**i).abs() == v).count() > 1) as u32;
        // The plateau is an interval.
        assert_eq!(plateau.len(), expected.d - expected.c + 1);

        // Proposition 4: volume bought equals volume sold equals V + I.
        let (r, admitted) = (expected, expected.admitted_io() as i64);
        let (at_price, residual) = (r.p_at_price as i64, r.residual() as i64);
        if v == 0 {
            assert_eq!((demand(p_star), supply(p_star)), (v_max, v_max));
        } else if !supply_long {
            assert_eq!((supply(p_star), demand(p_star)), (v_max, v_max + v));
            let bought = demand(p_star) - at_price + (at_price - residual);
            assert_eq!((bought, supply(p_star) + admitted), (v_max + admitted, v_max + admitted));
        } else {
            assert_eq!((demand(p_star), supply(p_star)), (v_max, v_max + v));
            let sold = supply(p_star) - at_price + (at_price - residual);
            assert_eq!((sold, demand(p_star) + admitted), (v_max + admitted, v_max + admitted));
        }
        assert!(v == 0 || (at_price >= v && residual <= at_price));
        assert_eq!(r.v_exec() as i64, v_max + admitted);
    }
    assert!(crosses > 800 && tied > 50, "{crosses} crosses, {tied} with ties");
}
