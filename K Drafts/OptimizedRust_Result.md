# zk_fba_csv_full: Optimized Rust, Updated Rationale and Results
---

Rust was faster than Noir at N=21 on
raw constant factors, but Rust's quotient construction was `O(n^2)` against
UltraHonk's `O(n log n)`, so the expectation was that Noir would eventually
close the gap and probably overtake Rust somewhere past N=1000.

There was this actual algorithmic gap and now we see what
the numbers look like now at N=100 and N=1000, the same two sizes the
original comparison used. Short version: the `O(n^2)` vs
`O(n log n)` story that was supposed to eventually favor Noir is gone. Rust's
quotient construction is `O(n log n)` now too, matching Noir's own mechanism,
and the margin between the two got wider, not narrower, at both sizes. There
is one honest new cost that came with this, which is that fixing a real
soundness gap along the way made the proof slightly bigger than before. That
is covered below too, not glossed over.


---

## The Gap: Two Separate `O(n^2)` Costs Hiding in the Rust Prover

We flagged this already but treated it as a known,
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

### Gap 2: `verify_all` was quietly `O(n^2)` too, and was not flagged

This one wasn't in the original document at all, we found it while going
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

## How we Fixed It

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

That closes the "prover could pick these polynomials after seeing the
challenge" problem. It does not yet close the whole gap: the actual
pairing-based opening of these six gadgets (Phase 3, in the code's own
terms) still hasn't landed, `bit_gadgets_ok` in `full_pipeline` is still a
plaintext `recon_ok`/`bool_ok` self-check, not a `batch_check`-verified
result. So this is real progress, not a full fix, and I'm not going to
round it up to "fixed" in this document.

The honest cost of even the partial fix: those 390 commitments now have to
be part of what gets sent to the verifier, since the verifier needs them to
recompute the same Fiat-Shamir challenge the prover used. That makes the
proof bigger than the original document's estimate. See the proof size
section below, this is the one place where the optimization pass made a
number look worse instead of better, and it's worth sitting with rather
than skipping past.

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
| Before this pass | 206.8 ms | 352.0 ms | 2.39 s |
| After this pass | 222.7 ms | 304.9 ms | 745.0 ms |

21 ticks got very slightly slower (noise, plus the extra bit-gadget
commit/absorb work now happening unconditionally), 100 ticks improved
modestly, and 1000 ticks dropped by roughly 3.2x. That's the shape you'd
expect from an `O(n^2)` to `O(n log n)` fix: the win is invisible at tiny n
and dominant once n actually gets somewhere.

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
check, deleted in a real deployment) and Layer 4b's build/commit cost (real
work, but not yet opened against the verifier, see the soundness note
above).

| | 21-tick | 100-tick | 1000-tick |
|---|---|---|---|
| Core proof, before | 45.9 ms | 54.0 ms | 141.1 ms |
| Core proof, after | ~55 ms | ~60-65 ms | ~105.8 ms |

100 ticks looks slightly worse here for the same reason L3c looks worse at
small N above, the FFT overhead isn't paid off yet at this size and now
gets paid on every core-proof run instead of being buried inside a bigger
number. 1000 ticks improved by about 25%. The 1000-tick number was very
stable across repeated runs (105.8 ms and 105.7 ms on two separate traces),
the 100-tick number had more run-to-run spread (60-65 ms), which is
expected, timing noise matters more when the absolute numbers are small.

---

## Comparing Noir/Barretenberg and Rust/arkworks, Updated

Noir numbers are unchanged from `~/zk_fba_noir/PROTOCOL_DESIGN_AND_RESULTS.md`,
I didn't touch that codebase or re-run its benchmarks, since the whole point
of this pass was fixing the Rust side. Rust numbers are the fresh ones
above.

| Metric | Noir, N=100 | Rust, N=100 | Noir, N=1000 | Rust, N=1000 |
|---|---|---|---|---|
| Circuit/constraint size | 13,736 gates | 23 KZG commitments (+390 bit-gadget, unopened) | 111,611 gates | 23 KZG commitments (+390 bit-gadget, unopened) |
| Domain / opcode count | 6,326 ACIR opcodes | domain size 128 | 63,026 ACIR opcodes | domain size 1024 |
| Prove time | 0.13 s (`bb prove`) | ~60-65 ms (core proof) | 0.45 s (`bb prove`) | ~105.8 ms (core proof) |
| Verify time | 0.02 s (`bb verify`) | 2.94 ms (`batch_check`) | 0.01 s (`bb verify`) | 2.99 ms (`batch_check`) |
| Proof size | 14,656 bytes | ~15,264 bytes (est., see below) | 14,656 bytes | ~15,264 bytes (est.) |
| Rust's margin on core proof | | ~2.0-2.2x faster | | ~4.25x faster |

Compare that margin against the original document: 2.8x at N=100 and 3.3x
at N=1000. At N=100 the margin actually shrank a little (the FFT overhead
tax mentioned above), at N=1000 it grew from 3.3x to about 4.25x. The
important change isn't really the margin at these two points though, it's
that the mechanism the original document used to predict this margin would
shrink toward zero as N grows further is no longer there.

### Why the proof size flipped from "smaller" to "about the same, slightly bigger"

This is the one place I want to be very direct about, because the original
document's proof-size section made a claim that no longer holds and I don't
want to just quietly drop it.

The old estimate (2,784 bytes) counted 23 base commitments plus 4 opening
groups, 27 G1 points and 60 field elements at 32 bytes each. That estimate
was honest for the code as it stood then, the six bit-gadget commitment sets
were computed and discarded, so they were never part of what a verifier
needed to see.

Now that those six gadgets' 390 commitments get absorbed into the
Fiat-Shamir transcript (the soundness fix described above), the verifier
needs those 390 G1 points to recompute the same challenge the prover used.
That's 417 G1 points plus the same 60 field elements, 477 elements at 32
bytes, about 15,264 bytes. That's roughly 4% bigger than Noir's flat 14,656
bytes, not smaller anymore.

I think this is a fair trade and not a regression: the original 2,784-byte
number was cheap partly because it wasn't paying for six constraints' worth
of binding at all. A verifier reading the old proof had no way to know the
bit-gadget commitments even existed, let alone that they were unconstrained.
Now they're at least bound into the challenge. Finishing Phase 3 (the actual
opening) will add a modest amount more, a handful of extra evaluations
inside the existing GWC19 batch-open call, not 390 more elements, since
batch opening folds many polynomials into one opening group by design [Kate
et al. 2010]. I haven't measured that yet since Phase 3 isn't wired up, so
I'm not putting a number on it here.

### Complexity summary, corrected

| Factor | Rust/arkworks scaling | Noir/Barretenberg scaling | Winner at large N |
|---|---|---|---|
| Quotient polynomial computation | `O(n log n)` coset FFT (was `O(n^2)`) | `O(n log n)` coset NTT | Tie, same mechanism now |
| Prover-side sanity check (`verify_all`) | `O(n log n)` batch eval (was `O(n^2)`) | n/a, Noir has no equivalent layer | n/a |
| MSM throughput | `O(n/log n)`, 1 thread | `O(n/(k log n))`, many threads | Noir, still real |
| Fixed per-call overhead | Low, but FFT setup now paid even at small n | Higher (SRS load, Honk setup rounds) | Rust at small N, by a smaller margin than before |
| Constraint coverage per proof | 26 of 32 fully bound, 6 bound-but-not-opened, 0 fully unbound | All 33, in one proof | Noir (soundness), n/a for speed |
| Proof size | ~15,264 bytes, six constraints partially bound | 14,656 bytes, all 33 fully bound | Noir, by about 4%, and it's proving strictly more per byte |

The one row that used to say "Noir wins at large N" and now doesn't is
quotient computation, that was the whole basis for the original document's
crossover prediction. The row that's left, MSM threading, is a real
engineering gap (both systems are the same Pippenger `O(m/log m)` complexity
class [Pippenger 1976], one just uses more cores), and per the original
document's own reasoning it doesn't start to bite until n climbs into the
tens of thousands, well past both of the sizes measured here. I have not
re-run either codebase at N=5,000-20,000 as part of this pass, so I'm not
claiming to know whether a crossover still exists somewhere out there, only
that the specific mechanism that used to predict one with confidence at
these two sizes is gone.

---

So a hand-rolled prover can win on constant factors at small to medium N,
especially against a general-purpose constraint compiler doing strictly
more work per proof. What changed is that "small to medium N" no longer
comes with an expiration date driven by an `O(n^2)` time bomb sitting in the
quotient construction. What's left standing is a
threading gap that's an engineering problem, not an algorithmic one, and a
genuine, not-yet-finished soundness item (Phase 3 opening of the bit
gadgets) that costs a bit of proof size now and will cost a bit more once
it's actually finished properly.


---

## References

- Cooley, J. W., and Tukey, J. W. (1965). An algorithm for the machine calculation of complex Fourier series. *Mathematics of Computation*, 19(90), 297-301.
- Gabizon, A., Williamson, Z. J., and Ciobanu, O. (2019). PLONK: Permutations over Lagrange-bases for Oecumenical Noninteractive arguments of Knowledge. *Cryptology ePrint Archive*, Report 2019/953.
- Gabizon, A., and Williamson, Z. J. (2020). plookup: A simplified polynomial protocol for lookup tables. *Cryptology ePrint Archive*, Report 2020/315.
- Kate, A., Zaverucha, G. M., and Goldberg, I. (2010). Constant-size commitments to polynomials and their applications. In *Advances in Cryptology - ASIACRYPT 2010*, LNCS 6477, pp. 177-194. Springer.
- Knuth, D. E. (1997). *The Art of Computer Programming, Vol. 2: Seminumerical Algorithms* (3rd ed.), sec. 4.6.1. Addison-Wesley.
- Pippenger, N. (1976). On the evaluation of powers and related problems. In *17th Annual Symposium on Foundations of Computer Science*, pp. 258-263. IEEE.
- Wahby, R. S., Tzialla, I., Shelat, A., Thaler, J., and Walfish, M. (2018). Doubly-efficient zkSNARKs without trusted setup. In *IEEE Symposium on Security and Privacy*, pp. 926-943.
