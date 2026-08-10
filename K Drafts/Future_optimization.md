# Handoff: Optimizing the Rust ZK-FBA Prover Further

Written to let a fresh conversation continue this work without re-reading the
whole prior session. Everything below is verified against the actual code as
of this writing (all tests pass, `cargo run --release` gives `ALL PASS: YES`
on 21/100/1000-tick datasets, Phase 3 bit-gadget soundness fix is complete).

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
- `~/zk_fba_csv_full/Cargo.toml`, no `features = [...]` set on any
  arkworks dependency, meaning the `parallel` (rayon) feature is off. This
  is item 1 below.
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

### 1. Enable arkworks' `parallel` feature, low effort, safe, real win, NOT DONE

`ark-ec 0.4.2`, `ark-poly 0.4.2`, `ark-ff 0.4.2`, `ark-std 0.4.0` all have a
`parallel` feature (rayon-backed) confirmed present in their `Cargo.toml`s
but not enabled here. It gates MSM (`ark-ec::scalar_mul::variable_base`,
used in layers 3b/3d and inside `batch_open`'s witness computation) and FFT
(`ark-poly::domain::radix2::fft`, used in layers 2/3c and inside
`divide_by_vanishing_coset`). Add `features = ["parallel"]` to each of
those four dependencies in `Cargo.toml`, rebuild, re-benchmark. This should
help prove time broadly, and specifically should help the newly-expensive
Layer 3f, since folding 390 independent polynomials' witnesses is
embarrassingly parallel. Verify nothing breaks (it shouldn't, this doesn't
change any algorithm, just adds threading), then re-run the full benchmark
suite and update the numbers in `OPTIMIZED_RATIONALE_AND_RESULTS.md`.

### 2. Memoize coset-FFT setup inside `compute_quotients`, low effort, safe, modest win, NOT DONE

`compute_quotients` calls `divide_by_vanishing_coset` 9 times per run, and
each call independently rebuilds the coset domain and the `Z_H` evaluations
over that coset from scratch. Building these once per `compute_quotients`
call and passing them in would save repeated setup work. Small win relative
to item 1, but essentially risk-free.

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

## What NOT to re-derive

Do not re-estimate proof size by counting elements on paper again, that
was tried twice earlier in this project and was wrong both times (missed
that `OpenEval::Plain` carries 2 field elements not 1, and missed the range
proof groups entirely). The `main.rs` block that prints `PROOF SIZE` walks
the actual `BatchCheckProof` structs and is the source of truth; if the
proof structure changes (items 3 or 4 above), update that counting block
and re-run it rather than computing a new estimate by hand.
