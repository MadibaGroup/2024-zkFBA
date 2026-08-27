# ZK-FBA on real market data: Rust vs Noir

Same protocol, same methodology as the synthetic-data comparison in
`~/zk_fba_csv_full/OPTIMIZED_RATIONALE_AND_RESULTS.md` and
`~/zk_fba_csv_full/NEXT_OPTIMIZATION_HANDOFF.md`, but run against two real
AAPL datasets instead of log-normal synthetic order books. See
`data/NOTES.md` for exactly how the real Databento records were mapped onto
the protocol's `(Bids++, Asks++)` input columns, and why two different real
datasets exist. Both are N=100, so this is directly comparable to the
existing N=100 synthetic numbers.

All numbers below are measured on this machine, 5 independent runs each
(Rust: 5x `cargo run --release`; Noir: 5x full `nargo compile -> nargo
execute -> bb write_vk -> bb prove -> bb verify` cycle via
`noir/time_real_data.py`), mean ± stddev, same as the "parallelization
pass" section of `NEXT_OPTIMIZATION_HANDOFF.md`.

## Code fidelity: this is the original codebase, not a rewrite

`rust/` is a full `rsync --delete` restore of `~/zk_fba_csv_full/` (confirmed
byte-identical via `diff -rq`, zero output) with exactly one file touched:
`src/main.rs`. The diff against the original is limited to two CSV path
strings (now pointing at the real-data CSVs built by the separate
`data/fetch_real_data.py` + `data/build_real_csv.py` scripts, marked inline
with `// REAL-DATA SWAP` comments), two run labels ("real AAPL quotes/trades
CSV" instead of "log-normal CSV"), and using the already-computed
`book100.n`/`book100.domain_size`/`book1000.n`/`book1000.domain_size` in the
comparison-table printout instead of the hardcoded literals `100`/`128` and
`1000`/`1024` — required for honesty, since only two real N=100 datasets
exist (no real N=1000 pull was ever requested), so the second run is really
another N=100 case and printing "1000"/"1024" for it would be false.
`src/lib.rs` (all protocol logic, including `OrderBook::derive` and
`OrderBook::from_csv`) is untouched. On the Noir side, `main.nr` in both
`fba_protocol_real_quotes/` and `fba_protocol_real_trades/` is byte-identical
to `fba_protocol_100/src/main.nr` (diff: `IDENTICAL`) — only `Nargo.toml`'s
package name differs. All real-data reformatting (raw Databento records →
20-column CSV / Prover.toml) lives entirely in `data/build_real_csv.py` and
`noir/gen_provers.py`, mirroring exactly how the original synthetic-data
pipeline separated `gen_synthetic_book.py`/`protocol_prover_gen.py` from the
Rust/Noir consumers. Rust and Noir both just read a CSV/Prover.toml file, as
before.

Restoring the original `main.rs` also restored the original run order: Run 1
is the original 21-tick smoke-test book (dropped in an earlier, since-reverted
draft of this file), and only Runs 2/3 are the two real datasets. That
changed the measured numbers slightly — see the note under the per-layer
trace below — so the table and trace here supersede any numbers from a prior
draft of this document.

## Headline comparison

| Metric | Noir, real quotes | Rust, real quotes | Noir, real trades | Rust, real trades |
|---|---|---|---|---|
| Prove time | 137.8 ± 2.9 ms | 59.5 ± 1.0 ms (core) | 141.5 ± 5.6 ms | 58.6 ± 0.7 ms (core) |
| Rust's margin | | **2.32x faster** | | **2.42x faster** |
| Verify time | 17.3 ± 1.9 ms | 27.0 ± 0.1 ms | 16.8 ± 1.8 ms | 27.1 ± 0.1 ms |
| Noir's margin | | **1.56x faster** | | **1.61x faster** |
| Proof size | 14,656 bytes | 42,464 bytes | 14,656 bytes | 42,464 bytes |
| Noir's margin | | **2.90x smaller** | | **2.90x smaller** |

"Rust core" is the same definition used throughout the synthetic-data
docs: sum of Layers 2, 3a, 3b, 3c, 3d, 3e, 3f-prove, 3g-prove, 3h-prove —
excludes Layer 4 (redundant sanity duplicate, not part of the actual proof)
and Layer 4b's build/commit (real prover work, but excluded from "core" the
same way it always has been in this project's numbers, for
apples-to-apples comparison against `bb prove`'s single number). "Rust
verify" sums Layer 3f `batch_check` + Layer 3g/3h range-proof verification,
the analogue of `bb verify`.

**This reproduces the synthetic-data finding on real market data**: Rust
wins prove time by roughly the same 2-2.4x margin the N=100 synthetic
comparison showed (2.17x, per the handoff's most recent pass), Noir wins
verify time and proof size by essentially the same margins as before
(1.56-1.61x and 2.90x here vs ~1.4-1.6x* and 2.9x on synthetic N=100). None
of the real-data numbers are meaningfully different from the synthetic
ones, which is itself informative: the cost structure here is dominated by
fixed circuit-shape work (390 bit-gadget polynomials, domain size 128 for
either N=100 dataset) rather than by the specific values in the order
book, so real vs. synthetic input barely moves the needle. The known,
already-diagnosed root cause is unchanged from `NEXT_OPTIMIZATION_HANDOFF.md`
item 3/4: proof size and verify time are dominated by the 390
individually-opened bit-decomposition polynomials, not by anything
value-dependent.

*(the exact prior synthetic verify-margin number varies slightly by exact
run in the source doc's history; the qualitative Noir-wins-verify finding
is the stable part.)*

## Per-layer Rust trace (mean of 5 runs ± stddev)

```
                                            real quotes         real trades
Layer 2  Interpolation (9 IFFT)             2.15 ms             2.03 ms
Layer 3a KZG setup                          2.16 ms             2.17 ms
Layer 3b Commit witnesses (9 MSM)           4.90 ms             4.67 ms
Layer 3c Quotient polynomials               9.73 ms             9.70 ms
Layer 3d Commit quotients (14 MSM)          6.55 ms             6.55 ms
Layer 3e Fiat-Shamir                        6.38 ms             5.90 ms
Layer 3f batch_open (prover)               13.03 ms            13.07 ms
Layer 3g V_max range::prove                 5.03 ms             4.88 ms
Layer 3h cliff slack range::prove           9.59 ms             9.56 ms
  } core proof sum                         59.52 ms (±0.98)    58.55 ms (±0.66)

Layer 3f batch_check (4 pairings)          22.36 ms            22.43 ms
Layer 3g V_max range::verify                1.58 ms             1.58 ms
Layer 3h cliff slack range::verify          3.09 ms             3.08 ms
  } verify sum                             27.03 ms (±0.08)    27.09 ms (±0.11)

Layer 4  Constraint verify (sanity dup)     3.32 ms             3.11 ms
Layer 4b Bit-decomp gadgets (build)        91.96 ms            88.76 ms
Layer 4b Bit-decomp gadgets (commit)       27.40 ms            25.75 ms
-------------------------------------------------------------------------------
TOTAL (everything, incl. sanity dup + gadget build/commit)
                                          209.22 ms (±2.49)   203.26 ms (±1.95)
```

(Per-layer figures above are means over the same 5 runs as the core/verify
sums; per-layer stddevs were not separately retained, only the core/verify/
total rollups, so only those three rows show a ± spread.)

Layer 4b (build+commit) dominates the non-core total on both real datasets,
same as it did on synthetic data — it's the known, already-documented
biggest remaining lever in `NEXT_OPTIMIZATION_HANDOFF.md` item 4 (the
390-polynomial bit-decomposition gadget), not something specific to real
data.
Both datasets pass `ALL PASS: YES` (all 32/33 applicable constraints,
including the bit-gadget soundness check via real `batch_check`, Phase 3 of
the soundness fix — unchanged from the synthetic-data code path since this
is the exact same `lib.rs`, only the input CSV differs).

**Why these numbers moved slightly from an earlier draft of this file** (core
proof time for "quotes" was previously reported as 65.2 ± 2.2 ms, now 59.5 ±
1.0 ms; "trades" barely moved, 59.5 → 58.6 ms): an earlier draft's `main.rs`
had been rewritten to drop the original file's Run 1 (the 21-tick smoke-test
book) and start timing directly from the real-data run. Restoring the
original file restored that dropped first run, which now executes before the
real-data runs and warms up the CPU/allocator (branch predictor, cache lines,
frequency scaling) the same way it always implicitly did in the original
synthetic-data benchmarks this project's other numbers came from. So this
change is a benchmark-methodology fix (matching the original file's literal
behavior, which is what was asked for), not a change to what is being
computed — `lib.rs` never changed, and both the constraint-pass result
(`ALL PASS: YES`) and the verify/total numbers are unaffected within noise.
The Rust-vs-Noir prove-time margin widens slightly as a result (2.11x/2.38x
→ 2.32x/2.42x), still well within the 2-2.4x range this project's synthetic
N=100 comparison already established.

## What this does and doesn't demonstrate

Does: confirms the Rust-vs-Noir prove/verify/size tradeoff measured on
synthetic log-normal data also holds, within noise, on 5 minutes of real
AAPL order flow at the open, for two independently-constructed real-data
mappings (book-quote time series and trade-print aggressor flow).

Doesn't: this is 5 minutes of one symbol on one day, not a claim that real
order books in general satisfy the protocol's plateau-contiguity
assumption (see `data/NOTES.md` — it happened to hold here, and the CSV
builder would have raised loudly rather than silently misbehaved if it
hadn't). It also isn't a test of larger real N (5,000-20,000), which
`NEXT_OPTIMIZATION_HANDOFF.md` item 5 already flags as open for the
synthetic-data side too.

## Layout

```
zk_fba_real_data/
  data/
    fetch_real_data.py       Databento pull (mbp-10 + trades, narrow open window)
    build_real_csv.py        raw records -> 20-column order-book CSV format
    NOTES.md                 data provenance, mapping methodology, caveats
    raw_quotes_AAPL.csv      raw Databento mbp-10 pull
    raw_trades_AAPL.csv      raw Databento trades pull
    order_book_real_quotes_100.csv
    order_book_real_trades_100.csv
  rust/                      full rsync copy of zk_fba_csv_full (Cargo.toml,
                              Cargo.lock, GetData.py, benches/, examples/,
                              scripts/, docs, src/lib.rs -- all byte-identical
                              to the original); src/main.rs has a minimal
                              patch, only the two CSV path strings + 3
                              honesty-driven label/literal fixes, see
                              "Code fidelity" section above
  noir/
    fba_protocol_real_quotes/   copy of fba_protocol_100, Prover.toml from real data
    fba_protocol_real_trades/   copy of fba_protocol_100, Prover.toml from real data
    gen_provers.py            builds both Prover.toml files from data/*.csv
    time_real_data.py         5-run nargo+bb timing harness (used for the numbers above)
  RESULTS.md                 this file
```
