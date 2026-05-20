// fba_mcv.nr
// Constraint C5 + MCV check: min-exclusivity and market clearing volume.
//
// Rust analogue:
//   C5   -- V_KL vanishing polynomial (Layer 3c, quotient Q5, degree 30)
//   MCV  -- Layer 1 array computation (max of min_arr)
//
// Array-level semantics:
//
//   C5   (acc_a[i] - min_arr[i]) * (acc_b[i] - min_arr[i]) = 0  for all i
//        min_arr[i] <= acc_a[i]  and  min_arr[i] <= acc_b[i]     (bounds)
//
//   MCV  min_arr[i] <= mcv  for all i        (mcv is an upper bound)
//        exists j  s.t.  min_arr[j] == mcv   (upper bound is achieved)
//
// The two checks together uniquely characterise mcv as max(min_arr).

pub fn check_mcv(
    acc_a:   [u64; 21],  // ask depth at each price tick
    acc_b:   [u64; 21],  // bid depth at each price tick
    min_arr: [u64; 21],  // min(acc_a[i], acc_b[i]) at each tick
    mcv:     u64,        // claimed Market Clearing Volume = max(min_arr)
) {
    // C5 : V_KL  --  min-exclusivity at every tick
    for i in 0..21 {
        // Range bounds: prevent prover from using the maximum instead of min.
        assert(min_arr[i] <= acc_a[i]);
        assert(min_arr[i] <= acc_b[i]);

        // Exclusivity: at least one depth curve equals min_arr at this tick.
        // Subtraction is safe: bounds above guarantee both differences >= 0.
        // Cast to Field so the multiplication uses a single mul-gate rather
        // than u64 wide-product logic.
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

    // MCV (b): mcv is achieved at least once -- it equals min_arr at some tick
    let mut found: bool = false;
    for i in 0..21 {
        found = found | (min_arr[i] == mcv);
    }
    assert(found);
}
