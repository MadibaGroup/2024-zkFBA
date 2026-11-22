// Zeequent clearing as a Noir circuit: computes the receipt and checks it with constraints F1-F33.

use std::hash::poseidon2_permutation;

// The four book columns. The u32 type is the range check on each tick (F1, F2, F24, F25).
pub struct Book<let N: u32> {
    pub bids: [u32; N],
    pub asks: [u32; N],
    pub io_bids: [u32; N],
    pub io_asks: [u32; N],
}

// The public clearing receipt.
pub struct Receipt {
    pub v_max: u64,
    pub c: u32,
    pub d: u32,
    pub p_star: u32,
    pub v_min_delta: u64,
    // false: demand long, true: supply long
    pub side: bool,
    pub p_at_price: u64,
    pub j_io: u64,
}

impl Eq for Receipt {
    fn eq(self, other: Self) -> bool {
        (self.v_max == other.v_max)
            & (self.c == other.c)
            & (self.d == other.d)
            & (self.p_star == other.p_star)
            & (self.v_min_delta == other.v_min_delta)
            & (self.side == other.side)
            & (self.p_at_price == other.p_at_price)
            & (self.j_io == other.j_io)
    }
}

// One field element per tick: the four volumes in 32-bit lanes.
fn packed<let N: u32>(book: Book<N>) -> [Field; N] {
    let lane: Field = 4294967296;
    let mut ticks = [0; N];
    for i in 0..N {
        ticks[i] = book.bids[i] as Field
            + lane * (book.asks[i] as Field
                + lane * (book.io_bids[i] as Field + lane * (book.io_asks[i] as Field)));
    }
    ticks
}

// Hash the four volume arrays with the private salt using a rate-three Poseidon2 sponge.
pub fn digest<let N: u32>(book: Book<N>, salt: Field) -> Field {
    let ticks = packed(book);
    let mut state: [Field; 4] = [salt, 0, 0, N as Field];
    state = poseidon2_permutation(state);
    for block in 0..(N + 2) / 3 {
        for k in 0..3 {
            let i = 3 * block + k;
            if i < N {
                state[k] += ticks[i];
            }
        }
        state = poseidon2_permutation(state);
    }
    state[0]
}

// Cumulative volume from the top tick down (F5, F7).
pub fn demand<let N: u32>(column: [u32; N]) -> [u64; N] {
    let mut acc = [0 as u64; N];
    let mut running: u64 = 0;
    for i in 0..N {
        let tick = N - 1 - i;
        running += column[tick] as u64;
        acc[tick] = running;
    }
    acc
}

// Cumulative volume from the bottom tick up (F6, F8).
pub fn supply<let N: u32>(column: [u32; N]) -> [u64; N] {
    let mut acc = [0 as u64; N];
    let mut running: u64 = 0;
    for i in 0..N {
        running += column[i] as u64;
        acc[i] = running;
    }
    acc
}

unconstrained fn smaller(x: u64, y: u64) -> u64 {
    if x < y {
        x
    } else {
        y
    }
}

// Executable volume and the two surpluses at one tick (F9-F13).
fn executable(acc_b: u64, acc_a: u64) -> (u64, u64, u64) {
    // Safety: `min` is pinned by the two subtractions and the product below.
    let min = unsafe { smaller(acc_b, acc_a) };
    let surp_b = acc_b - min;
    let surp_a = acc_a - min;
    assert((surp_a as Field) * (surp_b as Field) == 0, "F11");
    (min, surp_b, surp_a)
}

// Checks that `r` is the clearing of `book`.
pub fn check_clearing<let N: u32>(book: Book<N>, r: Receipt) {
    // Receipt checks.
    assert(r.c <= r.p_star, "receipt");
    assert(r.p_star <= r.d, "receipt");
    assert(r.d < N, "receipt");
    assert(r.v_max >= 1, "receipt");
    assert(r.v_min_delta <= r.p_at_price, "receipt");
    if r.v_min_delta == 0 {
        assert(!r.side, "receipt");
        assert(r.p_at_price == 0, "receipt");
        assert(r.j_io == 0, "receipt");
    }

    let acc_b = demand(book.bids);
    let acc_a = supply(book.asks);
    let io_acc_b = demand(book.io_bids);
    let io_acc_a = supply(book.io_asks);

    let mut min = [0 as u64; N];
    let mut surp_b = [0 as u64; N];
    let mut surp_a = [0 as u64; N];
    let mut delta = [0 as u64; N];
    for i in 0..N {
        let (m, sb, sa) = executable(acc_b[i], acc_a[i]);
        min[i] = m;
        surp_b[i] = sb;
        surp_a[i] = sa;
        // F18
        delta[i] = sa + sb;
        assert(m <= r.v_max, "F14");
        if (r.c <= i) & (i <= r.d) {
            assert(delta[i] >= r.v_min_delta, "F19");
        }
    }

    assert(min[r.c] == r.v_max, "F15");
    assert(min[r.d] == r.v_max, "F16");

    let below = if r.c > 0 { r.c - 1 } else { 0 };
    if r.c > 0 {
        assert(min[below] < r.v_max, "F17");
    }
    let above = if r.d + 1 < N { r.d + 1 } else { r.d };
    if r.d + 1 < N {
        assert(min[above] < r.v_max, "F17");
    }

    let p = r.p_star;
    assert(surp_b[p] == if r.side { 0 } else { r.v_min_delta }, "F20");
    assert(surp_a[p] == if r.side { r.v_min_delta } else { 0 }, "F20");

    let previous = if p > 0 { p - 1 } else { 0 };
    if (p > r.c) & (r.side | (r.v_min_delta == 0)) {
        assert(delta[previous] > r.v_min_delta, "F21");
    }

    if r.v_min_delta > 0 {
        let at_price = if r.side { book.asks[p] } else { book.bids[p] };
        assert(at_price as u64 == r.p_at_price, "F32");
        let eligible_io = if r.side { io_acc_b[p] } else { io_acc_a[p] };
        assert(eligible_io == r.j_io, "F33");
    }
}

// Runs the clearing rule. Its result is not trusted: `clearing` checks it.
unconstrained fn find_receipt<let N: u32>(book: Book<N>) -> Receipt {
    let acc_b = demand(book.bids);
    let acc_a = supply(book.asks);

    // Maximum executable volume and the ticks that attain it.
    let mut v_max: u64 = 0;
    for i in 0..N {
        let m = smaller(acc_b[i], acc_a[i]);
        if m > v_max {
            v_max = m;
        }
    }
    let mut c = 0;
    let mut d = 0;
    let mut found = false;
    for i in 0..N {
        if smaller(acc_b[i], acc_a[i]) == v_max {
            if !found {
                c = i;
                found = true;
            }
            d = i;
        }
    }

    // Minimum imbalance on the plateau.
    let mut delta = [0 as u64; N];
    for i in 0..N {
        delta[i] = if acc_a[i] > acc_b[i] { acc_a[i] - acc_b[i] } else { acc_b[i] - acc_a[i] };
    }
    let mut v = delta[c];
    for i in c..d + 1 {
        if delta[i] < v {
            v = delta[i];
        }
    }

    // Lowest minimizing tick whose at-price long volume covers the imbalance.
    let mut p = c;
    let mut side = false;
    let mut p_at_price: u64 = 0;
    let mut chosen = false;
    for i in c..d + 1 {
        if !chosen & (delta[i] == v) {
            let supply_long = acc_a[i] > acc_b[i];
            let resting = if supply_long { book.asks[i] } else { book.bids[i] };
            let at_price = resting as u64;
            if (v == 0) | (at_price >= v) {
                p = i;
                side = supply_long;
                p_at_price = if v == 0 { 0 } else { at_price };
                chosen = true;
            }
        }
    }

    // Short-side IO volume eligible at the clearing tick.
    let mut j_io: u64 = 0;
    if v > 0 {
        j_io = if side { demand(book.io_bids)[p] } else { supply(book.io_asks)[p] };
    }

    Receipt { v_max, c, d, p_star: p, v_min_delta: v, side, p_at_price, j_io }
}

// The clearing receipt of `book`, proved correct.
pub fn clearing<let N: u32>(book: Book<N>) -> Receipt {
    // Safety: `check_clearing` accepts only the correct receipt.
    let receipt = unsafe { find_receipt(book) };
    check_clearing(book, receipt);
    receipt
}

// Checks that no price crosses: demand or supply is zero at every tick.
pub fn check_no_cross<let N: u32>(book: Book<N>) {
    let acc_b = demand(book.bids);
    let acc_a = supply(book.asks);
    for i in 0..N {
        assert((acc_a[i] as Field) * (acc_b[i] as Field) == 0, "cross");
    }
}

// Bids 10 @ 5, asks 6 @ 2, IO sell 3 @ 4, IO buy 9 @ 6.
fn example_book() -> Book<8> {
    Book {
        bids: [0, 0, 0, 0, 0, 10, 0, 0],
        asks: [0, 0, 6, 0, 0, 0, 0, 0],
        io_bids: [0, 0, 0, 0, 0, 0, 9, 0],
        io_asks: [0, 0, 0, 0, 3, 0, 0, 0],
    }
}

fn example_receipt() -> Receipt {
    Receipt {
        v_max: 6,
        c: 2,
        d: 5,
        p_star: 5,
        v_min_delta: 4,
        side: false,
        p_at_price: 10,
        j_io: 3,
    }
}

// Bids 6 @ 5, asks 10 @ 2, IO buy 3 @ 4.
fn supply_long_book() -> Book<8> {
    Book {
        bids: [0, 0, 0, 0, 0, 6, 0, 0],
        asks: [0, 0, 10, 0, 0, 0, 0, 0],
        io_bids: [0, 0, 0, 0, 3, 0, 0, 0],
        io_asks: [0; 8],
    }
}

fn supply_long_receipt() -> Receipt {
    Receipt {
        v_max: 6,
        c: 2,
        d: 5,
        p_star: 2,
        v_min_delta: 4,
        side: true,
        p_at_price: 10,
        j_io: 3,
    }
}

// Bids 6 @ 5, asks 6 @ 2.
fn balanced_book() -> Book<8> {
    Book {
        bids: [0, 0, 0, 0, 0, 6, 0, 0],
        asks: [0, 0, 6, 0, 0, 0, 0, 0],
        io_bids: [0; 8],
        io_asks: [0; 8],
    }
}

fn balanced_receipt(p_star: u32) -> Receipt {
    Receipt { v_max: 6, c: 2, d: 5, p_star, v_min_delta: 0, side: false, p_at_price: 0, j_io: 0 }
}

// Bids 3 @ 0 and 7 @ 1, asks 6 @ 0 and 5 @ 1: one maximizing tick.
fn single_tick_book() -> Book<2> {
    Book { bids: [3, 7], asks: [6, 5], io_bids: [0, 2], io_asks: [1, 0] }
}

// Bids 5 @ 2, asks 7 @ 5.
fn quiet_book() -> Book<8> {
    Book {
        bids: [0, 0, 5, 0, 0, 0, 0, 0],
        asks: [0, 0, 0, 0, 0, 7, 0, 0],
        io_bids: [0; 8],
        io_asks: [0; 8],
    }
}

#[test]
fn finds_the_demand_long_receipt() {
    assert(clearing(example_book()) == example_receipt());
}

#[test]
fn finds_the_supply_long_receipt() {
    assert(clearing(supply_long_book()) == supply_long_receipt());
}

#[test]
fn finds_the_lowest_tick_of_a_balanced_book() {
    assert(clearing(balanced_book()) == balanced_receipt(2));
}

#[test]
fn finds_a_single_maximizing_tick() {
    // Demand 10, 7; supply 6, 11: volume 6 at tick 0, 7 at tick 1; supply long by 4 at tick 1.
    let expected = Receipt {
        v_max: 7,
        c: 1,
        d: 1,
        p_star: 1,
        v_min_delta: 4,
        side: true,
        p_at_price: 5,
        j_io: 2,
    };
    assert(clearing(single_tick_book()) == expected);
}

#[test(should_fail_with = "receipt")]
fn a_book_that_does_not_cross_has_no_receipt() {
    let _ = clearing(quiet_book());
}

#[test(should_fail_with = "receipt")]
fn rejects_the_lowest_tick_when_demand_is_long() {
    let mut receipt = example_receipt();
    receipt.p_star = 2;
    receipt.p_at_price = 0;
    check_clearing(example_book(), receipt);
}

#[test(should_fail_with = "F32")]
fn rejects_a_false_at_price_volume() {
    let mut receipt = example_receipt();
    receipt.p_star = 2;
    check_clearing(example_book(), receipt);
}

#[test(should_fail_with = "receipt")]
fn rejects_a_higher_tick_when_supply_is_long() {
    let mut receipt = supply_long_receipt();
    receipt.p_star = 5;
    receipt.p_at_price = 0;
    check_clearing(supply_long_book(), receipt);
}

#[test(should_fail_with = "F21")]
fn rejects_a_higher_tick_in_a_balanced_book() {
    check_clearing(balanced_book(), balanced_receipt(3));
}

#[test(should_fail_with = "F14")]
fn rejects_understated_volume() {
    let mut receipt = example_receipt();
    receipt.v_max = 5;
    check_clearing(example_book(), receipt);
}

#[test(should_fail_with = "F15")]
fn rejects_overstated_volume() {
    let mut receipt = example_receipt();
    receipt.v_max = 7;
    check_clearing(example_book(), receipt);
}

#[test(should_fail_with = "F17")]
fn rejects_a_plateau_shortened_on_the_left() {
    let mut receipt = example_receipt();
    receipt.c = 3;
    check_clearing(example_book(), receipt);
}

#[test(should_fail_with = "F17")]
fn rejects_a_plateau_shortened_on_the_right() {
    let mut receipt = supply_long_receipt();
    receipt.d = 4;
    check_clearing(supply_long_book(), receipt);
}

#[test(should_fail_with = "F19")]
fn rejects_an_overstated_imbalance() {
    let mut receipt = example_receipt();
    receipt.v_min_delta = 5;
    check_clearing(example_book(), receipt);
}

#[test(should_fail_with = "F20")]
fn rejects_an_understated_imbalance() {
    let mut receipt = example_receipt();
    receipt.v_min_delta = 3;
    check_clearing(example_book(), receipt);
}

#[test(should_fail_with = "F20")]
fn rejects_the_wrong_side() {
    let mut receipt = example_receipt();
    receipt.side = true;
    check_clearing(example_book(), receipt);
}

#[test(should_fail_with = "F33")]
fn rejects_inflated_io_volume() {
    let mut receipt = example_receipt();
    receipt.j_io = 8;
    check_clearing(example_book(), receipt);
}

#[test]
fn accepts_a_book_that_does_not_cross() {
    check_no_cross(quiet_book());
}

#[test(should_fail_with = "cross")]
fn rejects_no_cross_for_a_crossing_book() {
    check_no_cross(example_book());
}

#[test]
fn the_digest_depends_on_the_book_and_the_salt() {
    let book = example_book();
    let mut other = example_book();
    other.io_asks[4] = 4;
    assert(digest(book, 1) != digest(other, 1));
    assert(digest(book, 1) != digest(book, 2));
    assert(digest(book, 1) == digest(example_book(), 1));
}
