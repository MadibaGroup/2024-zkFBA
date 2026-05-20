// ZK-FBA -- MCV sub-circuit
// Constraint C5 + MCV check: min-exclusivity and market clearing volume.
//
// Rust analogue:
//   C5  -- Layer 3c quotient Q5 = V_KL / Z_H  (degree 30)
//           V_KL = (Acc_A - Min) * (Acc_B - Min)
//   MCV -- Layer 1 array computation: MCV = max(min_arr)
//
// Stand-alone circuit for isolated gate-count and timing analysis.
// The same logic is exposed as fba_mcv::check_mcv in the full
// zk_fba_noir package.

fn main(
    acc_a:   [u64; 21],  // ask depth at each price tick
    acc_b:   [u64; 21],  // bid depth at each price tick
    min_arr: [u64; 21],  // min(acc_a[i], acc_b[i]) at each tick

    // Public output
    mcv: pub u64,        // Market Clearing Volume = max(min_arr)
) {
    // C5 : V_KL  --  min-exclusivity at every tick
    for i in 0..21 {
        // Range bounds: prevent prover from using maximum instead of min.
        assert(min_arr[i] <= acc_a[i]);
        assert(min_arr[i] <= acc_b[i]);

        // Exclusivity: product of differences must be zero.
        // Subtraction is safe: bounds above guarantee both differences >= 0.
        let da: u64   = acc_a[i] - min_arr[i];
        let db: u64   = acc_b[i] - min_arr[i];
        let fa: Field = da as Field;
        let fb: Field = db as Field;
        assert(fa * fb == 0);
    }

    // MCV (a): mcv is an upper bound on every element of min_arr
    for i in 0..21 {
        assert(min_arr[i] <= mcv);
    }

    // MCV (b): mcv is achieved at least once in min_arr
    let mut found: bool = false;
    for i in 0..21 {
        found = found | (min_arr[i] == mcv);
    }
    assert(found);
}
