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
    book_to_html,
)


def max_reasonable_tie_length(n_ticks: int, cap: int = 8) -> int:

#    Reasonable upper bound on tie-plateau length, as a fraction of the
    #price grid: ~8% of all ticks, capped at "cap".

    #There's no published market-microstructure ratio for this, a tie
    #at the exact maximum executable volume is already a deliberately
    #engineered, measure-zero event. This heuristic just keeps the
    #plateau "local": wide enough to demonstrate a multi-way tie-break,
    #narrow enough that the tie doesn't swallow the whole supply/demand
    #curve and stop looking like price discovery at all.

    return max(2, min(cap, round(0.08 * n_ticks)))


def sample_tie_length(rng: np.random.Generator, hi: int,
                       sigma: float = 0.55) -> int:
    
#    Draw a tie-plateau length k in [2, hi] from a log-normal, instead of
    #uniformly over [2, hi].

    #A uniform draw makes an 8-tick tie just as
    #likely as a 2-tick tie, which isn't realistic data for unit testing,
    #exact-MCV ties of any length are already a measure-zero event, so
    #most of them should be short (k=2 or 3), with longer plateaus
    #occurring as a decaying tail. A log-normal centered at k=2 (median
    #of the underlying draw = 1, since exp(mu)=1 here) gives exactly that
    #shape: a peak near the minimum and a right tail that can still reach
    #"hi" occasionally.
    
    if hi <= 2:
        return 2
    raw = rng.lognormal(mean=0.0, sigma=sigma)
    k = 2 + int(np.floor(raw))
    return int(np.clip(k, 2, hi))


def design_order_book_random_tie(n_trades: int, prices: np.ndarray,
                                  rng: np.random.Generator,
                                  clear_frac: float = 0.40,
                                  position_jitter: int = 3,
                                  surplus_jitter_factor: float = 2.0,
                                  max_tie_len: int = None):

#    Generalized MCV-tie mechanism with a RANDOMIZED tie length k >= 2.

    #Depths only need to be monotonic, not strictly monotonic, D(p) (and
    #S(p)) are allowed to plateau (flat run). Per the latest relaxation,
    #a price tick may have Bids++ zero, Asks++ zero, or BOTH zero, a
    #fully dead tick is fine; nothing in the math requires either side
    #to stay positive.

    #Construction for a plateau of length k starting at tick c:
    #    mcv := S(c)                         (ask depth baseline at c,
    #                                          NEVER touched after this,
    #                                          so mcv stays valid)
    #    Want D(p) = mcv for p = c .. c+k-1   (flat bid-depth plateau)
    #    => bids_vol[c .. c+k-2] = 0          (k-1 interior ticks)
    #    => bids_vol[c+k-1] = mcv - D(c+k)    (transitional, baseline tail)
    #Since S(p) = mcv + sum(asks_vol[c+1 .. p]) and every term in that sum
    #is >= 0 (zero allowed), S(p) >= mcv automatically for all p in
    #[c, c+k-1], this holds NO MATTER which of asks_vol[c+1 .. c+k-2]
    #are zeroed. So those interior ticks' Asks++ can ALSO be randomly
    #zeroed (independently of the bid-side zeroing) without breaking the
    #tie: min(D, S) = mcv throughout the plateau either way. This is what
    #lets a tick be fully dead (both sides zero) while the tie and the
    #overall monotonic depths stay sound.

    #asks_vol[c] itself is never touched (it's what defines mcv via
    #S(c)), and bids_vol[c+k-1] (the transitional tick) is never zeroed
    #(it's what closes the plateau back to mcv exactly).

    #k=2 reduces to the original mechanism (1 interior tick, which may or
    #may not be zeroed on the ask side too, depending on the random draw).

    n = len(prices)
    ci = max(1, min(int(n * clear_frac), n - 3))
    if max_tie_len is None:
        max_tie_len = max_reasonable_tie_length(n)

    avg_qty = 200
    half_vol = (n_trades * avg_qty) // 2
    x = np.arange(n, dtype=float)
    floor = max(5, n_trades // (8 * n))

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

    cross = ci
    for i in range(n - 1):
        if diff[i] >= 0 and diff[i + 1] <= 0:
            cross = i
            break

#    Randomize the tie anchor within a small window around the natural
    #crossing, so the tie still sits where the market actually clears.
    lo = max(1, cross - position_jitter)
    hi = min(n - 3, cross + position_jitter)
    c = int(rng.integers(lo, hi + 1)) if hi >= lo else min(cross, n - 3)

    if bid_d[c] <= ask_d[c] and c > 0:
        c -= 1

#    Randomize the tie length k in [2, max_tie_len], bounded by the
    #room actually available before the price grid runs out. Drawn from
    #a log-normal (not uniform): in real order books, exact-MCV ties are
    #rare events whose length is short most of the time with an
    #occasional longer plateau -- a heavy-right-tailed shape, not "every
    #length 2..8 equally likely". See sample_tie_length() below.
    room = max(2, n - 1 - c)
    hi = max(2, min(max_tie_len, room))
    k = sample_tie_length(rng, hi)

    mcv = int(ask_d[c])
    d_tail = int(bid_d[c + k]) if c + k < n else 0

    needed = mcv - d_tail
    if needed <= 0:
        tail = bids_vol[c + k:]
        total_tail = int(tail.sum())
        if total_tail > 0 and c + k < n:
            target_tail = max(len(tail), mcv // 2)
            factor = target_tail / total_tail
            bids_vol[c + k:] = np.maximum(np.round(tail * factor).astype(int), floor)
            d_tail = int(np.cumsum(bids_vol[::-1])[::-1][c + k])
        needed = max(floor, mcv - d_tail)

#    k-1 interior ticks of the plateau get zero NEW bid volume (their
    #cumulative bid depth stays at mcv, carried through). The final
    #tick of the plateau is the transitional, strictly positive tick
    #that ties D back down to mcv exactly.
    if k >= 2:
        bids_vol[c:c + k - 1] = 0
    bids_vol[c + k - 1] = needed

#    Independently and randomly ALSO zero some interior ticks' Asks++
    #(any of c+1 .. c+k-2 -- never c itself, since S(c) defines mcv).
    #As shown in the docstring this can never break S(p) >= mcv, so it's
    #safe; it just means some plateau ticks end up fully dead (both
    #sides zero), which is now an allowed market shape, not a bug.
    double_zero_prob = 0.35
    for i in range(c + 1, c + k - 1):
        if rng.random() < double_zero_prob:
            asks_vol[i] = 0

#    Randomize surplus magnitude WITHOUT breaking the tie: mcv = S(c) is
    #independent of bids_vol[c-1] (the baseline-surplus tick just before
    #the plateau), and the ask surplus growth across the plateau is
    #already controlled by asks_vol[c+1 .. c+k-1] -- jitter those a bit
    #for variety in how close/far the Phase-2 tie-break looks (only the
    #ticks that weren't just zeroed out above).
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
    """Like generate_auction.generate_auction(), but with a randomized tie."""
    rng = np.random.default_rng(seed)  # seed=None -> different tie every run

#    One price tick per trade requested, so the order book file ends up
    #with exactly n_trades rows. The parent script's compute_order_book /
    #explode_volumes both int-cast prices (book['prices'].astype(int),
    #trade['limit_price'] = int(p)), so ticks must be distinct integers --
    #a fine float linspace over the fixed [70, 180] span would collapse
    #under that cast once n_trades exceeds the span. Instead, step by 1
    #starting at price_min for exactly n_trades consecutive integer
    #ticks (ignoring price_max, which no longer constrains the count).
    n_ticks = max(2, n_trades)
    prices = np.arange(price_min, price_min + n_ticks)

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

#   Fully dead ticks (both sides zero) are now an allowed market shape
    #(plateau in both D and S simultaneously) -- informational, not a
    #failure condition.
    both_zero = int(((book['bids_vol'] == 0) & (book['asks_vol'] == 0)).sum())
    n_zero_bid_ticks = int((book['bids_vol'] == 0).sum())
    n_zero_ask_ticks = int((book['asks_vol'] == 0).sum())
    tie_count = int(book['mcv_mask'].sum())
    print(f"  MCV tie level count (plateau length) : {tie_count}")
    print(f"  Ticks with zero Bids++               : {n_zero_bid_ticks}")
    print(f"  Ticks with zero Asks++               : {n_zero_ask_ticks}")
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
    hfile = os.path.join(out_dir, f"order_book_{n_trades}_random_highlighted.html")

    trades_to_df(trades).to_csv(tfile, index=False)
    bdf = book_to_df(book)
    bdf.to_csv(bfile, index=False)
    with open(hfile, "w") as f:
        f.write(book_to_html(book, n_trades))

    print(f"Saved: {tfile}")
    print(f"Saved: {bfile}  ({len(bdf):,} rows)")
    print(f"Saved: {hfile}")

    if not ok:
        sys.exit(1)


if __name__ == "__main__":
    main()
