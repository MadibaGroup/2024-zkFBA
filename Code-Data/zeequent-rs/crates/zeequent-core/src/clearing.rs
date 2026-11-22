//! The clearing rule and the witness columns.

use zeequent_types::{Book, Order, OrderKind, OrderSide, Outcome, Params, Receipt, Side};

/// Every value column of the proof, in evaluation form over the padded domain (length `n`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Witness {
    /// `B`: raw bid volume.
    pub b: Vec<u64>,
    /// `A`: raw ask volume.
    pub a: Vec<u64>,
    /// `AccB`: cumulative demand, summed from the top tick down.
    pub acc_b: Vec<u64>,
    /// `AccA`: cumulative supply, summed from the bottom tick up.
    pub acc_a: Vec<u64>,
    /// `Min`: executable volume.
    pub min: Vec<u64>,
    /// `SurpB = AccB - Min`.
    pub surp_b: Vec<u64>,
    /// `SurpA = AccA - Min`.
    pub surp_a: Vec<u64>,
    /// `Delta = SurpA + SurpB`.
    pub delta: Vec<u64>,
    /// `Slack`: strictness witness for the cliffs (F17) and the tie-break (F21).
    pub slack: Vec<u64>,
    /// `IOB`: raw imbalance-only bid volume.
    pub io_b: Vec<u64>,
    /// `IOA`: raw imbalance-only ask volume.
    pub io_a: Vec<u64>,
    /// `IOAccB`: IO bids with limit at or above the tick.
    pub io_acc_b: Vec<u64>,
    /// `IOAccA`: IO asks with limit at or below the tick.
    pub io_acc_a: Vec<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClearError {
    /// The book has a different number of ticks than the parameters.
    ShapeMismatch { book_ticks: usize, n_ticks: usize },
    /// A tick's volume does not fit in `k_in` bits.
    VolumeTooLarge { tick: usize },
    /// An internal invariant failed.
    Internal(&'static str),
}

fn pad(column: &[u64], n: usize) -> Vec<u64> {
    let mut out = column.to_vec();
    out.resize(n, 0);
    out
}

/// `out[i] = sum of column[i..]` (demand-style accumulator).
fn suffix_sum(column: &[u64]) -> Vec<u64> {
    let mut out = vec![0u64; column.len()];
    let mut running = 0u64;
    for i in (0..column.len()).rev() {
        running += column[i];
        out[i] = running;
    }
    out
}

/// `out[i] = sum of column[..=i]` (supply-style accumulator).
fn prefix_sum(column: &[u64]) -> Vec<u64> {
    let mut out = vec![0u64; column.len()];
    let mut running = 0u64;
    for i in 0..column.len() {
        running += column[i];
        out[i] = running;
    }
    out
}

/// Runs the clearing rule on a book and returns the public outcome together with the witness columns.
pub fn clear(book: &Book, params: &Params) -> Result<(Outcome, Witness), ClearError> {
    let n = params.n;
    let n_ticks = params.n_ticks;
    if book.n_ticks() != n_ticks {
        return Err(ClearError::ShapeMismatch {
            book_ticks: book.n_ticks(),
            n_ticks,
        });
    }
    let bound = params.input_bound();
    for tick in 0..n_ticks {
        if book.bids[tick] >= bound
            || book.asks[tick] >= bound
            || book.io_bids[tick] >= bound
            || book.io_asks[tick] >= bound
        {
            return Err(ClearError::VolumeTooLarge { tick });
        }
    }

    // Inputs, padded with empty ticks.
    let b = pad(&book.bids, n);
    let a = pad(&book.asks, n);
    let io_b = pad(&book.io_bids, n);
    let io_a = pad(&book.io_asks, n);

    // Phase 1: depth and executable volume.
    let acc_b = suffix_sum(&b);
    let acc_a = prefix_sum(&a);
    let io_acc_b = suffix_sum(&io_b);
    let io_acc_a = prefix_sum(&io_a);

    let min: Vec<u64> = (0..n).map(|i| acc_b[i].min(acc_a[i])).collect();
    let surp_b: Vec<u64> = (0..n).map(|i| acc_b[i] - min[i]).collect();
    let surp_a: Vec<u64> = (0..n).map(|i| acc_a[i] - min[i]).collect();
    let delta: Vec<u64> = (0..n).map(|i| surp_a[i] + surp_b[i]).collect();
    let mut slack = vec![0u64; n];

    let v_max = min.iter().copied().max().unwrap_or(0);
    if v_max == 0 {
        let witness = Witness {
            b,
            a,
            acc_b,
            acc_a,
            min,
            surp_b,
            surp_a,
            delta,
            slack,
            io_b,
            io_a,
            io_acc_b,
            io_acc_a,
        };
        return Ok((Outcome::NoCross, witness));
    }

    // The plateau: Min is single-peaked, so the maximizers are contiguous.
    let c = min
        .iter()
        .position(|&m| m == v_max)
        .ok_or(ClearError::Internal("no maximizing tick"))?;
    let d = min
        .iter()
        .rposition(|&m| m == v_max)
        .ok_or(ClearError::Internal("no maximizing tick"))?;
    if (c..=d).any(|i| min[i] != v_max) {
        return Err(ClearError::Internal("plateau is not contiguous"));
    }
    if d >= n_ticks {
        return Err(ClearError::Internal("plateau reaches padding"));
    }

    // Phase 2: minimum imbalance over the plateau, then the tertiary rule.
    let v = (c..=d)
        .map(|i| delta[i])
        .min()
        .ok_or(ClearError::Internal("empty plateau"))?;

    let mut chosen: Option<(usize, Side, u64)> = None;
    for i in c..=d {
        if delta[i] != v {
            continue;
        }
        if v == 0 {
            chosen = Some((i, Side::DemandLong, 0));
            break;
        }
        let (side, at_price) = if surp_b[i] > 0 {
            (Side::DemandLong, b[i])
        } else {
            (Side::SupplyLong, a[i])
        };
        if at_price >= v {
            chosen = Some((i, side, at_price));
            break;
        }
    }
    let (p_star, side, p_at_price) =
        chosen.ok_or(ClearError::Internal("no tick satisfies the tie-break rule"))?;

    // Phase 3: short-side IO volume eligible at p*.
    let j_io = if v == 0 {
        0
    } else {
        match side {
            // Demand long: supply is short, so sell IOs with limit <= p*.
            Side::DemandLong => io_acc_a[p_star],
            // Supply long: demand is short, so buy IOs with limit >= p*.
            Side::SupplyLong => io_acc_b[p_star],
        }
    };

    // Slack for the cliffs (F17).
    if c >= 1 {
        slack[c - 1] = (v_max - min[c - 1])
            .checked_sub(1)
            .ok_or(ClearError::Internal("left cliff is not strict"))?;
    }
    if d + 1 < n {
        slack[d + 1] = (v_max - min[d + 1])
            .checked_sub(1)
            .ok_or(ClearError::Internal("right cliff is not strict"))?;
    }
    // Slack for the tie-break (F21), active when supply is long or balanced.
    if p_star > c && (side == Side::SupplyLong || v == 0) {
        slack[p_star - 1] = delta[p_star - 1]
            .checked_sub(v)
            .and_then(|gap| gap.checked_sub(1))
            .ok_or(ClearError::Internal("a lower tie-break candidate exists"))?;
    }

    let receipt = Receipt {
        v_max,
        c,
        d,
        p_star,
        v_min_delta: v,
        side,
        p_at_price,
        j_io,
    };
    let witness = Witness {
        b,
        a,
        acc_b,
        acc_a,
        min,
        surp_b,
        surp_a,
        delta,
        slack,
        io_b,
        io_a,
        io_acc_b,
        io_acc_a,
    };
    Ok((Outcome::Cross(receipt), witness))
}

fn pro_rata(qty: u64, numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 {
        return 0;
    }
    ((qty as u128 * numerator as u128) / denominator as u128) as u64
}

/// Computes an order’s entitlement from its terms and the public receipt, truncating fractional units.
pub fn fill(receipt: &Receipt, n_ticks: usize, order: &Order) -> u64 {
    let p = receipt.p_star;
    let imbalance = receipt.has_imbalance();
    let rationed = |qty: u64| {
        pro_rata(
            qty,
            receipt.p_at_price - receipt.residual(),
            receipt.p_at_price,
        )
    };
    let io_share = |qty: u64| pro_rata(qty, receipt.admitted_io(), receipt.j_io);

    match (order.side, order.kind) {
        (OrderSide::Buy, OrderKind::Limit { .. }) | (OrderSide::Buy, OrderKind::Market) => {
            let tick = match order.kind {
                OrderKind::Limit { tick } => tick,
                _ => n_ticks - 1,
            };
            if tick > p {
                order.qty
            } else if tick == p {
                if imbalance && receipt.side == Side::DemandLong {
                    rationed(order.qty)
                } else {
                    order.qty
                }
            } else {
                0
            }
        }
        (OrderSide::Sell, OrderKind::Limit { .. }) | (OrderSide::Sell, OrderKind::Market) => {
            let tick = match order.kind {
                OrderKind::Limit { tick } => tick,
                _ => 0,
            };
            if tick < p {
                order.qty
            } else if tick == p {
                if imbalance && receipt.side == Side::SupplyLong {
                    rationed(order.qty)
                } else {
                    order.qty
                }
            } else {
                0
            }
        }
        // A buy IO helps only when supply is long, and only if its limit is at or above p*.
        (OrderSide::Buy, OrderKind::Io { tick }) => {
            if imbalance && receipt.side == Side::SupplyLong && tick >= p {
                io_share(order.qty)
            } else {
                0
            }
        }
        // A sell IO helps only when demand is long, and only if its limit is at or below p*.
        (OrderSide::Sell, OrderKind::Io { tick }) => {
            if imbalance && receipt.side == Side::DemandLong && tick <= p {
                io_share(order.qty)
            } else {
                0
            }
        }
    }
}
