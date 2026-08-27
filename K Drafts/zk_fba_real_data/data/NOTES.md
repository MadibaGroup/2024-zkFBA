# Real-data provenance and mapping notes

Source: Databento, `XNAS.ITCH` dataset, symbol `AAPL`, **2026-06-01
09:30:00-09:35:00 America/New_York** (first 5 minutes of the regular
session — the busiest window of the trading day, and the window
`~/zk_fba_csv_full/GetData.py` was already implicitly aimed at before this
was narrowed down from a full-day pull). Total Databento cost for both
pulls: ~$0.02 (checked via `client.metadata.get_cost` before fetching).

## Why two datasets, and why they're built differently

The ZK-FBA protocol consumes a `(Bids++, Asks++)` pair per price tick and
derives everything else (cumulative depth, surplus, delta, the clearing
plateau) from those two columns alone — see `OrderBook::from_csv` in
`zk_fba_csv_full/src/lib.rs` and `derive()` in
`zk_fba_noir/protocol_prover_gen.py`. The synthetic generator
(`gen_synthetic_book.py`) treats "Price" as a plain sequence index (0..n-1),
not a literal dollar price. Real market data doesn't come pre-packaged as a
100-level resting-depth curve, so two different, independently-reasonable
mappings from raw Databento records onto that same 2-column shape were used:

**`order_book_real_quotes_100.csv`** — 100 successive `mbp-10` top-of-book
snapshots, in event order. `Bids++` = `bid_sz_00`, `Asks++` = `ask_sz_00` at
each snapshot. "Price" is the snapshot's sequence index, exactly like the
synthetic generator's convention. This captures real, correlated
order-size dynamics at the open, at the cost of not being a literal
price-level depth curve (it's a time series of best-of-book size).

**`order_book_real_trades_100.csv`** — 100 real executed trades, `side in
{B, A}` only. The first ~150 raw prints of the session are dominated by the
NASDAQ opening-cross batch, which Databento tags `side='N'` (unclassified,
not a continuous-market aggressor trade) — those are skipped rather than
used. Each of the 100 remaining trades becomes one tick: its `size` goes
into `Bids++` if `side=='B'`, into `Asks++` if `side=='A'`, 0 in the other
column. Direction was verified empirically, not assumed: matching each
trade against the nearest preceding quote, `side='B'` trades print at the
best ask (buyer-initiated, "lifts the offer") 48.8% of the time vs 0.16% at
the bid, and `side='A'` trades print at the best bid (seller-initiated,
"hits the bid") 50.8% of the time vs 0.07% at the ask — consistent with
Databento's documented aggressor-side convention. (Neither hits 100%
because `merge_asof` against a coarsely-sampled quote series is an
approximation, not because the side tag itself is ambiguous.)

## Structural constraint that had to hold by luck, not construction

The protocol assumes `Min(X) = min(AccA(X), AccB(X))` is unimodal, so its
set of ties at the maximum (the "plateau") is a contiguous range `[c, d]`.
The synthetic generator enforces this by constructing smooth log-normal
curves specifically so it holds. Real order flow has no such guarantee —
`build_real_csv.py`'s `derive()` asserts contiguity and would have raised
loudly (not produced a silently-wrong CSV) had either real dataset violated
it. Both happened to satisfy it: quotes has a degenerate one-tick plateau
(`c=d=39`), trades has a 3-tick plateau (`c=53, d=55`). This is disclosed
here because it is a real assumption resting on 5 minutes of one symbol on
one day, not a property proven to hold for real order flow in general.

## Range-check bounds

`RANGE_BITS=16` (`V_max < 65536`) and `BIT_WIDTH=32` (all committed values
< 2^32) are unmodified from the synthetic-data setup. Real values fit
comfortably under both without any rescaling: quotes dataset `V_max=1588`,
trades dataset `V_max=251` — both real market sizes are much smaller than
the 65536 ceiling was ever exercising with synthetic data (`V_max=55000` by
construction there), so this real-data run is, if anything, a less
adversarial test of the range gadgets than the synthetic one.

## Reproducing

```
cd ~/zk_fba_real_data/data
python3 fetch_real_data.py    # Databento pull, ~$0.02, writes raw_*.csv
python3 build_real_csv.py     # writes the two order_book_real_*_100.csv
```
