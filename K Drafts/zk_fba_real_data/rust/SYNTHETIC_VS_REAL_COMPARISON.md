# Rust vs Noir: Synthetic Data vs Real Data, Unified Comparison

This file merges two comparisons that were previously written up separately,
using one consistent set of definitions so the four columns (synthetic
N=100, synthetic N=1000, real quotes N=100, real trades N=100) can be read
side by side:

- **Synthetic**: `~/zk_fba_csv_full/OPTIMIZED_RATIONALE_AND_RESULTS.md` +
  the "Update 2026-08-24: parallelization pass" section of
  `~/zk_fba_csv_full/NEXT_OPTIMIZATION_HANDOFF.md` — log-normal synthetic
  order books, N=100 and N=1000.
- **Real**: `~/zk_fba_real_data/RESULTS.md` — two independently-built real
  AAPL order-book mappings (100 quote snapshots, 100 aggressor-side trade
  prints), both N=100, Databento `XNAS.ITCH`, 2026-06-01 09:30-09:35 ET.

In all four cases the code under test is the same: `lib.rs` and `main.nr`
are unchanged between the synthetic and real runs — only the input CSV /
`Prover.toml` differs (see `RESULTS.md`'s "Code fidelity" section for how
that was verified). That's what makes a synthetic-vs-real comparison
meaningful here: any difference in the numbers below is attributable to the
data, not to different code paths.

## Methodology asymmetry, disclosed up front

The four columns are not all measured the same way, and this matters more
than it might look:

| | Synthetic N=100 | Synthetic N=1000 | Real quotes/trades N=100 |
|---|---|---|---|
| Rust numbers | 5-run mean ± stddev | 5-run mean ± stddev | 5-run mean ± stddev |
| Noir numbers | **1 reported run** | **1 reported run** | 5-run mean ± stddev |

The synthetic-data Noir figures (0.13s/0.45s prove, 0.02s/0.01s verify) are
single point estimates carried over from the original Noir doc — that
codebase was never re-benchmarked as part of the Rust optimization passes.
The real-data Noir figures were freshly measured 5 times each via
`noir/time_real_data.py`. Rust is measured consistently (5-run mean) in
every column. This doesn't appear to change any conclusion below — the
real-data 5-run Noir mean (137.8ms at N=100) lands close to the synthetic
single-run N=100 figure (130ms) — but it's a real asymmetry, not glossed
over here.

## Headline comparison, unified

"Rust core" / "Rust batch_check" / "Rust full verify" definitions are held
fixed across all four columns (see "Verify time, two definitions" below for
why there are two verify rows).

| Metric | Synthetic N=100 | Synthetic N=1000 | Real quotes N=100 | Real trades N=100 |
|---|---|---|---|---|
| Domain size | 128 | 1024 | 128 | 128 |
| **Noir prove** | 130 ms (1 run) | 450 ms (1 run) | 137.8 ± 2.9 ms (5-run) | 141.5 ± 5.6 ms (5-run) |
| **Rust prove (core)** | 59.8 ± 0.3 ms | 109.3 ± 2.7 ms | 59.5 ± 1.0 ms | 58.6 ± 0.7 ms |
| Rust's prove margin | **2.17x faster** | **4.12x faster** | **2.32x faster** | **2.42x faster** |
| **Noir verify** | 20 ms (1 run) | 10 ms (1 run) | 17.3 ± 1.9 ms (5-run) | 16.8 ± 1.8 ms (5-run) |
| **Rust verify, batch_check only** | ~22.4 ms | 22.44 ± 0.07 ms | 22.36 ms | 22.43 ms |
| Noir's margin vs. batch_check only | ~1.12x faster | ~2.24x faster | ~1.29x faster | ~1.33x faster |
| **Rust verify, full (batch_check + range proofs)** | not separately reported* | not separately reported* | 27.03 ± 0.08 ms | 27.09 ± 0.11 ms |
| Noir's margin vs. full verify | n/a | n/a | **1.56x faster** | **1.61x faster** |
| **Proof size** | 14,656 vs 42,464 B | 14,656 vs 42,464 B | 14,656 vs 42,464 B | 14,656 vs 42,464 B |
| Noir's size margin | **2.90x smaller** | **2.90x smaller** | **2.90x smaller** | **2.90x smaller** |

\* The synthetic-data docs' headline "Rust verify" number only ever cited
`batch_check` (Layer 3f-verify) — the Layer 3g/3h range-proof verify steps
were never broken out as a separate reported figure in those documents,
even though `main.rs` computes and prints them on every run. The real-data
work in this project instrumented the fuller definition (`batch_check` +
range-proof verify, the actual full cost a verifier pays, analogous to what
`bb verify` does end-to-end) — see "Rust verify" per-layer table below,
where the ~4.6-4.7ms gap between the two definitions is visible directly.
This is a documentation gap in the earlier synthetic-data write-ups, not a
difference in what was computed; nothing suggests the missing ~4.6ms would
be different for synthetic data, since it's driven by fixed
`RANGE_BITS=16` range-proof structure, not the input values (see next
section).

## The recurring finding: fixed-shape costs don't move with N or with real vs. synthetic data

The most useful cross-check the real-data run provides is on exactly the
layers that *shouldn't* depend on N or on which dataset is used, because
they're sized by fixed circuit parameters (`BIT_WIDTH=32`, `RANGE_BITS=16`,
390 bit-gadget polynomials, 6 gadget instances) rather than by n or by the
order-book values themselves:

| Layer | Synthetic N=1000 (5-run mean) | Real quotes N=100 (5-run mean) | Real trades N=100 (5-run mean) |
|---|---|---|---|
| L3f batch_check (4 pairings) | 22.44 ± 0.07 ms | 22.36 ms | 22.43 ms |
| L3g V_max range::prove | 4.89 ± 0.24 ms | 5.03 ms | 4.88 ms |
| L3h cliff slack range::prove | 9.47 ± 0.26 ms | 9.59 ms | 9.56 ms |
| Proof size | 42,464 B | 42,464 B | 42,464 B |

These four rows are within noise of each other **despite N differing by
10x** (1000 vs 100) and despite one column being real market data and two
being real market data of a completely different kind (quote sizes vs.
trade sizes) against a synthetic log-normal baseline. That's the direct
empirical confirmation, not just the theoretical claim, behind
`NEXT_OPTIMIZATION_HANDOFF.md`'s root-cause diagnosis: proof size and a
fixed chunk of verify time are dominated by the 390 individually-opened
bit-decomposition polynomials, a cost that is a property of the circuit's
shape, not of n or of the specific numbers flowing through it.

By contrast, every genuinely N-dependent layer scales the way you'd expect
between N=1000 (domain 1024) and the N=100 real datasets (domain 128) — see
the full per-layer table below.

## Full per-layer Rust trace, all measured columns

Real quotes/trades come from `RESULTS.md`; synthetic N=1000 comes from the
"parallelization pass" section of `NEXT_OPTIMIZATION_HANDOFF.md` (the only
per-layer trace with a 5-run mean available on the synthetic side — no
equivalent N=100 synthetic per-layer breakdown was reported in the source
docs, only the summary numbers in the headline table above).

```
                                    Synth N=1000    Real quotes N=100   Real trades N=100
Layer 2  Interpolation (9 IFFT)      5.34ms (±0.37)   2.15ms               2.03ms
Layer 3a KZG setup                   3.69ms (±0.15)   2.16ms               2.17ms
Layer 3b Commit witnesses (9 MSM)   11.86ms (±0.94)   4.90ms               4.67ms
Layer 3c Quotient polynomials       25.40ms (±1.23)   9.73ms               9.70ms
Layer 3d Commit quotients (14 MSM)  15.55ms (±1.88)   6.55ms               6.55ms
Layer 3e Fiat-Shamir                10.26ms (±0.05)   6.38ms               5.90ms
Layer 3f batch_open (prover)        22.81ms (±0.64)  13.03ms              13.07ms
Layer 3g V_max range::prove          4.89ms (±0.24)   5.03ms               4.88ms
Layer 3h cliff slack range::prove    9.47ms (±0.26)   9.59ms               9.56ms
  } core proof sum                 109.3 ms          59.52 ms             58.55 ms

Layer 3f batch_check (4 pairings)   22.44ms (±0.07)  22.36ms              22.43ms
Layer 3g V_max range::verify         not reported*     1.58ms               1.58ms
Layer 3h cliff slack range::verify   not reported*     3.09ms               3.08ms
  } verify sum (batch_check only)   22.44 ms          22.36 ms             22.43 ms
  } verify sum (full, incl. range)  not reported*     27.03 ms             27.09 ms

Layer 4b Bit-decomp gadgets (build) 282.31ms (±1.26)  91.96ms              88.76ms
Layer 4b Bit-decomp gadgets (commit)114.44ms (±0.49)  27.40ms              25.75ms
```

\* see the disclosure above — the synthetic-data docs never printed these
two lines separately, even though the code computes them on every run.

N-dependent layers (2, 3a-3e, 3f-prove, 4b) are all roughly proportional to
domain size (1024 vs 128, an 8x ratio) modulated by each layer's own
asymptotic class — L3c (`O(n log n)` FFT) and L4b build/commit (`O(n log
n)` per gadget instance, dominated by 6 x 32-way FFT work) show the
steepest scaling, consistent with them being the FFT-heavy layers. The
N-independent layers (3f-verify, 3g/3h) are flat, as expected.

## Circuit / constraint size, unchanged real vs. synthetic

Real quotes and real trades both compile the byte-identical
`fba_protocol_100/src/main.nr` circuit (confirmed via `diff`, see
`RESULTS.md`), so their gate/opcode counts are the same as synthetic N=100
by construction — this wasn't re-measured separately per dataset since the
circuit source never changes, only `Prover.toml`'s witness values do:

| Metric | N=100 (synthetic, real quotes, real trades — same circuit) | N=1000 (synthetic only) |
|---|---|---|
| Noir gate count | 13,736 gates | 111,611 gates |
| Noir ACIR opcodes | 6,326 opcodes | 63,026 opcodes |
| Rust KZG commitments | 23 base + 390 bit-gadget, all pairing-opened | 23 base + 390 bit-gadget, all pairing-opened |
| Rust domain size | 128 | 1024 |

## Complexity summary (holds for both synthetic and real data)

This table is structural — it describes the algorithms, not any particular
dataset — so it applies unchanged to every column above. Reproduced here
(from `OPTIMIZED_RATIONALE_AND_RESULTS.md`) because the real-data run is
direct empirical confirmation that it doesn't change with real input:

| Factor | Rust/arkworks scaling | Noir/Barretenberg scaling | Winner at large N |
|---|---|---|---|
| Quotient polynomial computation | `O(n log n)` coset FFT | `O(n log n)` coset NTT | Tie, same mechanism |
| MSM throughput | `O(n/log n)`, 1 thread (rayon-parallelized within a call) | `O(n/(k log n))`, many threads | Noir, still a real constant-factor gap |
| Constraint coverage per proof | 32 of 32 fully bound and pairing-opened | All 33, in one proof | Tie, both sound |
| Proof size | 42,464 bytes, flat regardless of N or data source | 14,656 bytes, flat | Noir, ~2.9x, structural |
| Verify time | `O(1)` pairings + `O(total polys)` scalar mults to fold groups | UltraHonk verifier | Noir, structural |

Proof size and verify time don't scale with n because `BIT_WIDTH=32` /
`RANGE_BITS=16` are fixed dataset parameters, not n-dependent ones — the
real-data numbers above are the empirical proof of that claim, not just the
theoretical one.

## What changes between synthetic and real data, and what doesn't

**Doesn't change:** the qualitative Rust-wins-prove / Noir-wins-verify /
Noir-wins-size finding, the ~2-2.4x prove margin at N=100 (synthetic 2.17x,
real 2.32x/2.42x), the 2.90x proof-size margin (exact, all four columns),
the fixed-cost layers (batch_check, range proofs) to within a few percent
despite a 10x N difference and real-vs-synthetic data source.

**Does change, but for a known, disclosed reason, not a real effect:** the
Noir verify margin looks larger against real data (1.56x/1.61x) than the
synthetic headline number (~1.12x at N=100) — but that's comparing
different definitions of "Rust verify" (batch_check-only for synthetic vs.
batch_check+range for real), not a real difference; see the two-definition
table above. The batch_check-only numbers alone (22.4ms synthetic vs.
22.36-22.43ms real) agree almost exactly.

**Genuinely dataset-dependent, by construction, not by measurement:** each
dataset's own derived values — `V_max` (55,000 synthetic by construction,
1,588 real quotes, 251 real trades), plateau width (`c,d` — real quotes has
a degenerate 1-tick plateau, real trades has a 3-tick plateau, synthetic is
constructed to have a clean wide plateau), and `p_star`. None of these
values feed back into prove/verify time or proof size in this protocol,
since KZG commitment/opening cost is a function of polynomial degree and
group structure, not of the field elements' magnitudes (as long as they
fit under `RANGE_BITS`/`BIT_WIDTH`, which all three datasets do
comfortably — see `data/NOTES.md`).

## Sources

- `~/zk_fba_csv_full/OPTIMIZED_RATIONALE_AND_RESULTS.md`
- `~/zk_fba_csv_full/NEXT_OPTIMIZATION_HANDOFF.md`
- `~/zk_fba_real_data/RESULTS.md`
- `~/zk_fba_real_data/data/NOTES.md`
