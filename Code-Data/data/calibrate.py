#!/usr/bin/env python3
"""Measure, from the replayed real data, the statistics the synthetic generator needs."""
import argparse
import json
import sys
from pathlib import Path

import numpy as np

import replay

HERE = Path(__file__).parent
NEAR = 100                    # offsets measured tick by tick
FAR_EDGES = [101, 150, 200, 300, 500, 750, 1000, 1500, 2000, 3000, 5000, 7500, 10000]
QUANTILES = [0, 5, 10, 25, 50, 75, 90, 95, 99, 99.9, 100]
MAX_NEAR_SAMPLES = 20000      # batches kept per batch length for the near profile
FAR_EVERY = 25                # use every 25th crossing batch for the far bands
CORE = 5                      # offsets -CORE..CORE whose joint occupancy is kept
BOOT = 10                     # offsets -BOOT..BOOT whose real volumes are kept for the bootstrap model


def quantile_table(matrix):
    """Per column: presence, and quantiles of the non-zero entries."""
    presence, table = [], []
    for column in matrix.T:
        nonzero = column[column > 0]
        presence.append(float(len(nonzero)) / len(column))
        table.append([float(q) for q in np.percentile(nonzero, QUANTILES)] if len(nonzero) else [0.0] * len(QUANTILES))
    return presence, table


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--symbol", default="AAPL")
    parser.add_argument("--deltas", default="0.1,1,10,60")
    parser.add_argument("--include-flagged", action="store_true",
                        help="also use days that fail the screen")
    args = parser.parse_args()
    deltas = [float(x) for x in args.deltas.split(",")]
    files = sorted(replay.RAW.glob(f"{args.symbol}_*_mbo.dbn.zst"))
    if not files:
        sys.exit(f"no mbo files for {args.symbol}")

    # Days that fail the screen are skipped unless --include-flagged is given.
    flagged = {} if args.include_flagged else replay.day_screen()
    for path in files:
        day = path.name.split("_")[1]
        if day in flagged:
            print(f"skipping {day}: {flagged[day]}")
    files = [path for path in files if path.name.split("_")[1] not in flagged]
    days = {path.name.split("_")[1]: replay.load_day(path)[:2] for path in files}
    rng = np.random.default_rng(20261117)
    out = {
        "symbol": args.symbol, "dataset": "XNAS.ITCH (Nasdaq TotalView-ITCH) via Databento",
        "days": sorted(days), "tick": "0.01 USD", "near": NEAR, "far_edges": FAR_EDGES,
        "quantile_levels": QUANTILES, "by_delta": {},
    }

    library = {}
    for delta in deltas:
        near_bids, near_asks, skipped, outer = [], [], [0], []
        far = {"bid": [[] for _ in FAR_EDGES[:-1]], "ask": [[] for _ in FAR_EDGES[:-1]]}
        far_ticks = {"bid": [0] * (len(FAR_EDGES) - 1), "ask": [0] * (len(FAR_EDGES) - 1)}
        tallies = {"crossing": 0, "no_tie": 0, "plateau_unique_valley": 0, "plateau_tied_valley": 0}
        v_max, counter = [], [0]

        def hook(k, lo, b, a, result, bid, ask):
            tallies["crossing"] += 1
            if result["width"] == 1:
                tallies["no_tie"] += 1
            elif result["valley"] == 1:
                tallies["plateau_unique_valley"] += 1
            else:
                tallies["plateau_tied_valley"] += 1
            v_max.append(result["v_max"])
            p = result["p_star"]
            if p - NEAR < 0 or p + NEAR >= len(b):
                skipped[0] += 1
                return
            near_bids.append(b[p - NEAR:p + NEAR + 1].astype(np.int32))
            near_asks.append(a[p - NEAR:p + NEAR + 1].astype(np.int32))
            # Totals outside the bootstrap core: bids above it and asks below it.
            outer.append((int(b[p + BOOT + 1:].sum()), int(a[:p - BOOT].sum())))
            counter[0] += 1
            if counter[0] % FAR_EVERY == 0:
                centre = lo + p
                for j in range(len(FAR_EDGES) - 1):
                    e1, e2 = FAR_EDGES[j], FAR_EDGES[j + 1]
                    below = bid[max(0, centre - e2 + 1):max(0, centre - e1 + 1)]
                    above = ask[centre + e1:centre + e2]
                    far_ticks["bid"][j] += len(below)
                    far_ticks["ask"][j] += len(above)
                    far["bid"][j].append(below[below > 0])
                    far["ask"][j].append(above[above > 0])

        for day, (events, flow) in days.items():
            replay.replay_day(day, events, flow, delta, hook=hook)

        keep = np.arange(len(near_bids))
        if len(keep) > MAX_NEAR_SAMPLES:
            keep = np.sort(rng.choice(len(keep), MAX_NEAR_SAMPLES, replace=False))
        bids = np.stack([near_bids[i] for i in keep])
        asks = np.stack([near_asks[i] for i in keep])
        # Joint occupancy of the core ticks, one bit per cell.
        core = slice(NEAR - CORE, NEAR + CORE + 1)
        weights = 1 << np.arange(2 * CORE + 1)
        patterns = np.stack([(bids[:, core] > 0) @ weights, (asks[:, core] > 0) @ weights], axis=1)
        bid_presence, bid_q = quantile_table(bids)
        ask_presence, ask_q = quantile_table(asks)
        # Bootstrap library: real volumes around the centre and the two outside totals.
        boot = slice(NEAR - BOOT, NEAR + BOOT + 1)
        outside = np.array([outer[i] for i in keep], np.int64)
        library[f"bids_{delta}"] = bids[:, boot]
        library[f"asks_{delta}"] = asks[:, boot]
        library[f"bids_above_{delta}"] = outside[:, 0]
        library[f"asks_below_{delta}"] = outside[:, 1]

        far_out = {}
        for side in ("bid", "ask"):
            presence, table = [], []
            for j in range(len(FAR_EDGES) - 1):
                sizes = np.concatenate(far[side][j]) if far[side][j] else np.array([])
                presence.append(float(len(sizes)) / max(1, far_ticks[side][j]))
                table.append([float(q) for q in np.percentile(sizes, QUANTILES)] if len(sizes) else [0.0] * len(QUANTILES))
            far_out[side] = {"presence": presence, "quantiles": table}

        n = tallies["crossing"]
        out["by_delta"][str(delta)] = {
            "crossing_batches": n, "batches_used_for_near_profile": int(len(keep)),
            "batches_skipped_near_window": skipped[0],
            "real_rates_pct": {key: 100.0 * tallies[key] / n for key in
                               ("no_tie", "plateau_unique_valley", "plateau_tied_valley")},
            "real_v_max_quantiles": [float(q) for q in np.percentile(v_max, QUANTILES)],
            "near": {"bid": {"presence": bid_presence, "quantiles": bid_q},
                     "ask": {"presence": ask_presence, "quantiles": ask_q}},
            "far": far_out,
            "core": CORE, "core_patterns": patterns.astype(int).tolist(),
        }
        centre = NEAR
        print(f"delta {delta} s: {n} crossing batches; near profile from {len(keep)}; "
              f"presence at p*: bid {bid_presence[centre]:.3f}, ask {ask_presence[centre]:.3f}; "
              f"at p*-1: bid {bid_presence[centre - 1]:.3f}, ask {ask_presence[centre - 1]:.3f}; "
              f"at p*+1: bid {bid_presence[centre + 1]:.3f}, ask {ask_presence[centre + 1]:.3f}; "
              f"median V_max {np.median(v_max):.0f}")

    out["boot"] = BOOT
    np.savez_compressed(HERE / f"calibration_{args.symbol}_core.npz", **library)
    path = HERE / f"calibration_{args.symbol}.json"
    path.write_text(json.dumps(out))
    print(f"wrote {path} ({path.stat().st_size // 1024} KB)")


if __name__ == "__main__":
    main()
