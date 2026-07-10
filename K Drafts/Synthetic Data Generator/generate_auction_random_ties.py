#!/usr/bin/env python3
"""Randomized-tie variant of the batch auction generator.
"""

import os
import sys
import argparse

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from generate_auction import (   # noqa: E402
    explode_volumes,
    compute_order_book,
    verify_aggregation,
    print_order_book,
    trades_to_df,
    book_to_df,
)


def max_reasonable_tie_length(n_ticks: int, cap: int = 8) -> int:
    # ~8% of price ticks, capped at cap
    return max(2, min(cap, round(0.08 * n_ticks)))


def sample_tie_length(rng: np.random.Generator, hi: int,
                       sigma: float = 0.55) -> int:
    # log-normal: peak at k=2, decaying tail -- more realistic than uniform
    if hi <= 2:
        return 2
    raw = rng.lognormal(mean=0.0, sigma=sigma)
    return int(np.clip(2 + int(np.floor(raw)), 2, hi))


def design_order_book_random_tie(n_trades: int, prices: np.ndarray,
                                  rng: np.random.Generator,
                                  clear_frac: float = 0.40,
                                  position_jitter: int = 3,
                                  surplus_jitter_factor: float = 2.0,
                                  max_tie_len: int = None):
    n = len(prices)
    ci = max(1, min(int(n * clear_frac), n - 3))
    if max_tie_len is None:
        max_tie_len = max_reasonable_tie_length(n)

    avg_qty = 200
    half_vol = (n_trades * avg_qty) // 2
    x = np.arange(n, dtype=float)
    floor = max(5, n_trades // (8 * n))

    # Gaussian-shaped volume distributions around the clearing price
    bid_mu = ci + n * 0.06
    bid_sig = n * 0.30
    raw_bid = np.exp(-0.5 * ((x - bid_mu) / bid_sig) ** 2) + 0.04
    bid_wts = raw_bid / raw_bid.sum()

    ask_mu = ci - n * 0.06
    ask_sig = n * 0.30
    raw_ask = np.exp(-0.5 * ((x - ask_mu) / ask_sig) ** 2) + 0.04
    ask_wts = raw_ask / raw_ask.sum()

    bids_vol = np.round(bid_wts * half_vol * rng.uniform(0.88, 1.12, n)).astype(int)
    asks_vol = np.round(ask_wts * half_vol * rng.uniform(0.88, 1.12, n)).astype(int)
    bids_vol = np.maximum(bids_vol, floor)
    asks_vol = np.maximum(asks_vol, floor)

    bid_d = np.cumsum(bids_vol[::-1])[::-1].astype(int)
    ask_d = np.cumsum(asks_vol).astype(int)
    diff = bid_d.astype(float) - ask_d.astype(float)

    # find natural supply/demand crossing
    cross = ci
    for i in range(n - 1):
        if diff[i] >= 0 and diff[i + 1] <= 0:
            cross = i
            break

    # jitter anchor around the crossing
    lo = max(1, cross - position_jitter)
    hi = min(n - 3, cross + position_jitter)
    c = int(rng.integers(lo, hi + 1)) if hi >= lo else min(cross, n - 3)
    if bid_d[c] <= ask_d[c] and c > 0:
        c -= 1

    # draw tie length from log-normal
    room = max(2, n - 1 - c)
    hi = max(2, min(max_tie_len, room))
    k = sample_tie_length(rng, hi)

    mcv = int(ask_d[c])
    d_tail = int(bid_d[c + k]) if c + k < n else 0

    # ensure enough bid depth in the tail for the transitional tick
    needed = mcv - d_tail
    if needed <= 0:
        tail = bids_vol[c + k:]
        total_tail = int(tail.sum())
        if total_tail > 0 and c + k < n:
            factor = max(len(tail), mcv // 2) / total_tail
            bids_vol[c + k:] = np.maximum(np.round(tail * factor).astype(int), floor)
            d_tail = int(np.cumsum(bids_vol[::-1])[::-1][c + k])
        needed = max(floor, mcv - d_tail)

    # zero interior bid ticks to create the flat D(p)=mcv plateau
    if k >= 2:
        bids_vol[c:c + k - 1] = 0
    bids_vol[c + k - 1] = needed

    # randomly zero some interior ask ticks (safe: S(p)>=mcv holds since S(c)=mcv and all terms >=0)
    double_zero_prob = 0.35
    for i in range(c + 1, c + k - 1):
        if rng.random() < double_zero_prob:
            asks_vol[i] = 0

    # jitter surplus magnitude (bid side just before plateau, ask side across it)
    max_jitter = max(floor, int(surplus_jitter_factor * floor))
    if c > 0:
        bids_vol[c - 1] += int(rng.integers(0, max_jitter + 1))
    for i in range(c + 1, c + k):
        if asks_vol[i] > 0:
            asks_vol[i] += int(rng.integers(0, max_jitter + 1))

    return bids_vol, asks_vol, c, k


def generate_auction_random_tie(n_trades: int, seed: int = None,
                                 price_min: int = 70, price_max: int = 180,
                                 position_jitter: int = 3,
                                 surplus_jitter_factor: float = 2.0):
    rng = np.random.default_rng(seed)
    # one integer price tick per trade so book CSV has exactly n_trades rows
    prices = np.arange(price_min, price_min + max(2, n_trades))
    bids_vol, asks_vol, _c, _k = design_order_book_random_tie(
        n_trades, prices, rng,
        position_jitter=position_jitter,
        surplus_jitter_factor=surplus_jitter_factor,
    )
    trades = explode_volumes(bids_vol, asks_vol, prices, n_trades, rng)
    book = compute_order_book(bids_vol, asks_vol, prices)
    return trades, book


def validate_market(trades, book) -> bool:
    ok = True
    agg_ok = verify_aggregation(trades, book)
    print(f"  Aggregation matches designed volumes : {'PASS' if agg_ok else 'FAIL'}")
    ok &= agg_ok

    bd, ad = book['bid_depth'], book['ask_depth']
    bd_mono = bool(np.all(bd[:-1] >= bd[1:]))
    ad_mono = bool(np.all(ad[:-1] <= ad[1:]))
    print(f"  Bid depth non-increasing in price    : {'PASS' if bd_mono else 'FAIL'}")
    print(f"  Ask depth non-decreasing in price    : {'PASS' if ad_mono else 'FAIL'}")
    ok &= bd_mono and ad_mono

    both_zero = int(((book['bids_vol'] == 0) & (book['asks_vol'] == 0)).sum())
    n_zero_bid = int((book['bids_vol'] == 0).sum())
    n_zero_ask = int((book['asks_vol'] == 0).sum())
    tie_count = int(book['mcv_mask'].sum())
    print(f"  MCV tie level count (plateau length) : {tie_count}")
    print(f"  Ticks with zero Bids++               : {n_zero_bid}")
    print(f"  Ticks with zero Asks++               : {n_zero_ask}")
    print(f"  Ticks with BOTH sides zero            : {both_zero}")
    ok &= tie_count >= 2

    return ok


def main():
    parser = argparse.ArgumentParser(
        description="Generate a batch auction with a randomly-positioned MCV tie."
    )
    parser.add_argument("n_trades", type=int, nargs="?", default=None)
    parser.add_argument("--seed", type=int, default=None,
                        help="Omit for a fresh random tie each run.")
    args = parser.parse_args()

    n_trades = args.n_trades
    if n_trades is None:
        while True:
            raw = input("How many trades should be generated? ").strip()
            try:
                n_trades = int(raw)
                break
            except ValueError:
                print("Please enter a positive integer.")
    if n_trades <= 0:
        parser.error("n_trades must be a positive integer.")

    trades, book = generate_auction_random_tie(n_trades, seed=args.seed)

    print(f"\nGenerating {n_trades:,} trades with a randomized tie position/magnitude...\n")
    print("--- Market simulation validation ---")
    ok = validate_market(trades, book)
    print(f"\nOverall market simulation correctness: {'PASS' if ok else 'FAIL'}\n")

    print_order_book(book, f"Batch Auction Order Book (random tie, n = {n_trades:,} trades)")

    out_dir = os.path.dirname(__file__) or "."
    tfile = os.path.join(out_dir, f"trades_{n_trades}_random.csv")
    bfile = os.path.join(out_dir, f"order_book_{n_trades}_random.csv")

    trades_to_df(trades).to_csv(tfile, index=False)
    bdf = book_to_df(book)
    bdf.to_csv(bfile, index=False)

    print(f"Saved: {tfile}")
    print(f"Saved: {bfile}  ({len(bdf):,} rows)")

    if not ok:
        sys.exit(1)


if __name__ == "__main__":
    main()
