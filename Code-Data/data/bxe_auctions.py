# Extracts the periodic auctions from a Cboe BXE message file.
# Usage: python bxe_auctions.py FILE.csv OUT_DIR [--books]
import csv
import json
import os
import statistics
import sys

AUCTION_UPDATE, AUCTION_SUMMARY = "172", "150"
TRADE_LONG, TRADE_SHORT, ORDER_EXECUTED, TRADE_OFF_BOOK = "65", "43", "35", "50"

path, out_dir = sys.argv[1], sys.argv[2]
write_books = "--books" in sys.argv
os.makedirs(out_dir, exist_ok=True)

auctions, updates, trades_at = [], [], {}
lit_volume = off_book_volume = 0
prices = set()
for row in csv.reader(open(path)):
    kind, time = row[0], int(row[2])
    if kind == AUCTION_UPDATE and row[4] == "P":
        # Associate positive indicative-price updates with the next periodic summary.
        if float(row[6]) > 0:
            updates.append((time, float(row[5]), float(row[6]), int(row[7])))
    elif kind == AUCTION_SUMMARY and row[4] == "P":
        auctions.append({"time": time, "price": float(row[5]), "shares": int(row[6]), "updates": updates})
        updates = []
    elif kind in (TRADE_LONG, TRADE_SHORT):
        flags = row[9]
        if flags.startswith("5I"):
            trades_at.setdefault(time, []).append(int(row[5]))
        else:
            lit_volume += int(row[5])
        prices.add(row[7])
    elif kind == ORDER_EXECUTED:
        lit_volume += int(row[4])
    elif kind == TRADE_OFF_BOOK:
        off_book_volume += int(row[3])

# Tick size is the smallest gap between distinct auction prices, in tenths of a cent.
grid = sorted({round(a["price"] * 10000) for a in auctions})
tick = min(b - a for a, b in zip(grid, grid[1:]))

rows = []
for i, a in enumerate(auctions):
    fills = trades_at.get(a["time"], [])
    first = a["updates"][0][0] if a["updates"] else a["time"]
    rows.append({
        "auction": i,
        "time_s": round(a["time"] / 1e9, 6),
        "price": a["price"],
        "shares": a["shares"],
        "fills": len(fills),
        "fill_shares": sum(fills),
        "updates": len(a["updates"]),
        "duration_ms": round((a["time"] - first) / 1e6, 3),
        "reference": a["updates"][-1][1] if a["updates"] else "",
    })

with open(os.path.join(out_dir, "auctions.csv"), "w", newline="") as f:
    w = csv.DictWriter(f, fieldnames=list(rows[0]))
    w.writeheader()
    w.writerows(rows)

shares = [r["shares"] for r in rows]
dur = [r["duration_ms"] for r in rows if r["updates"]]
gaps = [(b["time"] - a["time"]) / 1e9 for a, b in zip(auctions, auctions[1:])]
summary = {
    "auctions": len(rows),
    "auction_shares_total": sum(shares),
    "auction_shares_median": statistics.median(shares),
    "auction_shares_max": max(shares),
    "fills_total": sum(r["fills"] for r in rows),
    "fills_per_auction_median": statistics.median(r["fills"] for r in rows),
    "fills_per_auction_max": max(r["fills"] for r in rows),
    "auctions_with_fill_shares_equal": sum(r["fill_shares"] == r["shares"] for r in rows),
    "duration_ms_median": statistics.median(dur),
    "duration_ms_max": max(dur),
    "gap_s_median": statistics.median(gaps),
    "first_time_s": rows[0]["time_s"],
    "last_time_s": rows[-1]["time_s"],
    "distinct_prices": len(grid),
    "price_min": min(a["price"] for a in auctions),
    "price_max": max(a["price"] for a in auctions),
    "tick_in_1e-4": tick,
    "lit_book_shares": lit_volume,
    "off_book_shares": off_book_volume,
}
json.dump(summary, open(os.path.join(out_dir, "summary.json"), "w"), indent=1)
print(json.dumps(summary, indent=1))

if write_books:
    # Construct one balanced book per published outcome. The feed does not contain the underlying auction orders.
    half = 50
    for r in rows:
        fills = trades_at.get(auctions[r["auction"]]["time"], []) or [r["shares"]]
        centre = round(r["price"] * 10000)
        with open(os.path.join(out_dir, f"auction_{r['auction']:03d}.csv"), "w", newline="") as f:
            w = csv.writer(f)
            w.writerow(["Price", "Bids++", "Asks++"])
            for k in range(-half, half + 1):
                volume = sum(fills) if k == 0 else 0
                w.writerow([centre + k * tick, volume, volume])
