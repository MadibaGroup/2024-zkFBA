# Handoff: Optimizing the Rust ZK-FBA Prover Further

Written to let a fresh conversation continue this work without re-reading the
whole prior session. Everything below is verified against the actual code as
of this writing (all tests pass, `cargo run --release` gives `ALL PASS: YES`
on 21/100/1000-tick datasets, Phase 3 bit-gadget soundness fix is complete).

**2026-08-24 update: items 1 and 2 below are now DONE, and Rust has retaken
the prove-time lead over Noir at both N=100 and N=1000.** See "Update
2026-08-24: parallelization pass" near the end of this file for what changed,
the corrected finding about the `parallel` feature (it turns out to have
already been on the whole time — the premise in item 1 below was wrong), and
the new measured numbers. The "Current state" numbers table right below this
paragraph is now stale; the update section has the current one.

**How to use this file**: in a new conversation, just point Claude at this
one file (`~/zk_fba_csv_full/NEXT_OPTIMIZATION_HANDOFF.md`). It's the single
source of truth for "what's already done" and "what's left"; no need to
re-paste prior conversation history. Re-verify the build/test/run status in
the new session before trusting the numbers below, in case anything changed
between sessions (`cd ~/zk_fba_csv_full && cargo build --release && cargo
test --release && cargo run --release 2>&1 | grep "ALL PASS"`).

## Repo / GitHub upload status

Not a git repo yet. When ready to push:
- Add `.gitignore` with `/target` and `.DS_Store` (target/ doesn't exist as
  a tracked dir yet, but will once someone builds).
- Commit: `src/lib.rs`, `src/main.rs`, `Cargo.toml`, `Cargo.lock`,
  `benches/fba_bench.rs`, `examples/bench_large.rs`,
  `scripts/gen_synthetic_book.py`, and the `.md` docs (`RATIONALE_AND_RESULTS.md`,
  `OPTIMIZED_RATIONALE_AND_RESULTS.md`, `PROTOCOL_DESIGN_AND_RESULTS.md`,
  this file).
- Skip or merge in: `Optimized_Rust.md`, `Result_compare.md` (stale informal
  notes from before the FFT/Phase-3 work, numbers in them are superseded by
  `OPTIMIZED_RATIONALE_AND_RESULTS.md`).
- **Known portability issue, not yet fixed**: `src/main.rs`,
  `benches/fba_bench.rs`, and `examples/bench_large.rs` all hardcode absolute
  paths like `/Users/kimiaesmaili/Downloads/order_book_100_log-normal.csv`.
  Fine locally, breaks for anyone else cloning the repo or any CI. Fix before
  treating this as shareable: check small sample CSVs into a `data/` dir
  with relative paths, or gate the paths behind an env var.

## Files to hand over

- `~/zk_fba_csv_full/src/lib.rs`, the library. Core functions to know:
  `compute_quotients` (calls `divide_by_vanishing_coset` 9 times per run,
  each rebuilding its own coset domain), `compute_all_opening_proofs`
  (builds the `BatchCheckProof`, 3 main opening groups + up to 2 cliff-value
  groups), `commit_all_bit_gadgets` / `build_bit_gadget_set` (the 390
  bit-decomposition polynomials, `BIT_WIDTH = 32`, 6 instances for
  constraints #1/#2/#8/#15/#16/#23), `full_pipeline` (orchestrates
  everything).
- `~/zk_fba_csv_full/src/main.rs`, hand-mirrors `full_pipeline` with
  per-layer `Instant` timing printed to stdout, plus a proof-size accounting
  block (search for `PROOF SIZE`) that counts real elements from the actual
  `BatchCheckProof` structs rather than estimating.
- `~/zk_fba_csv_full/Cargo.toml`. Correction as of 2026-08-24: `cargo tree -f
  "{p} [{f}]" -e features` shows `ark-ec`, `ark-ff`, `ark-poly`, `ark-std` all
  already resolve with `parallel,rayon` active, cascaded transitively from
  `ark-poly-commit`'s own `default = ["std", "parallel"]`. No `features =
  [...]` line was ever needed on this project's own dependency lines; item 1
  below was investigating a non-problem. `rayon = "1"` was added as a direct
  dependency instead, for the actual fix (see the update section).
- `~/zk_fba_csv_full/benches/fba_bench.rs`, Criterion harness,
  `cargo bench --bench fba_bench -- full_pipeline` to filter to just the
  full-pipeline group (takes 1-2 minutes; other groups exist too).
- `~/zk_fba_csv_full/OPTIMIZED_RATIONALE_AND_RESULTS.md`, full writeup of
  everything through Phase 3 completion: the FFT quotient fix, the
  `verify_all` O(n^2)-to-O(n log n) fix, the three-phase bit-gadget
  soundness fix, and the updated (now honest, post-Phase-3) Noir comparison.
  Read this before doing anything else, it has the full reasoning behind
  every number below.
- `~/zk_fba_csv_full/RATIONALE_AND_RESULTS.md`, original design doc
  (constraint list, why bit-decomposition over Plookup was chosen
  originally, MadibaGroup gadget reuse). Still accurate, unchanged.
- `~/.cargo/git/checkouts/2024-gadgets-code-78319039a72d90cf/aa90ddc/src/utils.rs`,
  the MadibaGroup `gadgets` crate source (`batch_open`, `batch_check`,
  `BatchCheckProof`, `OpenEval`). Needed if attempting item 3 or 4 below,
  since both require understanding or patching this crate's opening API.
- `~/zk_fba_noir/PROTOCOL_DESIGN_AND_RESULTS.md`, the Noir/Barretenberg
  side, only needed for item 5 (re-benchmarking).
- CSVs for larger N already exist at `~/Downloads/order_book_{100,1000,
  5000,10000,20000}_log-normal.csv`, no need to regenerate them.

Not a git repo yet (`~/zk_fba_csv_full` has no `.git`). `protocol_constraints.md`
referenced in comments does not exist on disk, despite being cited as "the
formal spec" throughout `lib.rs` and both `.md` docs.

## Current state (verified, do not re-derive)

Phase 1 (transcript binding), Phase 2 (residue folding), and Phase 3 (real
KZG pairing-verified opening) of the bit-gadget soundness fix are all done.
`opening_ok` in `full_pipeline` now genuinely covers all 32 constraints via
`batch_check`. There's a negative-control test in `lib.rs`
(`bit_gadget_opening_rejects_swapped_commitment`) proving this is
load-bearing, not decorative, it swaps a bit-gadget commitment and confirms
`batch_check` rejects it. 4/4 unit tests pass.

Real, measured numbers (not estimates):

| Metric | Noir, N=100 | Rust, N=100 | Noir, N=1000 | Rust, N=1000 |
|---|---|---|---|---|
| Prove time | 0.13 s | ~137-161 ms | 0.45 s | ~210-247 ms |
| Verify time | 0.02 s | ~22-28 ms | 0.01 s | ~22-28 ms |
| Proof size | 14,656 bytes | 42,464 bytes (flat, all N) | 14,656 bytes | 42,464 bytes |

Rust wins prove time at N=1000 (~2x), is roughly tied at N=100, and loses on
proof size (~2.9x bigger) and verify time (up to ~2.8x slower) at both sizes.
Proof size and verify time don't scale with n (they're dominated by the
fixed-width bit-gadget group, `BIT_WIDTH = 32`/`RANGE_BITS = 16` regardless
of dataset size), so these two gaps won't close by re-measuring at larger N.

Root cause of the size/verify gap: `compute_all_opening_proofs`'s Group 2
opens 390 bit-gadget polynomials individually. Each one costs 1 G1
commitment + 2 field elements (`OpenEval::Plain(value, blinding)`, this
crate's `batch_open` always uses `Plain`, never the more compact
`Committed` variant) in the proof. 390 * (1 G1 + 2 Fr) = 390 G1 + 780 Fr,
the bulk of both the byte count and the verify-time cost (verify is O(1)
pairings but O(total polynomials across all groups) elliptic-curve scalar
multiplications to fold each group before the final pairing check, that
folding cost is what grew from ~3ms to ~22-28ms).

## Optimization roadmap, in priority order

### 1. Enable arkworks' `parallel` feature — DONE, but not the way this item assumed

Investigated 2026-08-24: the premise was wrong. `cargo tree -f "{p} [{f}]" -e
features` shows `ark-ec`, `ark-ff`, `ark-poly`, `ark-std` all already resolve
with `parallel,rayon` active — cascaded transitively from
`ark-poly-commit`'s own `default = ["std", "parallel"]`, with no explicit
`features = [...]` needed on this crate's own `Cargo.toml` lines. So there
was no feature to "turn on". arkworks' `parallel` feature only parallelizes
*inside* a single FFT or MSM call, though — it does nothing for hand-written
sequential loops that call those primitives many times over independent
inputs, which is exactly what the real bottlenecks (Layer 3f's `batch_open`
over 390 polynomials, and the 390-polynomial commit loop in
`commit_bit_gadget`) were. The actual fix: added `rayon = "1"` as a direct
dependency and wrote `parallel_batch_open` / `parallel_commit_many` (new
functions in `lib.rs`) as rayon-parallelized, math-identical reimplementations
of the outer loops in `gadgets::utils::batch_open` and in this crate's own
commit loop — see the update section near the end of this file for details
and numbers.

### 2. Memoize coset-FFT setup inside `compute_quotients` — DONE

Added a `CosetCache` struct in `lib.rs` (build-once-per-domain-size, keyed
lookup) and threaded it through `compute_quotients` and `build_bit_gadget` /
`build_bit_gadget_set` (via `divide_by_vanishing_coset_cached` and a
read-only variant, `divide_by_vanishing_coset_ro`, used for the
already-warmed cache inside the parallel booleanity loop). Measured effect
alone was modest as predicted (~10%, e.g. Layer 4b build 419.9ms → ~380ms) —
the FFT/interpolate computation itself dominates, not the coset/Z_H setup —
but it was risk-free and is folded into the combined numbers below.

### 3. Stop re-embedding already-sent commitments in the opening proof, medium effort, safe, moderate proof-size win, NOT DONE, NOT YET PROTOTYPED

This is a reasoning-only finding from this session, not yet implemented or
measured: of the 447 G1 points in the measured 42,464-byte proof, roughly
418 are literal duplicates of commitments (`wcomms`, `qcomms`, `bg_comms`)
that were already computed and (in a real Fiat-Shamir NIZK) already
transmitted to the verifier earlier in the protocol, before the challenge
was derived. The `gadgets` crate's `BatchCheckProof.commitments` field
re-embeds them purely so `batch_check` is self-contained and doesn't need
an external commitment lookup. A leaner wire-format proof would serialize
each commitment once and have the opening-proof messages reference it,
rather than duplicating ~13,376 bytes worth of G1 points that add no new
information. This would bring proof size from ~42,464 down to roughly
~29,000 bytes (worked out on paper, not measured), a real improvement,
but still short of Noir's 14,656, so this alone does not close the gap.
Implementing it means either (a) writing a custom serialization layer for
this codebase's own proof format that doesn't go through
`BatchCheckProof` as the wire format, or (b) forking/patching the gadgets
crate's `batch_check` to accept externally-supplied commitments instead of
bundling them. Either way, this changes serialization only, not any
cryptographic check, so it should be safe, but confirm with a fresh
`cargo test` and a check that `batch_check`'s actual verification logic
still runs against the same values either way.

### 4. Replace or aggregate the 390-polynomial bit-decomposition gadgets, high effort, biggest structural win, NOT DONE, NOT YET DESIGNED

This is the only item with a real shot at closing the proof-size and
verify-time gap outright rather than narrowing it. Two sub-options:

- **Full lookup argument** (Plookup [Gabizon and Williamson 2020] or
  similar), replacing the 6 bit-decomposition range/non-negativity checks.
  Biggest possible win, most work. No ready-made lookup gadget exists in
  the MadibaGroup `gadgets` crate this project depends on, so this would
  mean implementing the lookup argument from scratch on top of the existing
  KZG10 setup, a real cryptographic engineering project, not a tuning pass.
- **Aggregate the existing bit-decomposition approach**: instead of
  committing and opening 32 separate bit-column polynomials per gadget
  instance, combine them via a random linear combination (a batching
  technique, not a new argument system) before committing, cutting the
  390-polynomial count down significantly while keeping the same
  bit-decomposition idea. Less work than full Plookup, partial win, but
  needs care: `fiat_shamir_prove`'s existing residue-folding logic (Phase 2,
  the `r[15..212]` batched sum-check) and the Phase 3 opening logic both
  currently assume one polynomial per bit column and would need to be
  reworked to match whatever new aggregated structure is chosen.

Whichever sub-option is attempted, add a negative-control test in the same
style as `bit_gadget_opening_rejects_swapped_commitment` before considering
it done, tamper with something and confirm the new scheme's `batch_check`
(or equivalent) actually rejects it. Soundness verification standard set by
Phase 3 should carry forward, not be assumed.

### 5. Re-benchmark Noir and Rust at N=5,000-20,000, measurement only, NOT DONE

The MSM-threading gap (Rust single-threaded, Barretenberg multi-core, same
Pippenger `O(m/log m)` complexity class either way [Pippenger 1976]) is the
only remaining mechanism that could still produce a widening prove-time
crossover at larger N in Noir's favor. CSVs already exist for this. Doing
item 1 first (parallel MSM) before re-benchmarking would make this a fairer
comparison, otherwise you'd just be re-confirming a known, already-explained
gap.

## Update 2026-08-24: parallelization pass — Rust retakes the lead

Goal for this pass: close the prove-time gap that had shrunk to ~2x at
N=1000 and disappeared at N=100, without touching soundness, the protocol,
or proof size/wire format (those stayed out of scope — see items 3/4 above,
still not done). Three changes, all additive to `lib.rs`, none touching the
vendored `gadgets` crate or `batch_check` (the verifier side is byte-for-byte
unchanged):

1. **`CosetCache`** (item 2 above) — memoizes coset-FFT domain/`Z_H` setup
   across the 9 calls in `compute_quotients` and the 32-per-instance calls
   inside each of the 6 `build_bit_gadget` instances.
2. **`parallel_batch_open`** — a rayon-parallelized reimplementation of
   `gadgets::utils::batch_open`'s math (same witness-polynomial construction,
   same `gamma`-weighted folding, same `OpenEval` output), used only at the
   Group-2 call site in `compute_all_opening_proofs` (the 390-polynomial bit-
   gadget opening). Groups 0/1 and the two cliff-value openings still call
   the original sequential `batch_open` unchanged, since they're cheap
   (26 polynomials total) and not worth the risk of touching more than
   necessary.
3. **`parallel_commit_many`** — parallelizes the two MSMs (value + blinding)
   per polynomial in `commit_bit_gadget`'s bit-column and boolean-flag loops
   (32 items each, 6 instances). Randomness is drawn sequentially from the
   single RNG *before* the parallel section (an `R: RngCore` can't safely be
   shared/mutated across threads), only the MSMs themselves run in parallel.

Correctness/soundness verification: all 4 unit tests pass after every edit,
including `bit_gadget_opening_rejects_swapped_commitment` (tampers with a
bit-gadget commitment, confirms `batch_check` — untouched — still rejects
it). Since the verifier-side code has zero changes, any bug in the parallel
prover-side reimplementations would show up as `batch_check` failing on
honest proofs, not as silently-accepted bad proofs; the test suite covers
the honest-proof path (does it still verify) and the negative-control
covers the soundness path (is a bad proof still rejected).

### New measured numbers (fresh `cargo run --release`, per-layer `Instant` trace)

Core proof time uses the same definition as `OPTIMIZED_RATIONALE_AND_RESULTS.md`
("Core cryptographic proof time" section): sum of Layers 2, 3a, 3b, 3c, 3d,
3e, 3f-prove, 3g-prove, 3h-prove. Excludes Layer 4 (redundant sanity
duplicate) and Layer 4b's build/commit (real work, but excluded from this
number the same way it always was in this doc, even though it's now
genuinely pairing-opened via Layer 3f rather than just committed).

**Numbers below were re-measured as a mean over 5 independent `cargo run
--release` executions (not a single trace) after this update was first
written** — the first draft reported single-run point estimates with a "~"
marker, which is an honest flag but not a rigorous number. Std devs are
small relative to the margins, so the conclusion doesn't change, but the
exact figures below are the corrected, averaged ones.

| | 21-tick | 100-tick | 1000-tick |
|---|---|---|---|
| Core proof, after Phase 3, before this pass | ~128-154 ms | ~137-161 ms | ~210-247 ms |
| Core proof, after this pass (mean of 5 runs, ±stddev) | 49.9 ± 2.7 ms | 59.8 ± 0.3 ms | 109.3 ± 2.7 ms |

| Metric | Noir, N=100 | Rust, N=100 (this pass) | Noir, N=1000 | Rust, N=1000 (this pass) |
|---|---|---|---|---|
| Prove time | 0.13 s (`bb prove`) | 59.8 ± 0.3 ms | 0.45 s (`bb prove`) | 109.3 ± 2.7 ms |
| Rust's margin | | **2.17x faster** | | **4.12x faster** |

Noir's own number (`bb prove`, 0.13s/0.45s) is a single reported figure from
its own doc, not a multi-run mean — the Noir side wasn't re-benchmarked as
part of this pass, only Rust was. So this table compares a 5-run Rust mean
against a 1-run Noir figure; disclosed here rather than left implicit, since
it's a real methodological asymmetry even though the margins (2.17x, 4.12x)
are comfortably larger than the observed run-to-run noise on either side.

This reverses the finding that motivated this pass: at N=100, Rust went from
"margin gone, roughly tied or slightly behind" to 2.17x faster; at N=1000,
from "~2x, down from an earlier unsound 4.25x claim" to 4.12x faster,
back above the original (unsound) 4.25x figure but this time on a fully
Phase-3-sound proof. None of items 3/4 (proof size, verify time) were
touched, so those two rows are unchanged from before this pass: proof size
still exactly 42,464 bytes on every run (~2.9x bigger than Noir's 14,656,
zero variance since it's a fixed element count not a timing) and
`batch_check` verify still ~22.4ms with under 0.1ms stddev across all 5 runs
at every size (up to ~2.8x slower than Noir's 10-20ms) — this pass was
scoped to prove-time/latency only, per the explicit ask, not the size/verify
gap.

N=1000 per-layer trace, mean of the same 5 runs (±stddev), for reference:

```
Layer 2  Interpolation (9 IFFT)           5.34ms (±0.37)
Layer 3a KZG setup                        3.69ms (±0.15)
Layer 3b Commit witnesses (9 MSM)        11.86ms (±0.94)
Layer 3c Quotient polynomials            25.40ms (±1.23)
Layer 3d Commit quotients (14 MSM)       15.55ms (±1.88)
Layer 3e Fiat-Shamir                     10.26ms (±0.05)
Layer 3f batch_open (prover)             22.81ms (±0.64)   <- was ~121ms before parallel_batch_open
Layer 3f batch_check (4 pairings)        22.44ms (±0.07)   <- unchanged, verifier untouched
Layer 3g V_max range::prove               4.89ms (±0.24)
Layer 3h cliff slack range::prove         9.47ms (±0.26)
Layer 4b Bit-decomp gadgets (build)     282.31ms (±1.26)  <- was ~420ms before CosetCache + parallel loop
Layer 4b Bit-decomp gadgets (commit)    114.44ms (±0.49)  <- was ~243ms before parallel_commit_many
```

(No single "TOTAL" line above since this is now a mean of 5 separate runs,
not one trace — one representative single run's TOTAL was 542.3ms, was
918.4ms before this pass, consistent with the per-layer means above.)

Layer 3f's prover-side cost (the single biggest lever, matching the
`batch_open` bottleneck identified in problem-solving) dropped roughly
5-10x. Layer 4b's commit step dropped roughly 8-13x. Layer 4b's build step
only dropped ~1.4-1.5x — smaller than hoped, likely because the 32-way
parallelism per gadget instance doesn't fully saturate a many-core machine,
or the FFT/interpolate step itself (not the coset setup) is the remaining
cost there. That's the most promising remaining lever if further latency
work is wanted: parallelize *across* the 6 gadget instances too, not just
within each instance's 32-item booleanity loop, and/or parallelize the
per-instance quotient/interpolation FFTs themselves.

Not done in this pass, still open: N=5,000-20,000 re-benchmarking (item 5),
proof-size reduction (item 3), bit-gadget aggregation/Plookup (item 4) — all
explicitly out of scope since the ask was latency/time specifically, not
size or a protocol redesign.

## What NOT to re-derive

Do not re-estimate proof size by counting elements on paper again, that
was tried twice earlier in this project and was wrong both times (missed
that `OpenEval::Plain` carries 2 field elements not 1, and missed the range
proof groups entirely). The `main.rs` block that prints `PROOF SIZE` walks
the actual `BatchCheckProof` structs and is the source of truth; if the
proof structure changes (items 3 or 4 above), update that counting block
and re-run it rather than computing a new estimate by hand.
