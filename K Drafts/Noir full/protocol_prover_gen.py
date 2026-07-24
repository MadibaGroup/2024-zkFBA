#!/usr/bin/env python3
# Builds Prover.toml witness data for the full-protocol Noir circuits
# (fba_protocol_100 / fba_protocol_1000) directly from raw CSV price-tick
# data, following the notation and derivations of protocol_constraints.md.
#
# IMPORTANT: only Price / Bids++ (B) / Asks++ (A) columns are treated as
# ground truth. Every other column (AccB, AccA, Min, SurpB, SurpA, Delta,
# SlackL, SlackR, V_max, V_min_delta, c, d, p_star) is *recomputed here*
# from B/A, exactly mirroring what the Noir circuit itself re-derives and
# checks. The CSV's own precomputed columns (Selector_MCV, ln0_MCV,
# Bid_Surplus, Check_on_Delta, S_Delta, Selector_MCI, MCV, Clearing_Price,
# ...) are used only as an independent cross-check, never as circuit input.

import csv
import sys

def load_book(path):
    with open(path, newline="") as f:
        rows = list(csv.DictReader(f))
    prices = [int(r["Price"]) for r in rows]
    b = [int(r["Bids++"]) for r in rows]   # B(X)  -- raw bid volume
    a = [int(r["Asks++"]) for r in rows]   # A(X)  -- raw ask volume
    csv_cross_check = rows
    return prices, b, a, csv_cross_check

def derive(b, a):
    n = len(b)

    # Section 4 -- AccB(X): backward cumulative demand, seeded at the
    # highest tick (index n-1), summing downward.
    acc_b = [0] * n
    acc_b[n - 1] = b[n - 1]
    for i in range(n - 2, -1, -1):
        acc_b[i] = acc_b[i + 1] + b[i]

    # Section 4 -- AccA(X): forward cumulative supply, seeded at the
    # lowest tick (index 0), summing upward.
    acc_a = [0] * n
    acc_a[0] = a[0]
    for i in range(1, n):
        acc_a[i] = acc_a[i - 1] + a[i]

    # Section 5 -- Min(X) = min(AccA(X), AccB(X))
    min_x = [min(acc_a[i], acc_b[i]) for i in range(n)]

    # V_max: global maximum executable volume (the plateau height)
    v_max = max(min_x)

    # Plateau [c, d]: contiguous run of ticks where Min(X) == V_max.
    # Min(X) is single-peaked (Section 4 corollary), so this run is
    # contiguous and c/d are simply its first/last index.
    plateau_idx = [i for i in range(n) if min_x[i] == v_max]
    c, d = plateau_idx[0], plateau_idx[-1]
    assert plateau_idx == list(range(c, d + 1)), \
        "plateau is not contiguous -- Min(X) unimodality assumption violated"

    # Section 8 -- SurpB(X) = AccB(X) - Min(X), SurpA(X) = AccA(X) - Min(X)
    surp_b = [acc_b[i] - min_x[i] for i in range(n)]
    surp_a = [acc_a[i] - min_x[i] for i in range(n)]

    # Section 9 -- Delta(X) = SurpA(X) + SurpB(X) = |AccA(X) - AccB(X)|
    delta = [surp_a[i] + surp_b[i] for i in range(n)]

    # V_min_delta: minimum imbalance inside the plateau (the valley depth)
    v_min_delta = min(delta[i] for i in range(c, d + 1))

    # p*: clearing price tick = first plateau tick achieving V_min_delta
    p_star = next(i for i in range(c, d + 1) if delta[i] == v_min_delta)

    # Section 13 -- cliff slack (only defined where a cliff tick exists)
    slack_l = (v_max - 1 - min_x[c - 1]) if c > 0 else 0
    slack_r = (v_max - 1 - min_x[d + 1]) if d < n - 1 else 0
    has_left_cliff = c > 0
    has_right_cliff = d < n - 1

    return {
        "n": n, "acc_b": acc_b, "acc_a": acc_a, "min_x": min_x,
        "v_max": v_max, "c": c, "d": d,
        "surp_b": surp_b, "surp_a": surp_a, "delta": delta,
        "v_min_delta": v_min_delta, "p_star": p_star,
        "slack_l": slack_l, "slack_r": slack_r,
        "has_left_cliff": has_left_cliff, "has_right_cliff": has_right_cliff,
    }

def cross_check(rows, der):
    """Sanity-check our from-scratch derivation against the CSV's own
    precomputed columns. Never used as circuit input -- just a guard
    against a preprocessing bug before we burn time compiling/proving."""
    n = der["n"]
    errs = []
    for i, r in enumerate(rows):
        if int(r["Bid_Depth"]) != der["acc_b"][i]:
            errs.append(f"row {i}: Bid_Depth mismatch")
        if int(r["Ask_Depth"]) != der["acc_a"][i]:
            errs.append(f"row {i}: Ask_Depth mismatch")
        if int(r["Min_Bid_Ask"]) != der["min_x"][i]:
            errs.append(f"row {i}: Min_Bid_Ask mismatch")
        if int(r["Bid_Surplus"]) != der["surp_b"][i]:
            errs.append(f"row {i}: Bid_Surplus mismatch")
        if int(r["Ask_Surplus"]) != der["surp_a"][i]:
            errs.append(f"row {i}: Ask_Surplus mismatch")
        if int(r["Abs_Delta"]) != der["delta"][i]:
            errs.append(f"row {i}: Abs_Delta mismatch")
    if int(rows[der["c"]]["Price"]) - int(rows[0]["Price"]) != der["c"]:
        pass  # price spacing is 1 tick/row in these CSVs; index check only
    csv_mcv = int(rows[0]["MCV"])
    if csv_mcv != der["v_max"]:
        errs.append(f"V_max mismatch: derived {der['v_max']} vs CSV MCV {csv_mcv}")
    if errs:
        raise AssertionError(f"{len(errs)} cross-check mismatches, e.g.: {errs[:5]}")
    print(f"  cross-check OK against CSV precomputed columns ({n} rows)")

def toml_array(name, values):
    return f"{name} = [{', '.join(str(v) for v in values)}]\n"

def write_prover_toml(path, b, a, der):
    n = der["n"]
    with open(path, "w") as f:
        f.write(f"# Auto-generated by protocol_prover_gen.py -- n={n} price ticks\n")
        f.write("# Private witnesses (protocol_constraints.md column table, Section 1)\n")
        f.write(toml_array("b", b))                # B(X)      Bids++
        f.write(toml_array("a", a))                # A(X)      Asks++
        f.write(toml_array("acc_b", der["acc_b"]))  # AccB(X)   Bid Depth
        f.write(toml_array("acc_a", der["acc_a"]))  # AccA(X)   Ask Depth
        f.write(toml_array("min_x", der["min_x"]))  # Min(X)    Min(Bid,Ask)
        f.write(toml_array("surp_b", der["surp_b"])) # SurpB(X) Bid Surplus++
        f.write(toml_array("surp_a", der["surp_a"])) # SurpA(X) Ask Surplus++
        f.write(toml_array("delta", der["delta"]))   # Delta(X) Abs(Delta)
        f.write(f"slack_l = {der['slack_l']}\n")      # Slack_L  (scalar: only active at c-1)
        f.write(f"slack_r = {der['slack_r']}\n")      # Slack_R  (scalar: only active at d+1)
        f.write("\n# Public scalars disclosed in the clearing receipt (Section 1)\n")
        f.write(f"v_max = {der['v_max']}\n")
        f.write(f"v_min_delta = {der['v_min_delta']}\n")
        f.write(f"c = {der['c']}\n")
        f.write(f"d = {der['d']}\n")
        f.write(f"p_star = {der['p_star']}\n")

def main():
    jobs = [
        ("/Users/kimiaesmaili/Downloads/order_book_100_log-normal.csv",
         "/Users/kimiaesmaili/zk_fba_noir/fba_protocol_100/Prover.toml"),
        ("/Users/kimiaesmaili/Downloads/order_book_1000_log-normal.csv",
         "/Users/kimiaesmaili/zk_fba_noir/fba_protocol_1000/Prover.toml"),
    ]
    for csv_path, out_path in jobs:
        print(f"[{csv_path}]")
        prices, b, a, rows = load_book(csv_path)
        der = derive(b, a)
        cross_check(rows, der)
        print(f"  n={der['n']}  V_max={der['v_max']}  c={der['c']}  d={der['d']}  "
              f"V_min_delta={der['v_min_delta']}  p*={der['p_star']}  "
              f"left_cliff={der['has_left_cliff']}  right_cliff={der['has_right_cliff']}")
        write_prover_toml(out_path, b, a, der)
        print(f"  wrote {out_path}\n")

if __name__ == "__main__":
    main()
