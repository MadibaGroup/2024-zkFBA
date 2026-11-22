//! Pointwise check of constraints F1-F33 on the witness columns, over the integers.

use crate::clearing::Witness;
use zeequent_types::{Outcome, Params, Receipt};

/// A constraint that does not hold, and where.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Violation {
    /// Constraint label: `"F8"`, `"scalar"` for a receipt check, `"shape"` for a malformed witness.
    pub constraint: &'static str,
    pub tick: Option<usize>,
}

fn at(constraint: &'static str, tick: usize) -> Violation {
    Violation {
        constraint,
        tick: Some(tick),
    }
}

fn global(constraint: &'static str) -> Violation {
    Violation {
        constraint,
        tick: None,
    }
}

fn in_range(value: i128, bound: u64) -> bool {
    value >= 0 && value < bound as i128
}

/// The verifier's checks on the public receipt, before any polynomial check.
pub fn check_receipt_scalars(params: &Params, r: &Receipt) -> Result<(), Violation> {
    let bound = params.bound();
    let ok = r.c <= r.p_star
        && r.p_star <= r.d
        && r.d < params.n_ticks
        && r.v_max >= 1
        && r.v_max < bound
        && r.v_min_delta < bound
        && r.p_at_price < bound
        && r.j_io < bound
        && r.v_min_delta <= r.p_at_price
        && (r.v_min_delta > 0 || (r.side.bit() == 0 && r.p_at_price == 0 && r.j_io == 0));
    if ok {
        Ok(())
    } else {
        Err(global("scalar"))
    }
}

/// Input range, padding, and the two accumulators for one bid/ask column pair.
#[allow(clippy::too_many_arguments)]
fn check_inputs_and_depth(
    params: &Params,
    bids: &[u64],
    asks: &[u64],
    acc_bids: &[u64],
    acc_asks: &[u64],
    labels: [&'static str; 8],
) -> Result<(), Violation> {
    let n = params.n;
    let [range_b, range_a, pad_b, pad_a, init_b, init_a, trans_b, trans_a] = labels;
    let input_bound = params.input_bound();

    for i in 0..n {
        if bids[i] >= input_bound {
            return Err(at(range_b, i));
        }
        if asks[i] >= input_bound {
            return Err(at(range_a, i));
        }
        if i >= params.n_ticks {
            if bids[i] != 0 {
                return Err(at(pad_b, i));
            }
            if asks[i] != 0 {
                return Err(at(pad_a, i));
            }
        }
    }

    // Demand: initialised at the last domain point, AccB[i] = AccB[i+1] + B[i].
    if acc_bids[n - 1] != bids[n - 1] {
        return Err(at(init_b, n - 1));
    }
    // Supply: initialised at the first domain point, AccA[i+1] = AccA[i] + A[i+1].
    if acc_asks[0] != asks[0] {
        return Err(at(init_a, 0));
    }
    for i in 0..n - 1 {
        if acc_bids[i] as i128 != acc_bids[i + 1] as i128 + bids[i] as i128 {
            return Err(at(trans_b, i));
        }
        if acc_asks[i + 1] as i128 != acc_asks[i] as i128 + asks[i + 1] as i128 {
            return Err(at(trans_a, i));
        }
    }
    Ok(())
}

/// F9-F14: `Min` is the pointwise minimum of the depth curves and never exceeds `v_max`.
fn check_min(params: &Params, w: &Witness, v_max: u64) -> Result<(), Violation> {
    let bound = params.bound();
    for i in 0..params.n {
        if w.surp_b[i] as i128 != w.acc_b[i] as i128 - w.min[i] as i128 {
            return Err(at("F9", i));
        }
        if w.surp_a[i] as i128 != w.acc_a[i] as i128 - w.min[i] as i128 {
            return Err(at("F10", i));
        }
        if w.surp_a[i] != 0 && w.surp_b[i] != 0 {
            return Err(at("F11", i));
        }
        if w.surp_b[i] >= bound {
            return Err(at("F12", i));
        }
        if w.surp_a[i] >= bound {
            return Err(at("F13", i));
        }
        if !in_range(v_max as i128 - w.min[i] as i128, bound) {
            return Err(at("F14", i));
        }
    }
    Ok(())
}

fn check_shape(params: &Params, w: &Witness) -> Result<(), Violation> {
    let n = params.n;
    let columns: [&Vec<u64>; 13] = [
        &w.b,
        &w.a,
        &w.acc_b,
        &w.acc_a,
        &w.min,
        &w.surp_b,
        &w.surp_a,
        &w.delta,
        &w.slack,
        &w.io_b,
        &w.io_a,
        &w.io_acc_b,
        &w.io_acc_a,
    ];
    if n < 2 || columns.iter().any(|col| col.len() != n) {
        return Err(global("shape"));
    }
    Ok(())
}

/// Checks every constraint for the given outcome.
pub fn check(params: &Params, outcome: &Outcome, w: &Witness) -> Result<(), Violation> {
    check_shape(params, w)?;
    let n = params.n;
    let bound = params.bound();

    check_inputs_and_depth(
        params,
        &w.b,
        &w.a,
        &w.acc_b,
        &w.acc_a,
        ["F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8"],
    )?;

    let r = match outcome {
        Outcome::NoCross => {
            return check_min(params, w, 0);
        }
        Outcome::Cross(r) => r,
    };

    check_receipt_scalars(params, r)?;
    check_min(params, w, r.v_max)?;

    let v = r.v_min_delta as i128;
    let s = r.side.bit();
    let (c, d, p) = (r.c, r.d, r.p_star);

    // F15, F16: plateau endpoints.
    if w.min[c] != r.v_max {
        return Err(at("F15", c));
    }
    if w.min[d] != r.v_max {
        return Err(at("F16", d));
    }

    // F17: strict cliffs, at the cliff ticks that exist (Mask_C).
    let cliff = |i: usize| r.v_max as i128 - w.min[i] as i128 - 1 - w.slack[i] as i128;
    if c >= 1 && cliff(c - 1) != 0 {
        return Err(at("F17", c - 1));
    }
    if d + 1 < n && cliff(d + 1) != 0 {
        return Err(at("F17", d + 1));
    }

    for i in 0..n {
        // F18: imbalance definition.
        if w.delta[i] as i128 != w.surp_a[i] as i128 + w.surp_b[i] as i128 {
            return Err(at("F18", i));
        }
        // F22: slack is non-negative.
        if w.slack[i] >= bound {
            return Err(at("F22", i));
        }
    }

    // F19: floor on the plateau (Mask_P).
    for (i, delta) in w.delta.iter().enumerate().take(d + 1).skip(c) {
        if !in_range(*delta as i128 - v, bound) {
            return Err(at("F19", i));
        }
    }

    // F20a, F20b: valley pin on the disclosed side.
    if w.surp_b[p] as i128 != (1 - s as i128) * v {
        return Err(at("F20a", p));
    }
    if w.surp_a[p] as i128 != s as i128 * v {
        return Err(at("F20b", p));
    }

    // F21: tie-break (Mask_T), active when p* > c and (s = 1 or no imbalance).
    if p > c && (s == 1 || r.v_min_delta == 0) {
        let gap = w.delta[p - 1] as i128 - v - 1 - w.slack[p - 1] as i128;
        if gap != 0 {
            return Err(at("F21", p - 1));
        }
    }

    // F24-F31: the IO book.
    check_inputs_and_depth(
        params,
        &w.io_b,
        &w.io_a,
        &w.io_acc_b,
        &w.io_acc_a,
        ["F24", "F25", "F26", "F27", "F28", "F30", "F29", "F31"],
    )?;

    // F32, F33: Phase 3 pins, switched off when there is no imbalance.
    if r.has_imbalance() {
        let at_price = if s == 0 { w.b[p] } else { w.a[p] };
        if at_price != r.p_at_price {
            return Err(at("F32", p));
        }
        let eligible_io = if s == 0 {
            w.io_acc_a[p]
        } else {
            w.io_acc_b[p]
        };
        if eligible_io != r.j_io {
            return Err(at("F33", p));
        }
    }

    Ok(())
}
