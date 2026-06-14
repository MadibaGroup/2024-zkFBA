import numpy as np
import pandas as pd
import sys

# ---------------------------------------------------------------------------
#  Step 1: Design aggregate bid/ask volumes with an engineered tie region

def design_order_book(n_trades: int, prices: np.ndarray, rng: np.random.Generator,
                      clear_frac: float = 0.40):
    """
    Design BidsVol[p] and AsksVol[p] with a guaranteed 2-level MCV tie.

    Tie structure (indices c and c+1):
        price c:   D > S = MCV  (bid surplus)     [PINK]
        price c+1: D = S = MCV  (Delta = 0)       [PINK + GREEN]

    Key identity used:
        Let MCV = S[c]  (cumulative ask up to c, unchanged by engineering)
        Let d_tail = D_initial[c+3]  (bids from c+3 onwards, also unchanged)

        Set  bids_vol[c+1] = MCV - d_tail      (so D[c+1] = MCV - d_tail + d_tail = MCV)
             bids_vol[c+2] = 0                 (so D[c+2] = d_tail <= MCV)
             asks_vol[c+1] = 0                 (so S[c+1] = S[c] = MCV)

        Then D[c]   = bids_vol[c] + MCV > MCV     (bid surplus)   ✓
             D[c+1] = (MCV - d_tail) + 0 + d_tail = MCV           ✓
             S[c+1] = MCV + 0 = MCV                               ✓
             Delta at c+1 = 0                                      ✓ GREEN

    Returns:
        bids_vol, asks_vol: np.ndarray of int, length = len(prices)
        cross_idx: index c where the tie begins
    """
    n = len(prices)
    ci = max(1, min(int(n * clear_frac), n - 4))

    avg_qty = 200
    half_vol = (n_trades * avg_qty) // 2
    x = np.arange(n, dtype=float)

    # BidsVol: bell curve centred slightly ABOVE crossing (aggressive buyers bid high)
    bid_mu = ci + n * 0.06
    bid_sig = n * 0.30
    raw_bid = np.exp(-0.5 * ((x - bid_mu) / bid_sig) ** 2) + 0.04
    bid_wts = raw_bid / raw_bid.sum()

    # AsksVol: bell curve centred slightly BELOW crossing (aggressive sellers ask low)
    ask_mu = ci - n * 0.06
    ask_sig = n * 0.30
    raw_ask = np.exp(-0.5 * ((x - ask_mu) / ask_sig) ** 2) + 0.04
    ask_wts = raw_ask / raw_ask.sum()

    bids_vol = np.round(bid_wts * half_vol * rng.uniform(0.88, 1.12, n)).astype(int)
    asks_vol = np.round(ask_wts * half_vol * rng.uniform(0.88, 1.12, n)).astype(int)
    bids_vol = np.maximum(bids_vol, 50)
    asks_vol = np.maximum(asks_vol, 50)

    # Compute initial depths and locate crossing
    bid_d = np.cumsum(bids_vol[::-1])[::-1].astype(int)
    ask_d = np.cumsum(asks_vol).astype(int)
    diff = bid_d.astype(float) - ask_d.astype(float)

    cross = ci
    for i in range(n - 1):
        if diff[i] >= 0 and diff[i + 1] <= 0:
            cross = i
            break
    c = min(cross, n - 4)

    # Guarantee D[c] > S[c]
    if bid_d[c] <= ask_d[c]:
        boost = int(ask_d[c] - bid_d[c]) + max(300, int(ask_d[c] * 0.12))
        bids_vol[c] += boost
        bid_d = np.cumsum(bids_vol[::-1])[::-1].astype(int)
        ask_d = np.cumsum(asks_vol).astype(int)

    # MCV is the cumulative ask at c (unchanged by engineering below)
    mcv = int(ask_d[c])

    # d_tail = bids from c+3 onwards (unchanged: we only touch c+1 and c+2)
    d_tail = int(bid_d[c + 3]) if c + 3 < n else 0

    # Scale tail down if it would exceed MCV (rare with well-calibrated bell curves)
    if d_tail > mcv:
        factor = mcv / max(d_tail, 1)
        bids_vol[c + 3:] = np.maximum((bids_vol[c + 3:] * factor).astype(int), 1)
        d_tail = int(np.cumsum(bids_vol[::-1])[::-1][c + 3]) if c + 3 < n else 0

    # --- Apply the tie engineering ---
    bids_vol[c + 1] = max(0, mcv - d_tail)   # D[c+1] = MCV,  Delta=0  GREEN
    bids_vol[c + 2] = 0                        # D[c+2] = d_tail ≤ MCV
    asks_vol[c + 1] = 0                        # S[c+1] = S[c]  = MCV
    asks_vol[c + 2] = max(asks_vol[c + 2], 50) # S[c+2] > MCV  (ask surplus after tie)

    return bids_vol, asks_vol, c


# ---------------------------------------------------------------------------
#  Step 2: Explode aggregate volumes into individual limit orders

def _split_volume(total: int, k: int, rng: np.random.Generator):
    """Split `total` units into exactly `k` positive integer parts."""
    if k == 1:
        return [total]
    if total <= k:
        # Not enough volume to give every order at least 1; pad down k
        return [1] * k  # caller should ensure total >= k
    # Pick k-1 cut points in [1, total-1] without replacement
    cuts = np.sort(rng.choice(np.arange(1, total), size=k - 1, replace=False))
    parts = np.diff(np.concatenate([[0], cuts, [total]])).astype(int).tolist()
    return parts


def explode_volumes(bids_vol: np.ndarray, asks_vol: np.ndarray,
                    prices: np.ndarray, n_trades: int,
                    rng: np.random.Generator):
    """
    Turn aggregate volume arrays into individual limit order records.

    The generated orders, when re-aggregated by (side, price),
    exactly reproduce bids_vol and asks_vol.  Total count ≈ n_trades.
    """
    n = len(prices)
    n_buys = n_trades // 2
    n_sells = n_trades - n_buys

    def allocate_counts(vol: np.ndarray, n_orders: int) -> np.ndarray:
        """Distribute n_orders across ticks proportional to vol."""
        counts = np.zeros(n, dtype=int)
        nonzero = vol > 0
        nz = int(nonzero.sum())
        if nz == 0:
            return counts
        if n_orders <= nz:
            # Give 1 to the top-n_orders ticks by volume
            top = np.argsort(vol)[::-1][:n_orders]
            counts[top] = 1
        else:
            counts[nonzero] = 1          # at least 1 order per active tick
            remaining = n_orders - nz
            probs = vol.astype(float) / vol.sum()
            extra = rng.multinomial(remaining, probs)
            counts += extra
        return counts

    buy_counts = allocate_counts(bids_vol, n_buys)
    sell_counts = allocate_counts(asks_vol, n_sells)

    trades = []
    for i, p in enumerate(prices):
        k, vol = int(buy_counts[i]), int(bids_vol[i])
        if k > 0 and vol > 0:
            k = min(k, vol)          # can't have more orders than units
            qtys = _split_volume(vol, k, rng)
            for q in qtys:
                trades.append({'side': 'BUY', 'limit_price': int(p), 'quantity': max(1, q)})

    for i, p in enumerate(prices):
        k, vol = int(sell_counts[i]), int(asks_vol[i])
        if k > 0 and vol > 0:
            k = min(k, vol)
            qtys = _split_volume(vol, k, rng)
            for q in qtys:
                trades.append({'side': 'SELL', 'limit_price': int(p), 'quantity': max(1, q)})

    # Shuffle order of arrival and assign trade IDs
    idx = rng.permutation(len(trades))
    trades = [trades[int(i)] for i in idx]
    for i, t in enumerate(trades):
        t['trade_id'] = i + 1

    return trades


# ---------------------------------------------------------------------------
#  Step 3: Compute the full order book with all zk-FBA columns

def compute_order_book(bids_vol: np.ndarray, asks_vol: np.ndarray,
                       prices: np.ndarray) -> dict:
    """
    Compute every column shown in the paper screenshots plus ZK gadget columns.

    Columns (matching first screenshot headers):
      Price, Bids++, Asks++,
      Bid Depth (D(p)), Ask Depth (S(p)),
      Min(Bid,Ask)=MCV (V(p)),
      Selector [1 if V(p)=Vmax], ln{0,MCV},
      Bid Surplus++ (D-min), Ask Surplus++ (S-min),
      Abs(Delta) |D-S|,
      Check on Delta [1 if |Delta|=min within tie],
      S*Delta in {0,surplus},
      Selector (MCI), MCV,
      Cliff value, Slack++ (MCV - V(p))

    Phase 1: Vmax = max V(p); if unique -> p* directly
    Phase 2: within tie region, minimise |Delta(p)|; lowest p as tertiary tie-break
    """
    bid_d = np.cumsum(bids_vol[::-1])[::-1].astype(int)
    ask_d = np.cumsum(asks_vol).astype(int)
    exec_v = np.minimum(bid_d, ask_d)
    vmax = int(exec_v.max())

    mcv_mask = exec_v == vmax
    bid_surplus = np.maximum(bid_d - ask_d, 0).astype(int)
    ask_surplus = np.maximum(ask_d - bid_d, 0).astype(int)
    signed_delta = (bid_d - ask_d).astype(int)
    abs_delta = np.abs(signed_delta)

    # Phase 2 tie-break: minimum |Delta| within the MCV region
    _large = np.iinfo(np.int64).max
    masked_delta = np.where(mcv_mask, abs_delta, _large)
    min_delta = int(masked_delta.min())

    clearing_candidates = np.where(mcv_mask & (abs_delta == min_delta))[0]
    clearing_idx = int(clearing_candidates[0])      # lowest-price tie-break
    clearing_price = int(prices[clearing_idx])

    # --- ZK gadget columns ---
    # Selector: 1-hot over prices achieving Vmax  (Check MCV is Global Max section)
    selector_mcv = mcv_mask.astype(int)

    # ln{0, MCV}: MCV value at selected prices, else 0
    ln0_mcv = np.where(mcv_mask, vmax, 0).astype(int)

    # Check on Delta: 1 where |Delta| = min within tie  (MCI is Global Min section)
    check_delta = (mcv_mask & (abs_delta == min_delta)).astype(int)

    # S*Delta in {0, surplus}: imbalance scaled by whether it's the clearing price
    # 0 at clearing price; bid/ask surplus elsewhere in the tie
    s_delta = np.where(check_delta.astype(bool), 0,
                       np.where(mcv_mask, abs_delta, 0)).astype(int)

    # Selector (MCI is Global Min section): 1 where Delta reaches its minimum in tie
    selector_mci = check_delta.copy()

    # Cliff value (One Cliff Example): surplus just outside the MCV tie boundary
    cliff_val = np.zeros(len(prices), dtype=int)
    tie_indices = np.where(mcv_mask)[0]
    if len(tie_indices) > 0:
        first_tie = int(tie_indices[0])
        last_tie = int(tie_indices[-1])
        if first_tie > 0:
            cliff_val[first_tie - 1] = int(bid_surplus[first_tie - 1])
        if last_tie < len(prices) - 1:
            cliff_val[last_tie + 1] = int(ask_surplus[last_tie + 1])

    # Slack++ = Vmax - V(p)  (0 inside tie, positive outside)
    slack = (vmax - exec_v).astype(int)

    return {
        'prices':         prices.astype(int),
        'bids_vol':       bids_vol.astype(int),
        'asks_vol':       asks_vol.astype(int),
        'bid_depth':      bid_d,
        'ask_depth':      ask_d,
        'exec_vol':       exec_v,
        'vmax':           vmax,
        'mcv_mask':       mcv_mask,
        'bid_surplus':    bid_surplus,
        'ask_surplus':    ask_surplus,
        'abs_delta':      abs_delta,
        'min_delta':      min_delta,
        'clearing_price': clearing_price,
        # ZK gadget columns
        'selector_mcv':   selector_mcv,
        'ln0_mcv':        ln0_mcv,
        'check_delta':    check_delta,
        's_delta':        s_delta,
        'selector_mci':   selector_mci,
        'cliff_val':      cliff_val,
        'slack':          slack,
    }


# ---------------------------------------------------------------------------
#  Main generation entry point

def generate_auction(n_trades: int, seed: int = 42,
                     price_min: int = 70, price_max: int = 180,
                     price_step: int = 10):
    """
    Generate a synthetic batch auction with n_trades individual limit orders.

    Returns:
        trades    : list of dicts  {trade_id, side, limit_price, quantity}
        book      : dict of numpy arrays (all order book columns)
    """
    rng = np.random.default_rng(seed)
    prices = np.arange(price_min, price_max + price_step, price_step)

    bids_vol, asks_vol, _cross = design_order_book(n_trades, prices, rng)
    trades = explode_volumes(bids_vol, asks_vol, prices, n_trades, rng)
    book = compute_order_book(bids_vol, asks_vol, prices)

    return trades, book


# ---------------------------------------------------------------------------
#  Verification: re-aggregate trades and check they match designed volumes

def verify_aggregation(trades, book):
    """Re-aggregate individual trades and compare to designed book."""
    prices = book['prices']
    n = len(prices)
    p2i = {int(p): i for i, p in enumerate(prices)}

    agg_bids = np.zeros(n, dtype=int)
    agg_asks = np.zeros(n, dtype=int)
    for t in trades:
        idx = p2i[t['limit_price']]
        if t['side'] == 'BUY':
            agg_bids[idx] += t['quantity']
        else:
            agg_asks[idx] += t['quantity']

    bid_ok = np.array_equal(agg_bids, book['bids_vol'])
    ask_ok = np.array_equal(agg_asks, book['asks_vol'])
    return bid_ok and ask_ok


# ---------------------------------------------------------------------------
#  Pretty-printer: column view matching paper screenshots

def print_order_book(book: dict, title: str = "Order Book"):
    """
    Print the full order book in the column format shown in the paper screenshots.

    Legend for the 'Note' column:
      PINK  = price is in the MCV tie region (Min(Bid,Ask) = Vmax)
      GREEN = price has minimum |Delta| within the tie  (candidate clearing price)
      [p*]  = actual clearing price after Phase 2 tie-break
    """
    vmax = book['vmax']
    cp = book['clearing_price']
    min_delta = book['min_delta']
    n = len(book['prices'])

    sep = "=" * 130
    thin = "-" * 130

    print(f"\n{sep}")
    print(f"  {title}")
    print(f"  Market Clearing Volume (Vmax): {vmax:,}")
    print(f"  Clearing Price p*:             ${cp}")
    print(f"  MCV Tie region:                prices where Min(Bid,Ask) = {vmax:,}  [PINK]")
    print(f"  Min |Delta| in tie:            {min_delta:,}  [GREEN]")
    print(sep)

    # ---- Header (matches screenshot column order) ----
    H = (
        f"{'Price':>7} | "
        f"{'Bids++':>8} | {'Asks++':>8} | "
        f"{'Bid Depth':>10} | {'Ask Depth':>10} | "
        f"{'Min(B,A)':>9} | "
        f"{'Sel':>3} | {'ln{0,MCV}':>9} | "
        f"{'BidSurp':>8} | {'AskSurp':>8} | "
        f"{'|Delta|':>8} | "
        f"{'ChkDelta':>8} | {'S*Delta':>8} | "
        f"{'SelMCI':>6} | {'MCV':>6} | "
        f"{'CliffVal':>9} | {'Slack++':>8} | Note"
    )
    print(H)
    print(thin)

    for i in range(n):
        p   = book['prices'][i]
        bv  = book['bids_vol'][i]
        av  = book['asks_vol'][i]
        bd  = book['bid_depth'][i]
        ad  = book['ask_depth'][i]
        ev  = book['exec_vol'][i]
        bs  = book['bid_surplus'][i]
        as_ = book['ask_surplus'][i]
        ad_ = book['abs_delta'][i]
        sel = book['selector_mcv'][i]
        ln  = book['ln0_mcv'][i]
        chk = book['check_delta'][i]
        sd  = book['s_delta'][i]
        sm  = book['selector_mci'][i]
        cv  = book['cliff_val'][i]
        slk = book['slack'][i]
        in_tie = bool(book['mcv_mask'][i])
        is_min = bool(book['check_delta'][i])

        notes = []
        if in_tie:
            notes.append("PINK")
        if in_tie and is_min:
            notes.append("GREEN")
        if p == cp:
            notes.append("<-- p*")

        row = (
            f"${p:>6} | "
            f"{bv:>8,} | {av:>8,} | "
            f"{bd:>10,} | {ad:>10,} | "
            f"{ev:>9,} | "
            f"{sel:>3} | {ln:>9,} | "
            f"{bs:>8,} | {as_:>8,} | "
            f"{ad_:>8,} | "
            f"{chk:>8} | {sd:>8,} | "
            f"{sm:>6} | {vmax:>6,} | "
            f"{cv:>9,} | {slk:>8,} | "
            + ", ".join(notes)
        )
        print(row)

    print(sep)
    print()


# ---------------------------------------------------------------------------
#  CSV export helpers

def trades_to_df(trades) -> pd.DataFrame:
    df = pd.DataFrame(trades)[['trade_id', 'side', 'limit_price', 'quantity']]
    df = df.sort_values('trade_id').reset_index(drop=True)
    return df


def book_to_df(book: dict) -> pd.DataFrame:
    return pd.DataFrame({
        'Price':              book['prices'],
        'Bids++':             book['bids_vol'],
        'Asks++':             book['asks_vol'],
        'Bid_Depth':          book['bid_depth'],
        'Ask_Depth':          book['ask_depth'],
        'Min_Bid_Ask':        book['exec_vol'],
        'Selector_MCV':       book['selector_mcv'],
        'ln0_MCV':            book['ln0_mcv'],
        'Bid_Surplus':        book['bid_surplus'],
        'Ask_Surplus':        book['ask_surplus'],
        'Abs_Delta':          book['abs_delta'],
        'Check_on_Delta':     book['check_delta'],
        'S_Delta':            book['s_delta'],
        'Selector_MCI':       book['selector_mci'],
        'MCV':                book['vmax'],
        'Cliff_Value':        book['cliff_val'],
        'Slack':              book['slack'],
        'Is_MCV_Tie':         book['mcv_mask'].astype(int),
        'Is_Min_Delta':       book['check_delta'],
        'Clearing_Price':     book['clearing_price'],
    })


# ---------------------------------------------------------------------------
#  Main


def main():
    PRICE_MIN  = 70
    PRICE_MAX  = 180
    PRICE_STEP = 10
    SEED       = 42

    for n_trades in [100, 1000]:
        print(f"\n{'#'*80}")
        print(f"#  Generating {n_trades:,} trades")
        print(f"{'#'*80}")

        trades, book = generate_auction(
            n_trades,
            seed=SEED,
            price_min=PRICE_MIN,
            price_max=PRICE_MAX,
            price_step=PRICE_STEP,
        )

        # Verification
        ok = verify_aggregation(trades, book)
        n_actual = len(trades)
        print(f"  Aggregation check : {'PASS ✓' if ok else 'FAIL ✗'}")
        print(f"  Orders generated  : {n_actual:,}  (target {n_trades:,})")
        print(f"  Clearing price p* : ${book['clearing_price']}")
        print(f"  Vmax (MCV)        : {book['vmax']:,}")

        tie_prices = book['prices'][book['mcv_mask']]
        green_prices = book['prices'][book['check_delta'].astype(bool)]
        print(f"  MCV tie (PINK)    : prices {list(tie_prices)} ({len(tie_prices)} levels)")
        print(f"  |Delta|=0 (GREEN) : prices {list(green_prices)} ({len(green_prices)} levels)")

        # --- Column-format order book printout (matches screenshot) ---
        print_order_book(book, f"Batch Auction Order Book  (n = {n_trades:,} trades)")

        # --- CSV exports ---
        tdf = trades_to_df(trades)
        tfile = f"trades_{n_trades}.csv"
        tdf.to_csv(tfile, index=False)

        bdf = book_to_df(book)
        bfile = f"order_book_{n_trades}.csv"
        bdf.to_csv(bfile, index=False)

        print(f"  Saved: {tfile}  ({len(tdf):,} rows)")
        print(f"  Saved: {bfile}  ({len(bdf):,} rows)\n")

    # Quick distribution sanity check
    print("\n--- Distribution sanity check (100-trade book) ---")
    _, book100 = generate_auction(100, seed=SEED,
                                   price_min=PRICE_MIN, price_max=PRICE_MAX,
                                   price_step=PRICE_STEP)
    prices = book100['prices']
    bd = book100['bid_depth']
    ad = book100['ask_depth']
    print(f"  Bid Depth range: {bd.min():,} .. {bd.max():,}  (should be non-increasing)")
    print(f"  Ask Depth range: {ad.min():,} .. {ad.max():,}  (should be non-decreasing)")
    bd_mono = all(bd[i] >= bd[i+1] for i in range(len(bd)-1))
    ad_mono = all(ad[i] <= ad[i+1] for i in range(len(ad)-1))
    print(f"  Bid Depth non-increasing: {bd_mono}")
    print(f"  Ask Depth non-decreasing: {ad_mono}")


if __name__ == '__main__':
    main()
