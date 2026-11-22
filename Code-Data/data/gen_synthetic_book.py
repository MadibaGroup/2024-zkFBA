#!/usr/bin/env python3
"""Generate synthetic batch order books calibrated to real Nasdaq TotalView-ITCH order flow."""
import argparse
import csv
import json
from pathlib import Path

import numpy as np

from replay import clear

HERE = Path(__file__).parent
MODELS = ("independent", "occupancy", "bootstrap")

HEADER = [
    "Price", "Bids++", "Asks++", "Bid_Depth", "Ask_Depth", "Min_Bid_Ask",
    "Selector_MCV", "ln0_MCV", "Bid_Surplus", "Ask_Surplus", "Abs_Delta",
    "Check_on_Delta", "S_Delta", "Selector_MCI", "MCV", "Cliff_Value",
    "Slack", "Is_MCV_Tie", "Is_Min_Delta", "Clearing_Price",
]


class Calibration:
    def __init__(self, symbol, delta):
        data = json.loads((HERE / f"calibration_{symbol}.json").read_text())
        self.key = str(float(delta))
        if self.key not in data["by_delta"]:
            raise SystemExit(f"no calibration for delta {delta}; available: {list(data['by_delta'])}")
        self.symbol = symbol
        self.meta = {k: data[k] for k in ("symbol", "dataset", "days", "tick")}
        self.levels = np.array(data["quantile_levels"], float)
        self.near = data["near"]
        self.far_edges = data["far_edges"]
        self.boot = data["boot"]
        self.d = data["by_delta"][self.key]
        self._library = None

    def library(self):
        if self._library is None:
            archive = np.load(HERE / f"calibration_{self.symbol}_core.npz")
            self._library = {name: archive[f"{name}_{self.key}"]
                             for name in ("bids", "asks", "bids_above", "asks_below")}
        return self._library

    def sizes(self, quantiles, n, rng):
        """n draws from the distribution with these quantiles, at least 1."""
        draws = np.interp(rng.uniform(0, 100, n), self.levels, np.asarray(quantiles, float))
        return np.maximum(1, np.rint(draws)).astype(np.int64)


def rescale_rows(block, targets, anchor):
    """Rescales each row of `block` (in place) to sum to its target, keeping proportions."""
    for row, target in zip(block, targets):
        total = int(row.sum())
        if target == 0:
            row[:] = 0
        elif total == 0:
            row[anchor] = target
        else:
            scaled = row * int(target) // total
            scaled[int(np.argmax(row))] += int(target) - int(scaled.sum())
            row[:] = scaled


def generate(cal, n_ticks, count, rng, model="bootstrap"):
    """Returns (bids, asks), each of shape (count, n_ticks)."""
    width = 2 * cal.near + 1
    near = {}
    if model == "occupancy":
        core = cal.d["core"]
        patterns = np.asarray(cal.d["core_patterns"], np.int64)
        chosen = patterns[rng.integers(0, len(patterns), count)]
    for column_index, side in enumerate(("bid", "ask")):
        profile = cal.d["near"][side]
        present = rng.random((count, width)) < np.asarray(profile["presence"])
        if model == "occupancy":
            for offset in range(-core, core + 1):
                present[:, cal.near + offset] = (chosen[:, column_index] >> (offset + core)) & 1
        volume = np.zeros((count, width), np.int64)
        for index in range(width):
            rows = present[:, index]
            volume[rows, index] = cal.sizes(profile["quantiles"][index], int(rows.sum()), rng)
        near[side] = volume

    if model == "bootstrap":
        lib = cal.library()
        chosen = rng.integers(0, len(lib["bids"]), count)
        lo, hi = cal.near - cal.boot, cal.near + cal.boot + 1
        near["bid"][:, lo:hi] = lib["bids"][chosen]
        near["ask"][:, lo:hi] = lib["asks"][chosen]
        rescale_rows(near["bid"][:, hi:], lib["bids_above"][chosen], anchor=0)
        rescale_rows(near["ask"][:, :lo], lib["asks_below"][chosen], anchor=lo - 1)

    # Put the near region on the grid.
    centre = n_ticks // 2
    ticks = centre + np.arange(-cal.near, cal.near + 1)
    inside = (ticks >= 0) & (ticks < n_ticks)
    bids = np.zeros((count, n_ticks), np.int64)
    asks = np.zeros((count, n_ticks), np.int64)
    bids[:, ticks[inside]] = near["bid"][:, inside]
    asks[:, ticks[inside]] = near["ask"][:, inside]
    bids[:, n_ticks - 1] += near["bid"][:, ticks >= n_ticks].sum(axis=1)
    asks[:, 0] += near["ask"][:, ticks < 0].sum(axis=1)

    # Far region: resting bids below the centre, resting asks above, by band.
    for j in range(len(cal.far_edges) - 1):
        e1, e2 = cal.far_edges[j], cal.far_edges[j + 1]
        for side, book, band_ticks in (
            ("bid", bids, range(centre - e2 + 1, centre - e1 + 1)),
            ("ask", asks, range(centre + e1, centre + e2)),
        ):
            band = cal.d["far"][side]
            p = band["presence"][j]
            kept = [t for t in band_ticks if 0 <= t < n_ticks]
            if p <= 0 or not kept:
                continue
            present = rng.random((count, len(kept))) < p
            values = np.zeros((count, len(kept)), np.int64)
            values[present] = cal.sizes(band["quantiles"][j], int(present.sum()), rng)
            book[:, kept[0]:kept[-1] + 1] += values
    return bids, asks


def classify(bids, asks):
    tallies = {"batches": len(bids), "crossing": 0, "no_tie": 0,
               "plateau_unique_valley": 0, "plateau_tied_valley": 0}
    v_max = []
    for b, a in zip(bids, asks):
        result = clear(b, a)
        if result is None:
            continue
        tallies["crossing"] += 1
        v_max.append(result["v_max"])
        if result["width"] == 1:
            tallies["no_tie"] += 1
        elif result["valley"] == 1:
            tallies["plateau_unique_valley"] += 1
        else:
            tallies["plateau_tied_valley"] += 1
    return tallies, v_max


def derived_rows(prices, b, a):
    """The 20 spreadsheet columns for one book."""
    acc_b = np.cumsum(b[::-1])[::-1]
    acc_a = np.cumsum(a)
    volume = np.minimum(acc_b, acc_a)
    surp_b, surp_a = acc_b - volume, acc_a - volume
    delta = surp_a + surp_b
    result = clear(b, a)
    rows = []
    for i in range(len(b)):
        if result is None:
            extra = [0, 0, int(surp_b[i]), int(surp_a[i]), int(delta[i]), 0, 0, 0, 0, 0, 0, 0, 0, 0]
        else:
            c, d, p, mcv, v = result["c"], result["d"], result["p_star"], result["v_max"], result["v_min_delta"]
            in_plateau = c <= i <= d
            in_valley = in_plateau and delta[i] == v
            extra = [
                int(in_plateau),                                    # Selector_MCV
                mcv if in_plateau else 0,                           # ln0_MCV
                int(surp_b[i]), int(surp_a[i]), int(delta[i]),
                int(in_valley),                                     # Check_on_Delta
                int(delta[i]) if i == p else 0,                     # S_Delta
                int(i == p),                                        # Selector_MCI
                mcv,                                                # MCV
                int(volume[i]) if i in (c - 1, d + 1) else 0,       # Cliff_Value
                int(mcv - volume[i]),                               # Slack
                int(result["width"] > 1),                           # Is_MCV_Tie
                int(in_valley),                                     # Is_Min_Delta
                int(prices[p]),                                     # Clearing_Price
            ]
        rows.append([int(prices[i]), int(b[i]), int(a[i]), int(acc_b[i]), int(acc_a[i]), int(volume[i])] + extra)
    return rows, result


def validate(symbol, deltas, count, seed):
    print(f"Tie rates as % of crossing batches. Real = replay of {symbol}; "
          f"each model = {count} generated books of 601 ticks.\n")
    print("| delta (s) | source | crossing | no tie | plateau, unique valley | plateau, tied valley | median V_max |")
    print("|---|---|---|---|---|---|---|")
    report = {}
    for delta in deltas:
        cal = Calibration(symbol, delta)
        real = cal.d["real_rates_pct"]
        real_median = cal.d["real_v_max_quantiles"][cal.levels.tolist().index(50)]
        print(f"| {delta} | real | {cal.d['crossing_batches']} | {real['no_tie']:.2f} | "
              f"{real['plateau_unique_valley']:.2f} | {real['plateau_tied_valley']:.2f} | {real_median:.0f} |")
        report[str(delta)] = {"real": real, "real_median_v_max": real_median}
        for model in MODELS:
            bids, asks = generate(cal, 601, count, np.random.default_rng(seed), model)
            t, v_max = classify(bids, asks)
            n = max(1, t["crossing"])
            rates = {k: 100.0 * t[k] / n for k in ("no_tie", "plateau_unique_valley", "plateau_tied_valley")}
            print(f"| {delta} | {model} | {t['crossing']} | {rates['no_tie']:.2f} | "
                  f"{rates['plateau_unique_valley']:.2f} | {rates['plateau_tied_valley']:.2f} | "
                  f"{np.median(v_max) if v_max else 0:.0f} |")
            report[str(delta)][model] = {**rates, "median_v_max": float(np.median(v_max)) if v_max else 0.0,
                                         "crossing": t["crossing"], "batches": count}
    out = HERE.parent / "results" / f"generator_validation_{symbol}.json"
    out.write_text(json.dumps(report, indent=2))
    print(f"\nwrote {out}")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--symbol", default="AAPL")
    parser.add_argument("--delta", type=float, default=1.0, help="batch length the calibration is taken from")
    parser.add_argument("--model", choices=MODELS, default="bootstrap")
    parser.add_argument("--ticks", type=int, default=1000)
    parser.add_argument("--count", type=int, default=1)
    parser.add_argument("--seed", type=int, default=1)
    parser.add_argument("--first-price", type=int, default=0, help="price label of tick 0, in ticks")
    parser.add_argument("--out", default=str(HERE.parent / "syndata_calibrated"))
    parser.add_argument("--validate", action="store_true")
    args = parser.parse_args()

    if args.validate:
        validate(args.symbol, [0.1, 1.0, 10.0, 60.0], 20000, args.seed)
        return

    cal = Calibration(args.symbol, args.delta)
    rng = np.random.default_rng(args.seed)
    bids, asks = generate(cal, args.ticks, args.count, rng, args.model)
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    prices = np.arange(args.first_price, args.first_price + args.ticks)
    stem = f"{args.ticks}_{args.model}_d{args.delta:g}_s{args.seed}"
    books = []
    for i in range(args.count):
        rows, result = derived_rows(prices, bids[i], asks[i])
        name = f"order_book_{stem}_{i:03d}.csv"
        with open(out / name, "w", newline="") as f:
            w = csv.writer(f)
            w.writerow(HEADER)
            w.writerows(rows)
        summary = {"file": name, "crossed": result is not None, **(result or {})}
        books.append(summary)
        print(summary)
    (out / f"manifest_{stem}.json").write_text(json.dumps({
        "generator": "gen_synthetic_book.py", "model": args.model, "calibration": cal.meta,
        "batch_length_s": args.delta, "ticks": args.ticks, "seed": args.seed, "books": books,
    }, indent=2))


if __name__ == "__main__":
    main()
