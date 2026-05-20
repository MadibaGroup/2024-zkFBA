// fba_accumulators.nr
// Constraints C1-C4: ask-depth and bid-depth accumulator correctness.
//
// Rust analogue: the four vanishing polynomials V_AccA_init, V_AccA_rec,
// V_AccB_init, V_AccB_rec whose quotient polynomials Q1..Q4 are computed
// in Layer 3c of the Rust pipeline.
//
// Array-level semantics (matches Rust polynomial form):
//
//   C1  acc_a[0]       == ask[0]                   (V_AccA_init)
//   C2  acc_a[i]       == acc_a[i-1] + ask[i]      (V_AccA_rec, i = 1..20)
//   C3  acc_b[20]      == bid[20]                   (V_AccB_init)
//   C4  acc_b[i]       == acc_b[i+1] + bid[i]      (V_AccB_rec, i = 0..19)
//
// The domain skip points (w^20 for ask, w^31 for bid) in the polynomial
// form vanish naturally here: the forward recurrence ends at index 20 and
// the backward recurrence ends at index 19.

pub fn check_accumulators(
    bid:   [u64; 21],   // raw bid volume at each price tick
    ask:   [u64; 21],   // raw ask volume at each price tick
    acc_b: [u64; 21],   // bid depth = backward cumulative sum of bid
    acc_a: [u64; 21],   // ask depth = forward  cumulative sum of ask
) {
    // C1 : V_AccA_init  --  forward accumulator seeded at index 0
    assert(acc_a[0] == ask[0]);

    // C2 : V_AccA_rec  --  each ask-depth = previous + current ask
    for i in 1..21 {
        assert(acc_a[i] == acc_a[i - 1] + ask[i]);
    }

    // C3 : V_AccB_init  --  backward accumulator seeded at index 20
    assert(acc_b[20] == bid[20]);

    // C4 : V_AccB_rec  --  each bid-depth = next + current bid
    for i in 0..20 {
        assert(acc_b[i] == acc_b[i + 1] + bid[i]);
    }
}
