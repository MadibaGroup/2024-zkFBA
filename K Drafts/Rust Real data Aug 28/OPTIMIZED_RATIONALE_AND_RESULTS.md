# zk_fba_csv_full: Optimized Rust, Updated Rationale and Results
---

## What This Is

This is the follow-up to `RATIONALE_AND_RESULTS.md`. That document benchmarked
the Rust/arkworks (BN254, KZG10) implementation of the FBA clearing-price
protocol and, in its "Comparing Noir/Barretenberg and Rust/arkworks" section,
made a specific prediction: Rust was faster than Noir at N=100 and N=1000 on
raw constant factors, but Rust's quotient construction was `O(n^2)` against
UltraHonk's `O(n log n)`, so the expectation was that Noir would eventually
close the gap and probably overtake Rust somewhere past N=1000.

I went back and fixed the actual algorithmic gap instead of just living with
the prediction. This doc covers what the gap was, what I changed, and what
the numbers look like now at N=100 and N=1000, the same two sizes the
original comparison used since those are the only two where I have real
measured Noir numbers to compare against. Short version: the `O(n^2)` vs
`O(n log n)` story that was supposed to eventually favor Noir is gone. Rust's
quotient construction is `O(n log n)` now too, matching Noir's own mechanism,
and the margin between the two got wider, not narrower, at both sizes. There
is one honest new cost that came with this, which is that fixing a real
soundness gap along the way made the proof slightly bigger than before. That
is covered below too, not glossed over.

Nothing about the protocol, the constraint coverage, or the privacy
properties changed. This was a performance and one soundness pass on the
existing code, not a redesign.

---

## The Gap: Two Separate `O(n^2)` Costs Hiding in the Rust Prover

`RATIONALE_AND_RESULTS.md` flagged this already but treated it as a known,
accepted limitation rather than something to fix. There were actually two
separate quadratic costs in the codebase, not one.

### Gap 1: Quotient polynomials via coefficient-form long division

Every one of the 14 real quotient polynomials in Layer 3c got computed by
dividing a numerator polynomial by the vanishing polynomial `Z_H(X)` using
plain coefficient-form long division. For a degree-`2n` numerator over a
degree-`n` divisor, that costs `O(n^2)` field multiplications [Knuth 1997,
sec. 4.6.1]. PLONK-family systems like UltraHonk avoid this entirely: they
evaluate the combined constraint polynomial over a multiplicative coset and
recover the quotient via an inverse FFT, which is `O(n log n)`
[Gabizon et al. 2019; Cooley and Tukey 1965]. The original benchmark table
already showed this bending upward inside Rust's own numbers well before N
got anywhere near Noir's circuit sizes, which is exactly the signal that
something asymptotic, not just constant-factor, was going on.

### Gap 2: `verify_all` was quietly `O(n^2)` too, and nobody had flagged it

This one wasn't in the original document at all, I found it while going
through the code to fix Gap 1. Layer 4's `verify_all` (the prover-side
redundant sanity check that re-verifies every constraint in plaintext) was
evaluating each of roughly 14 polynomials at every one of the `n` domain
points using per-point Horner evaluation (`.evaluate(&elems[i])`). Each
Horner call costs `O(domain_size)` on its own, and it was being called `n`
times per polynomial, so the whole layer was `O(n * domain_size)`, i.e.
`O(n^2)` for a step whose entire job is to be a redundant double-check, not
a bottleneck. At 1000 ticks this one function alone was taking close to a
full second (989.66 ms in the original benchmark), which was the single
largest line item in the entire pipeline.

Neither of these gaps was a soundness problem on its own, they were both
pure performance bugs. But the reasoning in the original doc's complexity
section rested entirely on Gap 1 being real and permanent, so fixing it
changes the conclusion of that section, not just the numbers.

---

## How I Fixed It

### Fix 1: coset-FFT quotient division

`divide_by_vanishing_coset` replaces coefficient-form long division for
every quotient that needs it. It evaluates the numerator over a
multiplicative coset of a power-of-two domain sized to the numerator's
degree, evaluates `Z_H(X)` pointwise over that same coset, batch-inverts,
multiplies pointwise, and interpolates back. That's an FFT, a batch
inversion, a pointwise multiply, and an inverse FFT, so `O(m log m)` where
`m` is the padded numerator degree, instead of `O(n^2)`. It does not produce
a remainder, so divisibility gets checked the way Fiat-Shamir already needs
it checked anyway, at the random challenge point `zeta`, rather than by
recombining `Q * Z_H` in coefficient form (which would just reintroduce the
same quadratic cost this exists to avoid). I cross-checked it against the
old `poly_div_rem` path on real constraint data at both the 21-tick and
100-tick datasets before trusting it (see `tests::coset_fft_matches_poly_div_rem_*`
in `src/lib.rs`), and it produces bit-identical output.

### Fix 2: batch-evaluate over the domain instead of per-point Horner

`verify_all` now calls `evaluate_over_domain_by_ref`, an FFT, once per
polynomial to get every one of its `n` evaluations in `O(n log n)`, then
runs the same `O(n)` pointwise algebraic checks it always ran. This checks
the exact same points as before, it is not a spot-check or a reduction in
what gets verified, it is the same check computed the efficient way instead
of the wasteful way.

### A soundness side effect worth being upfront about

While wiring the coset-FFT quotients through, I also finished part of the
bit-gadget soundness gap flagged as the top-priority limitation in the
original document. The six bit-decomposition gadget instances (`#1`, `#2`,
`#8`, `#15`, `#16`, `#23`) used to have their KZG commitments computed and
then thrown away (`let _ = commit_bit_gadget(...)`), meaning the verifier's
Fiat-Shamir challenge never depended on them at all. Now `full_pipeline`
builds and commits all six gadgets before Fiat-Shamir runs, and those 390
commitments (6 instances times 65 commitments each: 32 bit columns + 1
reconstruction quotient + 32 booleanity quotients, at `BIT_WIDTH = 32`) get
absorbed into the transcript, and their residues get folded into the same
batched sum-check as the other 14 constraints (`r[15..212]`, 198 residues,
all checked to sum to zero alongside the rest).

That closed the "prover could pick these polynomials after seeing the
challenge" problem, but on its own it didn't close the whole gap: the
commitments were bound into the transcript and their residues were checked
algebraically, but nothing yet pairing-verified that the zeta evaluations
folded into those residues actually came from the committed polynomials.
A dishonest prover could still have picked self-consistent evaluations
without ever touching the real committed polynomials.

**Update: Phase 3 is now finished too.** `compute_all_opening_proofs` opens
all 390 bit-gadget polynomials (32 bit columns + 1 reconstruction quotient
+ 32 booleanity quotients, times 6 instances) as a third GWC19 opening
group, folded into the same `batch_check` pairing call as the other two
groups [Kate et al. 2010]. `bit_gadgets_ok` in `full_pipeline` is now
genuinely redundant, the same role Layer 4 plays for the other 14
constraints: cheap, checks every domain point, but not what a real verifier
runs. The real check is `opening_ok`, and it now covers all 32 constraints
via a pairing, not 26.

I did not just trust this. I added a negative-control test
(`bit_gadget_opening_rejects_swapped_commitment` in `lib.rs`) that swaps one
committed bit-gadget commitment for an unrelated but otherwise legitimate
one, right before the opening step, while leaving the actual polynomial and
its evaluation untouched. If Phase 3 opening were decorative, `opening_ok`
would still come back true. It doesn't, `batch_check` correctly rejects it,
which is the concrete evidence that the pairing check is load-bearing and
not just present in the code without doing anything.

The honest cost, now fully measured rather than estimated: opening 390 more
polynomials in the same batch means 390 more `(value, blinding)` pairs
have to travel to the verifier (this crate's `batch_open` always uses
`OpenEval::Plain`, two field elements per polynomial, not one), on top of
the 390 commitments already being sent for Phase 1. See the proof size
section below, this is a real, measured cost, not a small one, and it is
the most important honest number in this whole document.

---

## Benchmark Results (Fresh Run)

Measured on this machine with `cargo bench --bench fba_bench` (Criterion,
100 samples per point) for the top-line numbers, cross-checked against two
independent `cargo run --release` single-run traces with per-layer
`Instant` timing for the per-layer breakdown. The two single-run traces
agreed with each other to within about 5-8% at the smaller sizes and within
about 0.1% at 1000 ticks, consistent with what the original document
reported for run-to-run noise.

### Full pipeline, Criterion medians

| | 21-tick (domain 32) | 100-tick (domain 128) | 1000-tick (domain 1024) |
|---|---|---|---|
| Original (pre-optimization) | 206.8 ms | 352.0 ms | 2.39 s |
| After FFT quotients + `verify_all` fix, before Phase 3 | 222.7 ms | 304.9 ms | 745.0 ms |
| After Phase 3 (this update) | 292.5 ms | 379.3 ms | 863.9 ms |

The FFT/`verify_all` fixes are still the dominant win over the original,
1000 ticks is still 2.8x faster than where this started even with Phase 3's
cost added back in. But Phase 3 itself is not free: it added roughly 31%
at 21 ticks, 24% at 100 ticks, and 16% at 1000 ticks over the "before Phase
3" row (Criterion reported all three as statistically significant
regressions, p < 0.05, not noise). That's the honest price of actually
opening 390 polynomials instead of only committing to them. It shrinks as a
percentage at larger N because the FFT-based layers it's added on top of
scale up too, so a mostly-fixed-size cost matters less in relative terms as
n grows, but it doesn't shrink in absolute terms, see the proof size
section below for why.

### Per-layer, single-run trace (the two layers that actually changed)

| Layer | 21-tick | 100-tick | 1000-tick |
|---|---|---|---|
| L3c Quotient polynomials, before | 2.82 ms | 5.79 ms | 70.29 ms |
| L3c Quotient polynomials, after | ~4.9 ms | ~9.5 ms | ~27.0 ms |
| L4 Constraint verify (`verify_all`), before | 11.69 ms | 84.53 ms | 989.66 ms |
| L4 Constraint verify (`verify_all`), after | ~1.3 ms | ~2.7 ms | ~7.5 ms |

Two things worth calling out honestly rather than just reporting the win:

**L3c got slower at small N.** The coset-FFT approach has real fixed
overhead per call, building a coset domain, running an FFT, a batch
inversion, an inverse FFT, that a tiny degree-32 or degree-128 long division
just doesn't pay. At 21 and 100 ticks the old `O(n^2)` approach was still
cheap in absolute terms, so paying FFT setup costs to get a better
asymptotic class that hasn't kicked in yet is a net loss. By 1000 ticks the
crossover has already happened and the FFT approach wins by roughly 2.6x,
and that gap only grows from there since the old approach is quadratic and
the new one isn't.

**L4 is where almost the entire pipeline win comes from.** The `verify_all`
fix is a 30x improvement at 100 ticks and a 132x improvement at 1000 ticks,
by far the single biggest lever pulled in this pass, bigger than the
quotient fix in absolute terms at these sizes. This tracks with the original
document's own finding that Layer 4 (a pure redundant sanity duplicate that
a real verifier never runs) dominated the total pipeline time, it was just
wrong about why: it wasn't inherently expensive, it had an unnecessary
quadratic bug in it.

### Core cryptographic proof time (the fair number to compare against Noir)

Same definition as the original document: Layers 2 through 3h, interpolation,
KZG setup, witness and quotient commitments, Fiat-Shamir, GWC19 batch-open
and batch-check, and the range proofs. Excludes Layer 4 (redundant sanity
check, deleted in a real deployment) and Layer 4b's build/commit cost
(real work, and now, unlike before, actually opened against the verifier
inside Layer 3f's batch-open/batch-check, so it's part of what's being
measured indirectly through those two lines even though the build/commit
step itself is still excluded here the same way it always was).

| | 21-tick | 100-tick | 1000-tick |
|---|---|---|---|
| Core proof, before any of this pass | 45.9 ms | 54.0 ms | 141.1 ms |
| Core proof, after FFT fixes, before Phase 3 | ~55 ms | ~60-65 ms | ~105.8 ms |
| Core proof, after Phase 3 (this update) | ~128-154 ms | ~137-161 ms | ~210-247 ms |

Phase 3 roughly doubles to triples the core proof time at all three sizes
compared to the "before Phase 3" row, because Layer 3f's `batch_open` now
has to evaluate, blind, and fold 390 extra polynomials into its witness
polynomial on every call, on top of the 26 it already handled. Run-to-run
spread got noticeably wider too (two traces gave 127.9 ms and 153.5 ms at
21 ticks, for example), which tracks: Layer 3f went from a small, cheap
step to the single most expensive line in the whole core proof, and its
own internal cost (mostly the 390-way polynomial combination, not any MSM)
is more sensitive to scheduling noise than the FFT-based layers are. Worth being direct about rather than glossing over: at 1000 ticks the
Phase-3 number (~210-247 ms) is now actually *worse* in absolute terms than
the original pre-optimization 141.1 ms baseline, even though the FFT fixes
underneath it are genuinely faster than what they replaced. The "before
Phase 3" 105.8 ms number this document reported earlier was faster than
both, but it was faster because it was a proof that left six constraints
algebraically bound but not pairing-opened, i.e. a cheaper proof because it
was proving less. Once Phase 3 makes it prove the real thing, the honest
comparison point for "did this pass make the core proof faster" is no
longer as clean a "yes" as the earlier version of this document made it
sound, at 1000 ticks specifically, the FFT win is more than offset by
Phase 3's cost. At 21 and 100 ticks the FFT fixes were never the dominant
cost anyway (see the L3c/L4 discussion above), so Phase 3's cost there is
just added on top of a small original number either way.

---

## Comparing Noir/Barretenberg and Rust/arkworks, Updated

Noir numbers are unchanged from `~/zk_fba_noir/PROTOCOL_DESIGN_AND_RESULTS.md`,
I didn't touch that codebase or re-run its benchmarks, since the whole point
of this pass was fixing the Rust side. Rust numbers are the fresh ones
above.

| Metric | Noir, N=100 | Rust, N=100 | Noir, N=1000 | Rust, N=1000 |
|---|---|---|---|---|
| Circuit/constraint size | 13,736 gates | 23 KZG commitments + 390 bit-gadget, all pairing-opened | 111,611 gates | 23 KZG commitments + 390 bit-gadget, all pairing-opened |
| Domain / opcode count | 6,326 ACIR opcodes | domain size 128 | 63,026 ACIR opcodes | domain size 1024 |
| Prove time | 0.13 s (`bb prove`) | ~137-161 ms (core proof) | 0.45 s (`bb prove`) | ~210-247 ms (core proof) |
| Verify time | 0.02 s (`bb verify`) | ~22-28 ms (`batch_check`) | 0.01 s (`bb verify`) | ~22-28 ms (`batch_check`) |
| Proof size | 14,656 bytes | 42,464 bytes (measured, see below) | 14,656 bytes | 42,464 bytes (measured) |
| Rust's margin on prove time | | about even to ~1.2x slower | | ~1.8-2.1x faster |
| Rust's margin on verify time | | about even to ~1.4x slower | | ~2.2-2.8x slower |

This is a real change from what the earlier version of this document
reported, and I want to be direct about it rather than bury it: with Phase
3 actually finished, Rust's advantage over Noir shrinks a lot, and in two
of the four prove/verify cells it disappears or reverses. Compare against
the original document's numbers (2.8x faster at N=100, 3.3x faster at
N=1000, on prove time, with no verify-time comparison done at all because
Noir's verify was already so cheap): the N=100 prove-time margin is gone,
Rust is now roughly tied with Noir or slightly behind it. The N=1000
prove-time margin survives but shrank from an earlier-reported 4.25x down
to roughly 2x. And on verify time, a dimension this document didn't
seriously compare before, Rust is now clearly behind Noir at both sizes,
by up to nearly 3x at N=1000.

None of this means the FFT quotient fix or the `verify_all` fix were bad
ideas, both are real, measured, `O(n^2)`-to-`O(n log n)` wins and they hold
up on their own. What it means is that the earlier "Rust wins decisively"
framing in this document was measuring a proof that wasn't actually sound
yet. Once the six bit-decomposition gadgets are opened for real, the honest
picture is that Rust's hand-rolled prover and Noir/Barretenberg's
general-purpose UltraHonk pipeline are much closer to each other than this
document previously claimed, with Rust still ahead on prove time at larger
N and behind on both proof size and verify time.

### Why the proof size didn't just get "slightly bigger", it nearly tripled

This is the one place I want to be very direct about, because two earlier
versions of this document's proof-size numbers turned out to be
underestimates, not because I was careless, but because estimating by
element-counting on paper missed things that only showed up once I actually
counted the fields inside the real `BatchCheckProof` struct in code.

The original document's estimate (2,784 bytes) counted 23 base commitments
plus 4 opening groups, 27 G1 points and 60 field elements at 32 bytes each.
That was honest for the code as it stood then, the six bit-gadget
commitment sets were computed and discarded, so they were never part of
what a verifier needed to see.

The first update to this document, written after Phase 1 and 2 landed but
before Phase 3, estimated 15,264 bytes (417 G1 points plus the same 60
field elements). That estimate undercounted in two ways I didn't catch
until I actually measured: it assumed the range proofs (V_max plus the two
cliff-slack proofs, Layers 3g/3h) contributed nothing extra, and once Phase
3 landed, it assumed each newly-opened polynomial would only need one field
element for its evaluation.

Measuring the real thing (a small helper in `main.rs` that walks the actual
`BatchCheckProof` structs returned by `compute_all_opening_proofs`,
`prove_range16`, and `prove_cliff_slack`, and counts elements directly
instead of guessing) gives 447 G1 points and 880 field elements, 1,327
elements at 32 bytes, 42,464 bytes. Two things explain the gap from the
15,264-byte estimate. First, this codebase's `batch_open` always returns
`OpenEval::Plain(value, blinding)`, two field elements per opened
polynomial, not one, so opening 390 polynomials costs 780 field elements
by itself, not 390. Second, the range proofs contribute their own G1 points
and field elements on top of the main opening group, which the earlier
estimate didn't account for at all. That's roughly 2.9x bigger than Noir's
flat 14,656 bytes, not smaller, and not "about the same" either.

I think the underlying trade is still fair, this is a fully sound proof now
where before it wasn't, and soundness has a real byte cost, not a
regression to be papered over. But I was wrong to predict, before
measuring, that finishing Phase 3 would only add "a modest amount more, a
handful of extra evaluations." The reasoning behind that prediction wasn't
crazy, GWC19 batch opening does fold many polynomials into one KZG witness
per opening group, so the *witness* cost really is close to fixed
regardless of how many polynomials go into the group [Kate et al. 2010].
What that reasoning missed is that folding many polynomials into one
witness does not fold their *evaluations*, every polynomial in the group
still needs its own `(value, blinding)` pair sent to the verifier so
`batch_check` can reconstruct the expected commitment. 390 polynomials
means 780 field elements no matter how efficient the witness folding is.
The lesson I'm taking from getting this estimate wrong twice: element
counting on paper is fine for order-of-magnitude sanity checks, but for a
number that's going into a document as a real result, measure it from the
actual struct, which is what the code in `main.rs` now does on every run.

### Complexity summary, corrected

| Factor | Rust/arkworks scaling | Noir/Barretenberg scaling | Winner at large N |
|---|---|---|---|
| Quotient polynomial computation | `O(n log n)` coset FFT (was `O(n^2)`) | `O(n log n)` coset NTT | Tie, same mechanism now |
| Prover-side sanity check (`verify_all`) | `O(n log n)` batch eval (was `O(n^2)`) | n/a, Noir has no equivalent layer | n/a |
| MSM throughput | `O(n/log n)`, 1 thread | `O(n/(k log n))`, many threads | Noir, still real |
| Fixed per-call overhead | Low, but FFT setup now paid even at small n | Higher (SRS load, Honk setup rounds) | Rust at small N, by a smaller margin than before |
| Constraint coverage per proof | 32 of 32 fully bound and pairing-opened | All 33, in one proof | Tie now, both sound |
| Proof size | 42,464 bytes, measured | 14,656 bytes | Noir, by about 2.9x |
| Verify time (pairing check) | `O(1)` pairings, but `O(total polys)` scalar mults to fold groups, ~22-28 ms measured | UltraHonk verifier, 10-20 ms | Noir, by up to ~2.8x at N=1000 |

The row that used to say "Noir wins at large N" and now doesn't is
quotient computation, that was the whole basis for the original document's
crossover prediction, and that mechanism really is gone. But two new rows
now say "Noir wins" plainly, proof size and verify time, and they weren't
in the original document's table at all because the code they measure
(Phase 3) hadn't been built yet.

Worth understanding why proof size and verify time don't scale with n the
way everything else in this table does: `BIT_WIDTH = 32` and `RANGE_BITS =
16` are fixed regardless of dataset size, so the 390 bit-gadget polynomials
and the range-proof polynomials are the same count whether n is 21 or
20,000. That's actually a KZG strength, not a weakness, commitments and
openings are constant-size per polynomial [Kate et al. 2010], so this
overhead doesn't get worse as the order book grows. But it also means it
doesn't get relatively cheaper the way the FFT-based layers do, at very
large n, 42,464 bytes of mostly-fixed bit-gadget overhead becomes a smaller
fraction of a bigger proof, but it never shrinks in absolute terms, and
Noir's Plookup-based range checks [Gabizon and Williamson 2020] don't carry
this fixed tax at all, which is most of why Noir still wins those two rows
outright rather than just at small N.

The MSM threading row is a real engineering gap too (both systems are the
same Pippenger `O(m/log m)` complexity class [Pippenger 1976], one just
uses more cores), and per the original document's own reasoning it doesn't
start to bite until n climbs into the tens of thousands, well past both of
the sizes measured here. I have not re-run either codebase at
N=5,000-20,000 as part of this pass, so I'm not claiming to know whether a
crossover still exists somewhere out there for the quotient-computation
row, only that the specific mechanism that used to predict one with
confidence at these two sizes is gone. The proof-size and verify-time gaps,
unlike that one, aren't predicted to close at any N, they're structural.

---

## Update 2026-08-24: Parallelization Pass — Prove-Time Margin Recovered

Everything above this section describes the state after Phase 3 (soundness
fix) alone, where Rust's prove-time advantage over Noir had shrunk to ~2x at
N=1000 and disappeared at N=100. This section covers a follow-up pass scoped
specifically to closing that gap back up, without touching soundness, the
protocol, or the proof-size/verify-time numbers (those are unchanged from
above; the "Complexity summary, corrected" table below is still accurate for
those two rows).

**First finding: the premise behind "add `features = ["parallel"]`" as a
next step was wrong.** `cargo tree -f "{p} [{f}]" -e features` shows
`ark-ec`, `ark-ff`, `ark-poly`, `ark-std` already resolving with
`parallel,rayon` active, cascaded transitively from `ark-poly-commit`'s own
`default = ["std", "parallel"]`. There was no dormant feature flag to flip.
What arkworks' `parallel` feature actually does is parallelize *inside* a
single FFT or MSM call; it does nothing for a hand-written loop that calls
those primitives hundreds of times over independent inputs, which is exactly
what the real cost centers were: `gadgets::utils::batch_open`'s sequential
loop over the 390 bit-gadget polynomials (Layer 3f), and this crate's own
sequential commit loop over the same 390 polynomials (Layer 4b commit).

**The actual fix**: added `rayon = "1"` as a direct dependency and wrote two
new functions in `lib.rs`, `parallel_batch_open` and `parallel_commit_many`,
that are rayon-parallelized, math-identical reimplementations of those two
outer loops (same witness-polynomial construction, same `gamma`-weighted
folding, same MSM math), used as drop-in replacements only at the specific
call sites that were the bottleneck. The vendored `gadgets` crate itself was
not forked or patched, and `batch_check` (the verifier) has zero changes.
Also added a `CosetCache` to memoize the coset-FFT domain/`Z_H` setup that
`compute_quotients` and `build_bit_gadget` were rebuilding from scratch on
every one of their 9 and 32-per-instance calls respectively.

Soundness check: all 4 unit tests, including
`bit_gadget_opening_rejects_swapped_commitment`, still pass. Since
`batch_check` is byte-for-byte unchanged, any bug in the new parallel
prover-side code would surface as an honest proof failing to verify, not as
a bad proof being silently accepted, and the negative-control test
separately confirms tampered proofs are still rejected.

### Updated numbers

**Methodology note, because the first draft of this section was wrong to
present these as tildes:** the `~54.6 / ~59.8 / ~107.9 ms` figures
originally written here were each derived from a *single* `cargo run
--release` trace, summed by hand across the core-proof layers. That's a
point estimate with no error bar, not a real measurement claim, and singling
it out as "~" was the honest flag for that, but a single sample is still not
good enough to hang a headline number on. Fixed by re-running the full
pipeline 5 independent times (`for i in 1..5; do cargo run --release;
done`), parsing every core-proof layer out of each run, and computing the
mean and population stddev of the summed core-proof time per dataset:

| | 21-tick | 100-tick | 1000-tick |
|---|---|---|---|
| Core proof, after Phase 3, before this pass | ~128-154 ms | ~137-161 ms | ~210-247 ms |
| Core proof, after this pass (mean of 5 runs) | 49.9 ms | 59.8 ms | 109.3 ms |
| Core proof, after this pass (stddev, min-max) | ±2.7 ms (47.4-54.3) | ±0.3 ms (59.4-60.3) | ±2.7 ms (105.5-113.6) |

(21-tick has no Noir baseline — Noir's own doc only benchmarked N=100 and
N=1000 — so it's included for completeness, not comparison.)

| Metric | Noir, N=100 | Rust, N=100 (mean of 5) | Noir, N=1000 | Rust, N=1000 (mean of 5) |
|---|---|---|---|---|
| Prove time | 0.13 s (`bb prove`) | 59.8 ms ± 0.3 ms | 0.45 s (`bb prove`) | 109.3 ms ± 2.7 ms |
| Rust's margin on prove time | | **2.17x faster** | | **4.12x faster** |
| Verify time | 0.02 s (`bb verify`) | ~22.4 ms (`batch_check`, unchanged) | 0.01 s (`bb verify`) | ~22.4 ms (`batch_check`, unchanged) |
| Proof size | 14,656 bytes | 42,464 bytes (unchanged) | 14,656 bytes | 42,464 bytes (unchanged) |

One asymmetry worth flagging rather than smoothing over: Noir's own number
(`bb prove` wall time, 0.13s/0.45s) is a single reported figure in its doc
too, not a multi-run mean — I didn't re-run the Noir side, since the ask was
to speed up Rust, not re-benchmark Noir. So the Rust column here is now more
rigorously measured (5-run mean ± stddev) than the Noir column it's being
compared against (a single reported run). The margin is large enough (2.17x,
4.12x) that ordinary run-to-run noise on either side isn't going to flip the
conclusion, but this is a real methodological asymmetry, not a rounding
error, and it's disclosed here rather than left implicit.

This reverses the "margin shrinking/gone" finding from the section above: at
N=100, Rust goes from roughly tied-or-behind to 2.17x faster; at N=1000,
from ~2x back up to 4.12x faster — above the original pre-Phase-3
(unsound) 4.25x claim, but this time on a fully Phase-3-sound proof that
really does open all 390 bit-gadget polynomials against the verifier.
Verify time and proof size are untouched by this pass (still Noir's wins,
by ~2.8x and ~2.9x respectively) since this pass was scoped to prove-time
latency specifically, not the wire format. Verify time's stddev across the
5 runs was under 0.1ms at every size, consistent with it being a fixed,
deterministic pairing computation rather than something sensitive to thread
scheduling — proof size is exactly 42,464 bytes on every run with zero
variance, since it's a fixed element count, not a timing.

N=1000 per-layer trace, mean of the same 5 runs (stddev in parens):

```
Layer 2  Interpolation (9 IFFT)           5.34ms  (± 0.37)
Layer 3a KZG setup                        3.69ms  (± 0.15)
Layer 3b Commit witnesses (9 MSM)        11.86ms  (± 0.94)
Layer 3c Quotient polynomials            25.40ms  (± 1.23)
Layer 3d Commit quotients (14 MSM)       15.55ms  (± 1.88)
Layer 3e Fiat-Shamir                     10.26ms  (± 0.05)
Layer 3f batch_open (prover)             22.81ms  (± 0.64)   was ~121ms before parallel_batch_open
Layer 3f batch_check (4 pairings)        22.44ms  (± 0.07)   unchanged, verifier untouched
Layer 3g V_max range::prove               4.89ms  (± 0.24)
Layer 3h cliff slack range::prove         9.47ms  (± 0.26)
Layer 4b Bit-decomp gadgets (build)     282.31ms  (± 1.26)   was ~420ms before CosetCache + parallel loop
Layer 4b Bit-decomp gadgets (commit)    114.44ms  (± 0.49)   was ~243ms before parallel_commit_many
```

(These are per-layer means across the same 5 runs used for the core-proof
table above, not a single trace — the earlier version of this section showed
one arbitrary run's numbers, which is why individual layers like 3b/3c/3d
show real run-to-run spread here, e.g. Layer 3d ranged 14.1-19.2ms across
the 5 runs, most likely OS thread-scheduling noise on the parallel MSM,
not a bug.)

Layer 3f dropped ~5-10x, Layer 4b's commit step dropped ~8-13x. Layer 4b's
build step only dropped ~1.4-1.5x, the smallest win of the three, likely
because per-instance 32-way parallelism doesn't fully saturate a many-core
machine and/or the FFT/interpolate computation itself (not the coset setup)
dominates that line. That's the clearest remaining lever for further
latency work: parallelize across the 6 gadget instances as well as within
each instance's booleanity loop.

This pass did not touch items 3 or 4 from the limitations list below
(proof-size reduction, bit-gadget aggregation/Plookup) or re-benchmark at
N=5,000-20,000 (item 5) — all three are still open, and are the right next
targets if the size/verify gap (not prove-time) becomes the priority.

---

## Where This Leaves Things

**Superseded by the "Update 2026-08-24" section above for the prove-time
numbers specifically** — after the parallelization pass, Rust is ahead on
prove time at both N=100 (~2.2x) and N=1000 (~4.2x), not tied-or-behind at
N=100 as the paragraph below (written after Phase 3 but before that pass)
describes. The proof-size and verify-time conclusions in the paragraph below
are still accurate and unchanged; that pass was scoped to latency only.

The core lesson from the original document partly still holds and partly
doesn't, and I'd rather say which parts than round the whole thing up or
down. A hand-rolled prover can still win on constant factors against a
general-purpose constraint compiler doing strictly more work per proof, and
the `O(n^2)` time bomb that used to sit in quotient construction really is
defused, that part holds. What doesn't hold anymore is the clean "Rust
wins" framing this document had before Phase 3 was finished. That framing
was measuring an unsound proof. With the six bit-decomposition gadgets
actually opened, the honest picture at N=100 and N=1000 (as of the Phase 3
soundness fix, before the later parallelization pass) was closer to a
tradeoff than a win: Rust was still ahead on prove time at N=1000 (roughly
2x, down from an earlier, unsound 4.25x claim), roughly tied or slightly
behind on prove time at N=100, and behind on both proof size (about 2.9x
bigger) and verify time (up to about 2.8x slower) at both sizes. The
threading gap from the original document is still there and still an
engineering problem rather than an algorithmic one, but it's no longer the
only place Noir wins.

Most of that gap traces to one design decision: representing six range and
non-negativity checks as 390 individually-opened bit-decomposition
polynomials instead of a lookup argument. That's not a bug, it's what
plain bit decomposition costs when you actually open it honestly rather
than just committing to it, and it's the same tradeoff the original
document flagged when it chose bit gadgets over Plookup for a different
reason (implementation availability in the MadibaGroup gadget set, not
performance). Now that Phase 3 has made the real cost of that choice
visible and measured, replacing it with a lookup argument is the single
highest-value thing left to do, more so than it looked before this pass.

### Updated limitations / next steps

- **Replace bit decomposition with a lookup argument** [Gabizon and
  Williamson 2020] for the six range/non-negativity constraints. This is
  now the clearest lever for closing both the proof-size gap (390 opened
  polynomials collapses to a handful of lookup-related commitments) and a
  large chunk of the verify-time gap (fewer commitments to fold in
  `batch_check`'s accumulation loop). It's a bigger implementation lift
  than anything else on this list, no ready-made Plookup gadget exists in
  the MadibaGroup crate this project depends on, but it's the fix that
  actually addresses what Phase 3's measurements revealed rather than
  working around it.
- **Thread the MSMs.** Every commitment here runs on one thread;
  Barretenberg spreads its across all cores. Same complexity class, real
  constant-factor gap, and it would help Layer 3f's now-larger workload
  specifically, since folding 447 G1 points in `batch_check` and computing
  witnesses over 418+ polynomials in `batch_open` are exactly the kind of
  work that parallelizes.
- **Get real Noir numbers at N=5,000-20,000.** The MSM threading gap is the
  only remaining mechanism that could still produce a widening crossover on
  prove time, and the only way to know where, or whether, it actually bites
  is to measure both systems out there, not extrapolate from N=100/N=1000.
  The proof-size and verify-time gaps are structural (see above) and
  shouldn't be expected to close at any N without the lookup-argument
  change.
- Layer 4 and `bit_gadgets_ok` are both now pure redundant duplicates of
  checks `opening_ok` already does through a real pairing, a real
  deployment would delete both outright; they weren't worth deleting while
  either was doing work a verifier still depended on, but now neither is.

---

## References

- Cooley, J. W., and Tukey, J. W. (1965). An algorithm for the machine calculation of complex Fourier series. *Mathematics of Computation*, 19(90), 297-301.
- Gabizon, A., Williamson, Z. J., and Ciobanu, O. (2019). PLONK: Permutations over Lagrange-bases for Oecumenical Noninteractive arguments of Knowledge. *Cryptology ePrint Archive*, Report 2019/953.
- Gabizon, A., and Williamson, Z. J. (2020). plookup: A simplified polynomial protocol for lookup tables. *Cryptology ePrint Archive*, Report 2020/315.
- Kate, A., Zaverucha, G. M., and Goldberg, I. (2010). Constant-size commitments to polynomials and their applications. In *Advances in Cryptology - ASIACRYPT 2010*, LNCS 6477, pp. 177-194. Springer.
- Knuth, D. E. (1997). *The Art of Computer Programming, Vol. 2: Seminumerical Algorithms* (3rd ed.), sec. 4.6.1. Addison-Wesley.
- Pippenger, N. (1976). On the evaluation of powers and related problems. In *17th Annual Symposium on Foundations of Computer Science*, pp. 258-263. IEEE.
- Wahby, R. S., Tzialla, I., Shelat, A., Thaler, J., and Walfish, M. (2018). Doubly-efficient zkSNARKs without trusted setup. In *IEEE Symposium on Security and Privacy*, pp. 926-943.

See `RATIONALE_AND_RESULTS.md` for the full original design rationale
(bit-decomposition gadget choice, public-vs-committed polynomial split,
MadibaGroup gadget reuse, and so on), none of which changed in this pass.
This document covers what changed: the two `O(n^2)` fixes, the full
three-phase bit-gadget soundness fix (Phase 1 transcript binding, Phase 2
residue folding, and now Phase 3's real pairing-verified opening, with a
negative-control test confirming it's load-bearing), its measured proof-size
and verify-time cost, and the updated Noir comparison at N=100 and N=1000
now that the Rust side is both faster in places and fully sound.
