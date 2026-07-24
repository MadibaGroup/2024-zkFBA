// ZK-FBA -- Full protocol circuit (protocol_constraints.md, constraints #1-#33)
// Author: Kimia Esmaili, Concordia University
//
// See ../PROTOCOL_DESIGN_AND_RESULTS.md for the design rationale (why
// Mask_P/Mask_V/Mask_C are computed rather than witnessed, why no manual
// bit-decomposition/Plookup gadgets are used) and benchmark results.

global N: u32 = 1000;
global N_MAX: u64 = 1_000_000_000; // order-size ceiling (stands in for Plookup table t_in, Section 3)

fn main(
    // -- Private witnesses (Section 1 column table) --
    b:      [u64; N],  // B(X)     Bids++
    a:      [u64; N],  // A(X)     Asks++
    acc_b:  [u64; N],  // AccB(X)  Bid Depth
    acc_a:  [u64; N],  // AccA(X)  Ask Depth
    min_x:  [u64; N],  // Min(X)   Min(Bid,Ask)
    surp_b: [u64; N],  // SurpB(X) Bid Surplus++
    surp_a: [u64; N],  // SurpA(X) Ask Surplus++
    delta:  [u64; N],  // Delta(X) Abs(Delta)
    slack_l: u64,       // Slack_L(X), opened at c-1 (only meaningful if c > 0)
    slack_r: u64,       // Slack_R(X), opened at d+1 (only meaningful if d < N-1)

    // -- Public scalars disclosed in the clearing receipt (Section 1) --
    v_max:       pub u64,  // V_max        global maximum executable volume
    v_min_delta: pub u64,  // V_min_delta  minimum imbalance inside the plateau
    c:           pub u32,  // c            first tick index of the plateau
    d:           pub u32,  // d            last tick index of the plateau
    p_star:      pub u32,  // p*           clearing price tick index
) {
    // Section 3 -- bid/ask range check (#1, #2)
    for i in 0..N {
        assert(b[i] <= N_MAX);
        assert(a[i] <= N_MAX);
    }

    // Section 4 -- accumulator init + transition (#3-#6)
    assert(acc_b[N - 1] == b[N - 1]);   // Demand init
    assert(acc_a[0] == a[0]);           // Supply init
    for i in 0..N - 1 {
        assert(acc_b[i] == acc_b[i + 1] + b[i]); // Demand transition
    }
    for i in 1..N {
        assert(acc_a[i] == acc_a[i - 1] + a[i]); // Supply transition
    }

    // Section 5 -- Min(X) mutual exclusivity + ceiling (#7, #8)
    for i in 0..N {
        assert(min_x[i] <= acc_a[i]);
        assert(min_x[i] <= acc_b[i]);
        let da: Field = (acc_a[i] - min_x[i]) as Field;
        let db: Field = (acc_b[i] - min_x[i]) as Field;
        assert(da * db == 0);      // mutual exclusivity
        assert(min_x[i] <= v_max); // ceiling
    }

    // Section 6/7 -- plateau Mask_P, endpoints, InMCV (#9-#14)
    // Mask_P/InMCV are computed from c, d rather than witnessed (see .md).
    assert(min_x[c] == v_max); // plateau left endpoint (#11)
    assert(min_x[d] == v_max); // plateau right endpoint (#12)

    // Section 8/9 -- surplus definition + Delta (#15-#17)
    for i in 0..N {
        assert(surp_b[i] + min_x[i] == acc_b[i]);
        assert(surp_a[i] + min_x[i] == acc_a[i]);
        assert(delta[i] == surp_a[i] + surp_b[i]);
    }

    // Section 10/11 -- ChkD, Mask_V, Delta floor, valley pin (#18-#24)
    // ChkD/Mask_V are computed rather than witnessed (see .md).
    let p_in_plateau: bool = (p_star >= c) & (p_star <= d);
    assert(p_in_plateau); // Mask_V containment (#22): p* must lie in [c,d]

    for i in 0..N {
        let in_plateau: bool = (i >= c) & (i <= d);
        if in_plateau {
            assert(delta[i] >= v_min_delta); // Delta floor (#23)
        }
    }
    assert(delta[p_star] == v_min_delta); // valley pin (#24)

    // Section 12 -- SD membership (#25, #26)
    // Redundant given the valley pin above; kept for constraint traceability.
    let sd_p: Field = delta[p_star] as Field;
    assert(sd_p * (sd_p - v_min_delta as Field) == 0);

    // Section 13 -- cliff exhaustiveness (#27-#32)
    // Guarded: no cliff tick exists at a domain boundary (c == 0 or d == N-1).
    let has_left_cliff: bool = c > 0;
    if has_left_cliff {
        let idx_l: u32 = c - 1;
        assert(slack_l + 1 + min_x[idx_l] == v_max); // left cliff (#29), Slack_L non-negativity (#31)
    }
    let has_right_cliff: bool = d < N - 1;
    if has_right_cliff {
        let idx_r: u32 = d + 1;
        assert(slack_r + 1 + min_x[idx_r] == v_max); // right cliff (#30), Slack_R non-negativity (#32)
    }
}
