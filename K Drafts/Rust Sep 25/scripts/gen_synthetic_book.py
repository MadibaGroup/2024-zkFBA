#!/usr/bin/env python3
"""Generate a synthetic order-book CSV in the same 20-column format as
order_book_{100,1000}_log-normal.csv, for benchmarking at N ticks the
existing sample data doesn't cover (5000/10000/20000).

Only columns 0,1,2,3,4,5,8,9,10,11,14 (Price, Bids++, Asks++, Bid_Depth,
Ask_Depth, Min_Bid_Ask, Bid_Surplus, Ask_Surplus, Abs_Delta, Check_on_Delta,
MCV) are read/cross-checked by OrderBook::from_csv; the rest are filled with
0 placeholders to keep the column count at 20.

V_max (= MCV = max(min(AccA, AccB))) must stay under 2^16 = 65536 (the
RANGE_BITS bound checked in OrderBook::derive), so raw bid/ask sizes are
scaled down as N grows and then rescaled iteratively until the derived
V_max fits under a target ceiling.
"""
import csv
import random
import sys


def derive_v_max(b, a):
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
    return acc_a, acc_b, min_x, max(min_x)


def generate(n, target_v_max, seed):
    rng = random.Random(seed)
    avg_size = max(1.0, (target_v_max * 1.6) / n)
    b = [max(0, round(rng.lognormvariate(0.0, 0.6) * avg_size)) for _ in range(n)]
    a = [max(0, round(rng.lognormvariate(0.0, 0.6) * avg_size)) for _ in range(n)]

    # Iteratively rescale until derived V_max fits comfortably under target.
    for _ in range(30):
        acc_a, acc_b, min_x, v_max = derive_v_max(b, a)
        if v_max == 0:
            b = [x + 1 for x in b]
            a = [x + 1 for x in a]
            continue
        if v_max <= target_v_max:
            break
        ratio = target_v_max / v_max * 0.95
        b = [max(0, round(x * ratio)) for x in b]
        a = [max(0, round(x * ratio)) for x in a]
    else:
        raise RuntimeError("failed to converge V_max under target")

    acc_a, acc_b, min_x, v_max = derive_v_max(b, a)
    assert 0 < v_max < 65536, f"V_max {v_max} out of range"

    surp_b = [acc_b[i] - min_x[i] for i in range(n)]
    surp_a = [acc_a[i] - min_x[i] for i in range(n)]
    delta = [surp_a[i] + surp_b[i] for i in range(n)]

    plateau = [i for i in range(n) if min_x[i] == v_max]
    c, d = plateau[0], plateau[-1]
    assert plateau == list(range(c, d + 1)), "plateau not contiguous"
    v_min_delta = min(delta[i] for i in range(c, d + 1))
    chk_d = [1 if (c <= i <= d and delta[i] == v_min_delta) else 0 for i in range(n)]

    return b, a, acc_a, acc_b, min_x, surp_b, surp_a, delta, chk_d, v_max


def write_csv(path, n, target_v_max, seed):
    b, a, acc_a, acc_b, min_x, surp_b, surp_a, delta, chk_d, v_max = generate(
        n, target_v_max, seed
    )
    header = [
        "Price", "Bids++", "Asks++", "Bid_Depth", "Ask_Depth", "Min_Bid_Ask",
        "Selector_MCV", "ln0_MCV", "Bid_Surplus", "Ask_Surplus", "Abs_Delta",
        "Check_on_Delta", "S_Delta", "Selector_MCI", "MCV", "Cliff_Value",
        "Slack", "Is_MCV_Tie", "Is_Min_Delta", "Clearing_Price",
    ]
    with open(path, "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(header)
        for i in range(n):
            w.writerow([
                i, b[i], a[i], acc_b[i], acc_a[i], min_x[i],
                0, 0, surp_b[i], surp_a[i], delta[i], chk_d[i],
                0, 0, v_max, 0, 0, 0, 0, 0,
            ])
    print(f"wrote {path}: n={n} V_max={v_max}")


if __name__ == "__main__":
    sizes = [int(x) for x in sys.argv[1:]] or [5000, 10000, 20000]
    for n in sizes:
        out = f"/Users/kimiaesmaili/Downloads/order_book_{n}_log-normal.csv"
        write_csv(out, n, target_v_max=55000, seed=42 + n)
