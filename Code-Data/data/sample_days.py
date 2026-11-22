#!/usr/bin/env python3
"""Draw a reproducible random sample of "ordinary" trading days."""
import csv
import random
from datetime import date, timedelta
from pathlib import Path

SEED = 20261117
FRAME_START, FRAME_END = date(2026, 1, 2), date(2026, 11, 13)

HOLIDAYS = {
    date(2025, 11, 27), date(2025, 12, 25), date(2026, 1, 1), date(2026, 1, 19),
    date(2026, 2, 16), date(2026, 4, 3), date(2026, 5, 25), date(2026, 6, 19),
    date(2026, 7, 3), date(2026, 9, 7),
}
EARLY_CLOSES = {date(2025, 11, 28), date(2025, 12, 24), date(2026, 7, 2)}

FOMC_DECISIONS = [
    date(2025, 12, 10), date(2026, 1, 28), date(2026, 3, 18), date(2026, 4, 29),
    date(2026, 6, 17), date(2026, 7, 29), date(2026, 9, 16), date(2026, 10, 28),
]
CPI_RELEASES = [
    date(2025, 12, 18), date(2026, 1, 13), date(2026, 2, 13), date(2026, 3, 11),
    date(2026, 4, 10), date(2026, 5, 12), date(2026, 6, 10), date(2026, 7, 14),
    date(2026, 8, 12), date(2026, 9, 11), date(2026, 10, 14), date(2026, 11, 10),
]
EMPLOYMENT_RELEASES = [
    date(2025, 11, 20), date(2025, 12, 16), date(2026, 1, 9), date(2026, 2, 11),
    date(2026, 3, 6), date(2026, 4, 3), date(2026, 5, 8), date(2026, 6, 5),
    date(2026, 7, 2), date(2026, 8, 7), date(2026, 9, 4), date(2026, 10, 2),
    date(2026, 11, 6),
]


def trading_days():
    day, days = FRAME_START, []
    while day <= FRAME_END:
        if day.weekday() < 5 and day not in HOLIDAYS:
            days.append(day)
        day += timedelta(days=1)
    return days


# Second-stage screen.

STUDY_STOCK = "AAPL"
MARKET_PROXY = "QQQ"
PEERS = ["NVDA", "MSFT", "GOOGL", "AMZN", "META", "TSLA"]
CROSS_ASSET = {"SMH": "semiconductors", "USO": "crude oil", "TLT": "long Treasuries", "GLD": "gold"}
OWN_SD, SHOCK_SD = 2.0, 3.0
VOLUME_HIGH, VOLUME_LOW, RANGE_MULTIPLE, PEER_VOLUME = 2.0, 0.5, 2.0, 3.0
RETURN_LOOKBACK, VOLUME_LOOKBACK = 60, 20


def _dates(*month_days):
    return [date(2026, m, d) for m, d in month_days]


EARNINGS = {
    "AAPL": _dates((1, 29), (4, 30), (7, 30), (11, 2)),
    "NVDA": _dates((2, 25), (5, 20), (8, 26)),
}
SCHEDULED = {
    "PPI release": _dates((1, 14), (1, 30), (2, 27), (3, 18), (4, 14), (5, 13), (6, 11), (7, 15),
                          (8, 13), (9, 10), (10, 15), (11, 13)),
    "GDP release": _dates((2, 20), (3, 13), (4, 9), (4, 30), (5, 28), (6, 25), (7, 30), (8, 26),
                          (9, 30), (10, 29)),
    "PCE release": _dates((1, 22), (2, 20), (3, 13), (4, 9), (4, 30), (5, 28), (6, 25), (7, 30),
                          (8, 26), (9, 30), (10, 29)),
    "Employment Cost Index": _dates((2, 10), (4, 30), (7, 31), (10, 30)),
    "JOLTS release": _dates((1, 7), (2, 5), (3, 13), (3, 31), (5, 5), (6, 2), (6, 30), (8, 4),
                            (9, 1), (9, 29), (11, 3)),
    "FOMC minutes": _dates((2, 18), (4, 8), (5, 20), (7, 8), (8, 19), (10, 7)),
    "Jackson Hole symposium": _dates((8, 27), (8, 28)),
    "AAPL ex-dividend": _dates((2, 9), (5, 11), (8, 10), (11, 12)),
    "Apple WWDC keynote": _dates((6, 8)),
    "Apple product event": _dates((9, 9)),
}


def screen(days):
    """Returns {day: [reasons]} for the days that fail the screen."""
    flags = {}

    def flag(day, reason):
        flags.setdefault(day, []).append(reason)

    # (a) Scheduled events.
    position = {d: i for i, d in enumerate(days)}
    for symbol, dates in EARNINGS.items():
        for announced in dates:
            if announced in position:
                flag(announced, f"{symbol} earnings (after close)")
                if position[announced] + 1 < len(days):
                    flag(days[position[announced] + 1], f"day after {symbol} earnings")
    for reason, dates in SCHEDULED.items():
        for d in dates:
            flag(d, reason)
    for i, d in enumerate(days):
        if i + 1 == len(days) or days[i + 1].month != d.month:
            if d != FRAME_END:          # the frame's last day is not a month end
                flag(d, "last trading day of the month")

    bars_path = Path(__file__).parent / "daily_bars.csv"
    if not bars_path.exists():
        print("daily_bars.csv not found: the price and volume screens were skipped")
        return flags
    series = {}
    with open(bars_path, newline="") as f:
        for row in csv.DictReader(f):
            series.setdefault(row["symbol"], []).append(
                (date.fromisoformat(row["date"]), float(row["high"]), float(row["low"]),
                 float(row["close"]), float(row["volume"])))
    for rows in series.values():
        rows.sort()

    def median(values):
        ordered = sorted(values)
        mid = len(ordered) // 2
        return ordered[mid] if len(ordered) % 2 else (ordered[mid - 1] + ordered[mid]) / 2

    def moves(symbol, multiple, label):
        rows = series[symbol]
        returns = [rows[i][3] / rows[i - 1][3] - 1 for i in range(1, len(rows))]
        for i in range(RETURN_LOOKBACK, len(returns)):
            window = returns[i - RETURN_LOOKBACK:i]
            mean = sum(window) / len(window)
            sd = (sum((r - mean) ** 2 for r in window) / (len(window) - 1)) ** 0.5
            if abs(returns[i]) > multiple * sd:
                flag(rows[i + 1][0], f"{label} {100 * returns[i]:+.1f}% "
                                     f"(> {multiple:g} s.d. = {100 * multiple * sd:.1f}%)")

    def volumes(symbol, high, low, label):
        rows = series[symbol]
        for i in range(VOLUME_LOOKBACK, len(rows)):
            ratio = rows[i][4] / median([r[4] for r in rows[i - VOLUME_LOOKBACK:i]])
            if ratio > high or (low is not None and ratio < low):
                flag(rows[i][0], f"{label} volume {ratio:.1f}x its 20-day median")

    def ranges(symbol, label):
        rows = series[symbol]
        spans = [(r[1] - r[2]) / r[3] for r in rows]
        for i in range(VOLUME_LOOKBACK, len(rows)):
            ratio = spans[i] / median(spans[i - VOLUME_LOOKBACK:i])
            if ratio > RANGE_MULTIPLE:
                flag(rows[i][0], f"{label} high-low range {ratio:.1f}x its 20-day median")

    # (b) The stock and the market against their own history.
    for symbol in (STUDY_STOCK, MARKET_PROXY):
        moves(symbol, OWN_SD, f"{symbol} move")
        ranges(symbol, symbol)
    volumes(STUDY_STOCK, VOLUME_HIGH, VOLUME_LOW, STUDY_STOCK)

    # (c) Shocks in peers and in other asset classes.
    for symbol in PEERS:
        moves(symbol, SHOCK_SD, f"peer {symbol} move")
        volumes(symbol, PEER_VOLUME, None, f"peer {symbol}")
    for symbol, name in CROSS_ASSET.items():
        moves(symbol, SHOCK_SD, f"{name} ({symbol}) move")
    return flags


def third_friday(year, month):
    first = date(year, month, 1)
    return first + timedelta(days=(4 - first.weekday()) % 7 + 14)


def main():
    days = trading_days()
    index = {d: i for i, d in enumerate(days)}
    reasons = {}

    def exclude(day, reason, neighbours=False):
        if day in index:
            reasons.setdefault(day, reason)
            if neighbours:
                for j in (index[day] - 1, index[day] + 1):
                    if 0 <= j < len(days):
                        reasons.setdefault(days[j], f"adjacent to {reason}")

    for d in FOMC_DECISIONS:
        exclude(d, "FOMC decision", neighbours=True)
    for d in CPI_RELEASES:
        exclude(d, "CPI release", neighbours=True)
    for d in EMPLOYMENT_RELEASES:
        exclude(d, "Employment Situation release", neighbours=True)
    for d in days:
        if d == third_friday(d.year, d.month):
            exclude(d, "monthly options expiration")
    exclude(date(2026, 6, 26), "Russell reconstitution")
    exclude(date(2026, 11, 3), "US midterm election day", neighbours=True)
    for d in EARLY_CLOSES:
        exclude(d, "early close")
    for holiday in HOLIDAYS:
        before = max((d for d in days if d < holiday), default=None)
        after = min((d for d in days if d > holiday), default=None)
        for d in (before, after):
            if d:
                reasons.setdefault(d, "adjacent to market holiday")

    eligible = [d for d in days if d not in reasons]
    order = eligible[:]
    random.Random(SEED).shuffle(order)

    # Second stage: screen each drawn day for unscheduled disturbances.
    flags = screen(days)
    out = Path(__file__).parent / "sample_days.csv"
    with open(out, "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["draw", "date", "weekday", "screen", "reasons"])
        for i, d in enumerate(order, 1):
            why = flags.get(d, [])
            w.writerow([i, d.isoformat(), d.strftime("%A"), "fail" if why else "pass", "; ".join(why)])
    failed = [d for d in order if flags.get(d)]
    print(f"screen: {len(order) - len(failed)} of {len(order)} drawn days pass; {len(failed)} fail")
    for d in order[:12]:
        print(f"  {d}  {'FAIL: ' + '; '.join(flags[d]) if flags.get(d) else 'pass'}")

    print(f"frame: {FRAME_START} to {FRAME_END}: {len(days)} trading days")
    print(f"excluded: {len(reasons)}; eligible: {len(eligible)}; seed: {SEED}")
    tally = {}
    for reason in reasons.values():
        key = reason.replace("adjacent to ", "") if not reason.startswith("adjacent to market") else reason
        tally[key] = tally.get(key, 0) + 1
    for key, count in sorted(tally.items(), key=lambda kv: -kv[1]):
        print(f"  {count:3d}  {key} (day or neighbour)")
    print("first 30 draws:")
    for i, d in enumerate(order[:30], 1):
        print(f"  {i:2d}  {d}  {d.strftime('%a')}")
    by_month = {}
    for d in order[:20]:
        by_month[d.strftime("%Y-%m")] = by_month.get(d.strftime("%Y-%m"), 0) + 1
    print("months covered by the first 20:", dict(sorted(by_month.items())))
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
