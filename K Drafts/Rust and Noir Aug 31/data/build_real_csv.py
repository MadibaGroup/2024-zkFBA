#!/usr/bin/env python3
"""Turn raw Databento pulls (raw_quotes_AAPL.csv, raw_trades_AAPL.csv) into
two N=100 order-book CSVs in the exact 20-column format that
OrderBook::from_csv (Rust) and protocol_prover_gen.py (Noir) expect --
same header, same derived-column math as gen_synthetic_book.py.

Two datasets, two different real-data-to-(Bids++,Asks++) mappings, per the
approach agreed on for this project:

  order_book_real_quotes_100.csv
    100 successive mbp-10 top-of-book snapshots right at the open.
    Bids++ = bid_sz_00, Asks++ = ask_sz_00 at each snapshot. "Price" is a
    sequence index (event order), not a literal dollar price -- the same
    convention the synthetic generator already uses.

  order_book_real_trades_100.csv
    100 real executed trades (side in {B, A} only; the NASDAQ opening-cross
    prints are tagged side='N'/unclassified and are skipped -- see
    NOTES.md). Verified empirically against nearest quote: side='B' trades
    print at the ask (buyer-aggressor, "lifts the offer"), side='A' trades
    print at the bid (seller-aggressor, "hits the bid"). Each trade becomes
    one tick: its size goes into Bids++ if side=='B', into Asks++ if
    side=='A', 0 in the other column.

Both datasets must satisfy the same structural assumption the protocol
relies on (Min(X) = min(AccA, AccB) is unimodal, so its plateau of ties is
contiguous) -- this is checked here and will raise loudly if real data
violates it, rather than silently producing a bad CSV.
"""
import csv
import sys
from pathlib import Path

import pandas as pd

HERE = Path(__file__).parent
RANGE_BITS = 16   # must match zk_fba_csv_full/src/lib.rs RANGE_BITS
BIT_WIDTH = 32     # must match zk_fba_csv_full/src/lib.rs BIT_WIDTH

HEADER = [
    "Price", "Bids++", "Asks++", "Bid_Depth", "Ask_Depth", "Min_Bid_Ask",
    "Selector_MCV", "ln0_MCV", "Bid_Surplus", "Ask_Surplus", "Abs_Delta",
    "Check_on_Delta", "S_Delta", "Selector_MCI", "MCV", "Cliff_Value",
    "Slack", "Is_MCV_Tie", "Is_Min_Delta", "Clearing_Price",
]


def derive(b, a):
    n = len(b)
    acc_b = [0] * n
    acc_b[n - 1] = b[n - 1]
    for i in range(n - 2, -1, -1):
        acc_b[i] = acc_b[i + 1] + b[i]
    acc_a = [0] * n
    acc_a[0] = a[0]
    for i in range(1, n):
        acc_a[i] = acc_a[i - 1] + a[i]
    min_x = [min(acc_a[i], acc_b[i]) for i in range(n)]
    v_max = max(min_x)
    plateau = [i for i in range(n) if min_x[i] == v_max]
    c, d = plateau[0], plateau[-1]
    assert plateau == list(range(c, d + 1)), (
        f"plateau not contiguous ({len(plateau)} tied ticks, span {d - c + 1}) "
        "-- real data violated the protocol's Min(X) unimodality assumption"
    )
    surp_b = [acc_b[i] - min_x[i] for i in range(n)]
    surp_a = [acc_a[i] - min_x[i] for i in range(n)]
    delta = [surp_a[i] + surp_b[i] for i in range(n)]
    v_min_delta = min(delta[i] for i in range(c, d + 1))
    chk_d = [1 if (c <= i <= d and delta[i] == v_min_delta) else 0 for i in range(n)]
    return acc_a, acc_b, min_x, surp_b, surp_a, delta, chk_d, v_max, c, d


def check_bounds(label, b, a, acc_a, acc_b, v_max):
    assert 0 < v_max < (1 << RANGE_BITS), (
        f"{label}: V_max={v_max} out of range for RANGE_BITS={RANGE_BITS} "
        f"(must be in (0, {1 << RANGE_BITS}))"
    )
    biggest = max(max(b), max(a), max(acc_a), max(acc_b))
    assert biggest < (1 << BIT_WIDTH), (
        f"{label}: value {biggest} exceeds BIT_WIDTH={BIT_WIDTH} bound"
    )


def write_csv(path, b, a):
    n = len(b)
    acc_a, acc_b, min_x, surp_b, surp_a, delta, chk_d, v_max, c, d = derive(b, a)
    check_bounds(path.name, b, a, acc_a, acc_b, v_max)
    with open(path, "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(HEADER)
        for i in range(n):
            w.writerow([
                i, b[i], a[i], acc_b[i], acc_a[i], min_x[i],
                0, 0, surp_b[i], surp_a[i], delta[i], chk_d[i],
                0, 0, v_max, 0, 0, 0, 0, 0,
            ])
    print(f"wrote {path}: n={n} V_max={v_max} c={c} d={d} "
          f"(plateau width {d - c + 1})")


def build_quotes(n=100):
    q = pd.read_csv(HERE / "raw_quotes_AAPL.csv")
    q = q.head(n)
    assert len(q) == n, f"only {len(q)} quote rows available, need {n}"
    b = q["bid_sz_00"].astype(int).tolist()
    a = q["ask_sz_00"].astype(int).tolist()
    assert all(x > 0 for x in b) and all(x > 0 for x in a), (
        "zero-size top-of-book quote found; from_csv/derive can't handle a "
        "zero AccA seed at i=0 or zero AccB seed at i=n-1"
    )
    write_csv(HERE / "order_book_real_quotes_100.csv", b, a)


def build_trades(n=100):
    t = pd.read_csv(HERE / "raw_trades_AAPL.csv")
    t = t[t["side"].isin(["B", "A"])].sort_values("ts_recv").head(n)
    assert len(t) == n, f"only {len(t)} classified (B/A) trades available, need {n}"
    b, a = [], []
    for side, size in zip(t["side"], t["size"].astype(int)):
        if side == "B":   # buyer-initiated (lifts the ask) -> demand/bid pressure
            b.append(size); a.append(0)
        else:             # side == "A": seller-initiated (hits the bid) -> supply/ask pressure
            b.append(0); a.append(size)
    # AccB is seeded at b[n-1] and AccA at a[0]; both must be nonzero for the
    # accumulator base case in derive()/lib.rs to be meaningful. Real trade
    # order is fixed (it's the actual sequence of prints), so if either end
    # tick happens to be 0 the CSV is still numerically valid (0 is a legal
    # size) but document rather than silently reorder real trade sequence.
    if b[-1] == 0 or a[0] == 0:
        print(f"note: trades dataset has b[n-1]={b[-1]}, a[0]={a[0]} "
              "(one-sided trade at a sequence endpoint -- still valid, just flagged)")
    write_csv(HERE / "order_book_real_trades_100.csv", b, a)


if __name__ == "__main__":
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 100
    build_quotes(n)
    build_trades(n)
