#!/usr/bin/env python3
"""Replay a day of TotalView-ITCH order flow in batches and count how often the tie-breaks are needed."""
import argparse
import csv
import json
import math
import statistics
import sys
import warnings
from datetime import datetime
from pathlib import Path
from zoneinfo import ZoneInfo

import numpy as np

HERE = Path(__file__).parent
RAW = HERE.parent / "realdata" / "databento"
RESULTS = HERE.parent / "results"
NEW_YORK = ZoneInfo("America/New_York")
MAX_CENTS = 500_000          # prices above $5,000 are clamped to the top cell
HALF_WINDOW = 300            # ticks kept on each side of the batch's trade prices
E9_PER_CENT = 10_000_000     # Databento prices are in units of 1e-9 dollars


def clear(bids, asks):
    """The Zeequent clearing rule on two histograms (lowest price first)."""
    demand = np.cumsum(bids[::-1])[::-1]
    supply = np.cumsum(asks)
    volume = np.minimum(demand, supply)
    v_max = int(volume.max())
    if v_max == 0:
        return None
    plateau = np.flatnonzero(volume == v_max)
    c, d = int(plateau[0]), int(plateau[-1])
    gap = (supply - demand)[c:d + 1]
    delta = np.abs(gap)
    v = int(delta.min())
    valley = np.flatnonzero(delta == v) + c
    if v == 0:
        candidates = valley
    else:
        # The whole imbalance must rest at the clearing tick, on the long side.
        long_volume = np.where(gap[valley - c] < 0, bids[valley], asks[valley])
        candidates = valley[long_volume >= v]
    return {
        "v_max": v_max, "c": c, "d": d, "v_min_delta": v,
        "width": d - c + 1, "valley": int(len(valley)),
        "candidates": int(len(candidates)),
        "p_star": int(candidates[0]) if len(candidates) else -1,
    }


def selftest():
    def book(n, bids, asks):
        b, a = np.zeros(n, np.int64), np.zeros(n, np.int64)
        for tick, qty in bids:
            b[tick] += qty
        for tick, qty in asks:
            a[tick] += qty
        return b, a

    # The same cases as zeequent-core/tests/clearing.rs.
    cases = [
        ("unique maximizer", book(8, [(6, 5), (4, 5)], [(2, 4), (4, 4)]), (8, 4, 4, 4, 2)),
        ("demand-long plateau", book(8, [(5, 10)], [(2, 6)]), (6, 2, 5, 5, 4)),
        ("supply-long plateau", book(8, [(5, 6)], [(2, 10)]), (6, 2, 5, 2, 4)),
        ("supply-long valley", book(8, [(5, 6), (3, 5)], [(2, 6), (4, 4)]), (6, 2, 5, 4, 4)),
        ("balanced plateau", book(8, [(5, 5)], [(2, 5)]), (5, 2, 5, 2, 0)),
    ]
    for name, (b, a), expected in cases:
        r = clear(b, a)
        got = (r["v_max"], r["c"], r["d"], r["p_star"], r["v_min_delta"])
        assert got == expected, f"{name}: {got} != {expected}"
    assert clear(*book(8, [(2, 5)], [(5, 5)])) is None
    # The two-tick example of a global imbalance tie with a unique maximizer.
    r = clear(*book(2, [(0, 3), (1, 7)], [(0, 6), (1, 5)]))
    assert (r["width"], r["valley"]) == (1, 1)
    print("selftest passed")


def to_cents(price_e9, is_bid):
    """Floor for the buy side, ceiling for the sell side, clamped to the grid."""
    down = price_e9 // E9_PER_CENT
    up = -((-price_e9) // E9_PER_CENT)
    return np.clip(np.where(is_bid, down, up), 0, MAX_CENTS).astype(np.int64)


def load_day(path):
    import databento as db
    warnings.simplefilter("ignore")
    rec = db.DBNStore.from_file(str(path)).to_ndarray()
    action = rec["action"].astype("U1")
    side = rec["side"].astype("U1")
    ts = rec["ts_recv"].astype(np.int64)
    price = rec["price"].astype(np.int64)
    size = rec["size"].astype(np.int64)

    # Depth events: adds and cancels, signed.
    depth = (action == "A") | (action == "C")
    is_bid = side[depth] == "B"
    events = {
        "ts": ts[depth],
        "is_bid": is_bid,
        "cents": to_cents(price[depth], is_bid),
        "signed": np.where(action[depth] == "A", size[depth], -size[depth]),
    }
    # Flow: fills of resting orders (their own side) and aggressor trades.
    flow_mask = ((action == "F") | (action == "T")) & (side != "N")
    flow_bid = side[flow_mask] == "B"
    flow = {
        "ts": ts[flow_mask],
        "is_bid": flow_bid,
        "cents": to_cents(price[flow_mask], flow_bid),
        "size": size[flow_mask],
        "is_trade": action[flow_mask] == "T",
    }
    counts = {
        "records": int(len(rec)),
        "adds": int(np.sum(action == "A")), "cancels": int(np.sum(action == "C")),
        "fills": int(np.sum(action == "F")), "trades": int(np.sum(action == "T")),
        "trades_without_side": int(np.sum((action == "T") & (side == "N"))),
        "clears": int(np.sum(action == "R")),
    }
    return events, flow, counts


def day_screen():
    """{date: reasons} for sampled days that fail the screen."""
    path = HERE / "sample_days.csv"
    if not path.exists():
        return {}
    with open(path, newline="") as f:
        return {row["date"]: row.get("reasons", "") for row in csv.DictReader(f)
                if row.get("screen") == "fail"}


def bucket_of(seconds_into_session, session_seconds):
    if seconds_into_session < 1800:
        return "first30"
    if seconds_into_session >= session_seconds - 1800:
        return "last30"
    return "middle"


def replay_day(day, events, flow, delta_seconds, dump=None, hook=None):
    """Returns a list of per-batch result rows for one day and batch length."""
    y, m, d = (int(x) for x in day.split("-"))
    open_ns = int(datetime(y, m, d, 9, 30, tzinfo=NEW_YORK).timestamp()) * 1_000_000_000
    close_ns = int(datetime(y, m, d, 16, 0, tzinfo=NEW_YORK).timestamp()) * 1_000_000_000
    session_seconds = (close_ns - open_ns) / 1e9
    step = int(round(delta_seconds * 1e9))
    n_batches = (close_ns - open_ns) // step

    bid = np.zeros(MAX_CENTS + 1, np.int64)
    ask = np.zeros(MAX_CENTS + 1, np.int64)
    eb, ea = events["is_bid"], ~events["is_bid"]
    ev_bid = (events["ts"][eb], events["cents"][eb], events["signed"][eb])
    ev_ask = (events["ts"][ea], events["cents"][ea], events["signed"][ea])
    done = [0, 0]

    def advance(until_ns):
        for which, (book, (ts, cents, signed)) in enumerate(((bid, ev_bid), (ask, ev_ask))):
            stop = int(np.searchsorted(ts, until_ns, side="left"))
            if stop > done[which]:
                np.add.at(book, cents[done[which]:stop], signed[done[which]:stop])
                done[which] = stop

    in_session = (flow["ts"] >= open_ns) & (flow["ts"] < open_ns + n_batches * step)
    f_ts = flow["ts"][in_session]
    f_bid = flow["is_bid"][in_session]
    f_cents = flow["cents"][in_session]
    f_size = flow["size"][in_session]
    f_trade = flow["is_trade"][in_session]
    f_batch = (f_ts - open_ns) // step
    # The replay evaluates intervals containing a trade with an identified aggressor side.
    active = np.unique(f_batch[f_trade])
    starts = np.searchsorted(f_batch, active, side="left")
    stops = np.searchsorted(f_batch, active, side="right")

    rows = []
    window_violations = 0
    for k, i0, i1 in zip(active.tolist(), starts.tolist(), stops.tolist()):
        advance(open_ns + (k + 1) * step)
        cents = f_cents[i0:i1]
        lo = max(0, int(cents.min()) - HALF_WINDOW)
        hi = min(MAX_CENTS, int(cents.max()) + HALF_WINDOW)
        b = bid[lo:hi + 1].copy()
        a = ask[lo:hi + 1].copy()
        is_bid = f_bid[i0:i1]
        np.add.at(b, cents[is_bid] - lo, f_size[i0:i1][is_bid])
        np.add.at(a, cents[~is_bid] - lo, f_size[i0:i1][~is_bid])
        if len(rows) % 500 == 0:
            # The window is exact only if no resting bid lies above it and no resting ask below it.
            if bid[hi + 1:].any() or ask[:lo].any():
                window_violations += 1
        result = clear(b, a)
        seconds_in = k * delta_seconds
        row = {
            "batch": k, "bucket": bucket_of(seconds_in, session_seconds),
            "trades": int(f_trade[i0:i1].sum()),
            "crossed": result is not None,
        }
        if result:
            row.update(result)
            row["touches_window_edge"] = result["c"] == 0 or result["d"] == hi - lo
            if hook is not None:
                hook(k, lo, b, a, result, bid, ask)
            if dump is not None and result["width"] > 1:
                # Keep the first plateau batch of each tie category per day and batch length.
                kind = (day, delta_seconds, result["valley"] > 1)
                if kind not in dump["seen"] and len(dump["books"]) < dump["limit"]:
                    dump["seen"].add(kind)
                    dump["books"].append((day, delta_seconds, k, lo, b, a, result))
        rows.append(row)

    advance(close_ns)
    meta = {
        "batches": int(n_batches), "batches_with_aggressor_trade": int(len(active)),
        "window_spot_check_violations": window_violations,
        "min_depth_bid": int(bid.min()), "min_depth_ask": int(ask.min()),
    }
    return rows, meta


def summarise(rows):
    crossing = [r for r in rows if r["crossed"]]
    n = len(crossing)
    wide = [r for r in crossing if r["width"] > 1]
    tied = [r for r in wide if r["valley"] > 1]
    widths = sorted(r["width"] for r in crossing)
    return {
        "crossing": n,
        "no_tie": n - len(wide),
        "plateau_unique_valley": len(wide) - len(tied),
        "plateau_tied_valley": len(tied),
        "tied_valley_two_candidates": sum(1 for r in tied if r["candidates"] > 1),
        "no_candidate": sum(1 for r in crossing if r["candidates"] == 0),
        "touches_window_edge": sum(1 for r in crossing if r["touches_window_edge"]),
        "median_width": widths[len(widths) // 2] if widths else 0,
        "max_width": widths[-1] if widths else 0,
        "median_trades": statistics.median(r["trades"] for r in crossing) if crossing else 0,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--selftest", action="store_true")
    parser.add_argument("--symbol", default="AAPL")
    parser.add_argument("--deltas", default="0.1,1,10,60")
    parser.add_argument("--dump-books", type=int, default=0,
                        help="write N plateau batches as order-book CSVs for cross-checking")
    args = parser.parse_args()
    if args.selftest:
        selftest()
        return
    selftest()

    deltas = [float(x) for x in args.deltas.split(",")]
    files = sorted(RAW.glob(f"{args.symbol}_*_mbo.dbn.zst"))
    if not files:
        sys.exit(f"no mbo files for {args.symbol} in {RAW}")
    RESULTS.mkdir(exist_ok=True)
    dump = {"books": [], "seen": set(), "limit": args.dump_books} if args.dump_books else None

    fields = ["symbol", "date", "delta_s", "bucket", "batches", "batches_with_aggressor_trade",
              "crossing", "no_tie", "plateau_unique_valley", "plateau_tied_valley",
              "tied_valley_two_candidates", "no_candidate", "touches_window_edge",
              "median_width", "max_width", "median_trades"]
    out_rows, day_meta = [], {}
    for path in files:
        day = path.name.split("_")[1]
        events, flow, counts = load_day(path)
        day_meta[day] = {"counts": counts}
        print(f"{day}: {counts}")
        for delta in deltas:
            rows, meta = replay_day(day, events, flow, delta, dump)
            day_meta[day][str(delta)] = meta
            for bucket in ["all", "first30", "middle", "last30"]:
                subset = rows if bucket == "all" else [r for r in rows if r["bucket"] == bucket]
                s = summarise(subset)
                out_rows.append({"symbol": args.symbol, "date": day, "delta_s": delta, "bucket": bucket,
                                 "batches": meta["batches"] if bucket == "all" else "",
                                 "batches_with_aggressor_trade": len(subset), **s})
            s = summarise(rows)
            pct = lambda x: 100.0 * x / s["crossing"] if s["crossing"] else float("nan")
            print(f"  delta {delta:>5} s: {meta['batches']:>7} batches, {s['crossing']:>6} crossing | "
                  f"no tie {pct(s['no_tie']):5.1f}%  plateau+unique valley {pct(s['plateau_unique_valley']):5.1f}%  "
                  f"plateau+tied valley {pct(s['plateau_tied_valley']):5.1f}%  | median width {s['median_width']}, "
                  f"max {s['max_width']}; checks: window {meta['window_spot_check_violations']}, "
                  f"edge {s['touches_window_edge']}, no-candidate {s['no_candidate']}, "
                  f"min depth {min(meta['min_depth_bid'], meta['min_depth_ask'])}")

    out = RESULTS / f"replay_{args.symbol}.csv"
    with open(out, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=fields)
        w.writeheader()
        w.writerows(out_rows)
    (RESULTS / f"replay_{args.symbol}_meta.json").write_text(json.dumps(day_meta, indent=2))
    print(f"\nwrote {out}")

    # Between-day variation.
    print("\nBetween-day summary over days that pass the screen (bucket = all). "
          "Percentages of crossing batches; s.d. across days.")
    print("| delta (s) | days | category | mean % | s.d. (pp) | 95% t half-width (pp) | days for +-1 pp |")
    print("|---|---|---|---|---|---|---|")
    t_table = {2: 12.706, 3: 4.303, 4: 3.182, 5: 2.776, 6: 2.571, 7: 2.447, 8: 2.365, 9: 2.306, 10: 2.262}
    summary_rows = []
    flagged = day_screen()
    for day in sorted(day_meta):
        if day in flagged:
            print(f"(excluded from this summary: {day}, {flagged[day]})")
    for delta in deltas:
        per_day = [r for r in out_rows if r["delta_s"] == delta and r["bucket"] == "all"
                   and r["crossing"] and r["date"] not in flagged]
        for category in ["no_tie", "plateau_unique_valley", "plateau_tied_valley"]:
            values = [100.0 * r[category] / r["crossing"] for r in per_day]
            if len(values) < 2:
                continue
            mean, sd = statistics.mean(values), statistics.stdev(values)
            half = t_table.get(len(values), 1.96) * sd / math.sqrt(len(values))
            needed = max(2, math.ceil((1.96 * sd / 1.0) ** 2))
            print(f"| {delta} | {len(values)} | {category} | {mean:.2f} | {sd:.2f} | {half:.2f} | {needed} |")
            summary_rows.append([delta, len(values), category, round(mean, 3), round(sd, 3), round(half, 3), needed])
    with open(RESULTS / f"replay_{args.symbol}_between_day.csv", "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["delta_s", "days", "category", "mean_pct", "sd_pp", "t_halfwidth_pp", "days_for_1pp"])
        w.writerows(summary_rows)

    if dump:
        folder = RESULTS / "replay_books"
        folder.mkdir(exist_ok=True)
        index = []
        for i, (day, delta, k, lo, b, a, result) in enumerate(dump["books"]):
            name = f"book_{i:02d}.csv"
            with open(folder / name, "w", newline="") as f:
                w = csv.writer(f)
                w.writerow(["Price", "Bids++", "Asks++"])
                for j in range(len(b)):
                    w.writerow([lo + j, int(b[j]), int(a[j])])
            index.append({"file": name, "date": day, "delta_s": delta, "batch": k, "lo_cents": lo, **result})
        (folder / "index.json").write_text(json.dumps(index, indent=2))
        print(f"wrote {len(index)} plateau books to {folder}")


if __name__ == "__main__":
    main()
