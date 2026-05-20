// ZK-FBA -- accumulator sub-circuit
// Constraints C1-C4 only: ask-depth and bid-depth correctness.
//
// Rust analogue: Layer 3c quotient polynomials Q1..Q4
//   V_AccA_init  Q1 = (Acc_A - Ask) / (X - w^0)
//   V_AccA_rec   Q2 = (X-w^20)(Acc_A(wX)-Acc_A(X)-Ask(wX)) / Z_H
//   V_AccB_init  Q3 = (Acc_B - Bid) / (X - w^20)
//   V_AccB_rec   Q4 = (X-w^31)(Acc_B(X)-Acc_B(wX)-Bid(X)) / Z_H
//
// Stand-alone circuit for isolated gate-count and timing analysis.
// The same logic is exposed as fba_accumulators::check_accumulators
// in the full zk_fba_noir package.

fn main(
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
