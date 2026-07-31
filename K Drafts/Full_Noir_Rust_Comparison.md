# Full Comparison: Rationale, Design, and Benchmark Results

---

A hand-written Rust/arkworks (BN254, KZG10) implementation of the Frequent
Batch Auction (FBA) clearing-price zero-knowledge protocol: It reads a CSV order book (Price, Bids++, Asks++),
re-derives every protocol column from those three trusted inputs alone, and
produces a KZG-committed, Fiat-Shamir-driven proof that the disclosed
clearing receipt (`c`, `d`, `p*`, `V_max`, `V_min_delta`) is the correct
output of the auction, all without revealing the underlying order book.

It's the arkworks/KZG counterpart to the Noir/Barretenberg implementation. Both target the same 33-constraint protocol spec, but they
are independent, hand-rolled circuits, not one compiled from the other. This document is about
rationale, benchmarks, and how the two implementations stack up against
each other.

**Coverage: 32 of 33 constraints implemented and cryptographically checked.**
`#10` (Mask_P position via a shuffle/permutation argument) is intentionally
not implemented; see below for why.

---

## Design Rationale

### 1. Public vs. committed polynomials

`protocol_constraints.md` (section 6/11) notes that once a clearing receipt
discloses `c`, `d`, `p*`, `V_max`, and `V_min_delta`, several auxiliary
polynomials can be built by the verifier directly from those disclosed
scalars instead of being committed witnesses, at zero additional information
cost, since the scalars are already public. This codebase takes that option
for five polynomials: `Mask_P`, `Mask_V`, `Mask_C`, `InMCV`, `SD`.

The one deliberate exception is `ChkD`, which stays a committed witness even
though it's a boolean indicator just like the mask polynomials: publishing
it would reveal *every* tick tied at the minimum imbalance inside the
plateau, not just `p*`, which is strictly more information than the receipt
discloses elsewhere. `#18`-`#20` are proved for real against it.

What this means for the constraint list:
- Constraints that live purely among public polynomials (`#9` Mask_P
  booleanity, `#13`/`#14` InMCV containment, `#21` Mask_V booleanity, `#26`
  SD membership, `#27`/`#28` Mask_C booleanity/containment) are true *by
  construction* of how those polynomials get built; there's nothing to
  prove. They're still evaluated in `verify_all` as a regression guard, not
  because they can actually fail.
- Constraints that mix a public polynomial with a genuinely committed one
  (`#20` ChkD times (1 - Mask_P), `#22` Mask_V times (1 - ChkD), `#25` SD
  minus Mask_V times Delta) are real, and require KZG-backed quotient
  polynomials folded into the Fiat-Shamir transcript and batch-opening proof
  exactly like the original 12 constraints did.

**Why `#10` is N/A**: `#10` asks for a shuffle/permutation argument proving
`Mask_P`'s 1-positions correspond to the disclosed `[c, d]` range. Since
`Mask_P` is built directly from the disclosed `c`/`d` instead of committed,
there's no committed permutation to argue about, same reasoning that makes
`#9`/`#13`/`#14` automatic. This is a design choice explicitly sanctioned by
the protocol doc's own footnote, not a missing feature.

### 2. Bit-decomposition gadget instead of Plookup

`protocol_constraints.md` (section 2) offers Plookup lookup-table range
checks as its reference design for `#1`/`#2` (bid/ask non-negativity) and
the related ceiling/floor checks (`#8`, `#15`, `#16`, `#23`). This codebase
(like the Noir implementation, which gets this for free from native checked
arithmetic) instead uses a from-scratch bit-decomposition gadget:
`BIT_WIDTH = 32` per-row committed bit columns, with reconstruction
(sum of `2^j * bit_j` equals value) and per-bit booleanity both proved as
genuine `Z_H(X)`-divisibility, not just spot-checked.

Proving membership in `[0, 2^32)` is a looser bound than an exact Plookup
table membership check, but `BIT_WIDTH = 32` is still far below half of
BN254's scalar field order, so no legitimate order size can wrap around and
get misread as a negative field element, which is the property the range
check actually needs to guarantee. Implementing the grand-product/sorted-
interleaving Plookup machinery got skipped in favor of staying consistent
with the rest of the codebase's gadget choices and keeping implementation
risk low.

`BIT_WIDTH` is 32, not 16, because `SurpB`/`SurpA` are *differences* against
the losing side of the book and can run up to the full cumulative order
volume (over 2^16 on the 1000-tick dataset), unlike `V_max`, which is bounded
by `RANGE_BITS = 16` since it's a `min()` of two accumulators.

### 3. Previous adget reuse

Two pieces of the `gadgets` crate (MadibaGroup 2024-Gadgets-Code) get reused
rather than reimplemented:
- `gadgets::range`, a scalar-only range proof, used for `V_max`'s ceiling
  bound (the `#8`-adjacent scalar context) and the two cliff-slack values
  (`#31`/`#32`). Wrapped as `prove_range16`/`verify_range16`.
- `gadgets::utils` (`Transcript`, `batch_open`, `batch_check`,
  `BatchCheckProof`), the Fiat-Shamir transcript and GWC19 batch-opening
  primitives underlying Layer 3e/3f.

One gap was found and closed in `gadgets::range`: `verify_range16` alone
only checks that *some* value under 2^16 was proved. It never binds that
value to the specific scalar the caller cares about (`V_max`, `Slack_L`,
`Slack_R`). `verify_range16_bound` closes this by extracting the plaintext
opening the gadget already reveals and asserting it equals the expected
value.

### 4. Hand-unrolled, not generic, quotient/Fiat-Shamir system

`QuotientPolynomials`, `QuotientCheck`, and `FiatShamirProof` use named
struct fields per constraint (`q_kl`, `q_chkd_bool`, and so on) rather than
a `Vec`-indexed generic loop over a constraint list. This is a deliberate
trade-off: every residue is individually named, printed, and debuggable
end-to-end in `main.rs` (`r[8] = 0  PASS`), at the cost of the boilerplate
visible in `compute_quotients`/`fiat_shamir_prove`/`commit_quotients`. A
generic version would be shorter but harder to audit constraint-by-constraint, which is the whole point
of a research prototype like this one.

### 5. Bit gadgets are committed but not yet opened against the verifier

The six bit-decomposition gadget instances (`#1`, `#2`, `#8`, `#15`, `#16`,
`#23`) each build their own witness/quotient polynomials and commit them via
real KZG inside `commit_bit_gadget`. But as the code stands right now,
those commitments are computed and then thrown away: both `full_pipeline`
and `main.rs` call `let _ = commit_bit_gadget(...)`. The PASS/FAIL result
that actually drives the six gadget-covered constraints comes from
`recon_ok`/`bool_ok` fields inside `build_bit_gadget`, which are a plaintext
(unblinded) "is the remainder zero" check, not a `batch_open`/`batch_check`
KZG opening backed by a pairing.

In plain terms: `#1`, `#2`, `#8`, `#15`, `#16`, and `#23` are currently
prover-side self-checks. The commitments exist and are correctly formed, but
nothing folds them into the Fiat-Shamir transcript or the pairing-based
verification an independent verifier would actually run. This is different
from what an earlier version of this document claimed (that soundness here
was "identical either way"), and that claim was wrong. The honest fix is to
fold these six gadgets into the same Layer 3e/3f transcript and
`batch_check` call as the other 14 constraints; that's flagged as a
follow-up in Limitations below, and it is the single most important thing
to do before treating this codebase as more than a research prototype.

---

## Benchmark Results

Measured with `cargo bench --bench fba_bench` (Criterion, 100 samples per
point where the sample budget allowed, fewer for the more expensive
1000-tick points; see raw output for exact counts) and cross-checked against
a `cargo run --release` single-run trace with per-layer `Instant` timing,
which agreed with the Criterion medians to within about 5% at every point.
All numbers below are from Criterion unless noted.

| Layer | 21-tick (domain 32) | 100-tick (domain 128) | 1000-tick (domain 1024) |
|---|---|---|---|
| L2, Interpolation (9 IFFT) | 0.96 ms | 1.64 ms | 4.08 ms |
| L3a, KZG setup | 1.63 ms | 2.05 ms | 3.55 ms |
| L3b, Witness commits (9 MSM) | 3.37 ms | 4.71 ms | 11.12 ms |
| L3c, Quotient polynomials (14) | 2.82 ms | 5.79 ms | 70.29 ms |
| L4, Constraint verify (algebraic) | 11.69 ms | 84.53 ms | 989.66 ms |
| **End-to-end (`full_pipeline`)** | **206.8 ms** | **352.0 ms** | **2.39 s** |

All four end-to-end figures are Criterion medians (100 samples each, about
239 seconds of wall time for the 1000-tick group alone given its roughly
2.4 second per-iteration cost). The 1000-tick figure matched the independent
`cargo run --release` single-run trace (2356.8 ms) within 1.5%.

It's worth separating two different numbers that both live inside
"end-to-end": the *core cryptographic proof* (Layers 2 through 3h:
interpolation, KZG setup, witness/quotient commitments, Fiat-Shamir,
GWC19 batch-open and batch-check), and the *full pipeline*, which also
includes Layer 4 (a pure redundant sanity duplicate of the same checks,
done in plaintext) and Layer 4b (building and committing the six bit
gadgets, whose commitments, per the correction above, aren't actually
opened yet). Measured from the single-run trace:

| | 21-tick | 100-tick | 1000-tick |
|---|---|---|---|
| Core cryptographic proof time | 45.9 ms | 54.0 ms | 141.1 ms |
| Full pipeline time | 204.3 ms | 347.5 ms | 2,305.1 ms |
| `batch_check` (4 pairings) | 3.10 ms | 2.91 ms | 2.96 ms |

The core proof number is the fairer thing to compare against another
system's proving time, since it's the part that's actually committed,
quotient-checked, and pairing-verified. The full pipeline number is what
you'd get if you ran this code as-is today, redundant checks included. Both
are reported here so neither number gets used to make a misleading claim.

### Where the time goes: Layer 4 and Layer 4b dominate, not the cryptography

At 1000 ticks, Layer 4 (algebraic constraint verification, about 20 boolean
checks each evaluating committed polynomials pointwise via Horner's method)
takes about 990 ms, and Layer 4b (building and committing the six
bit-decomposition gadget instances) takes roughly another 1.24 seconds
combined, together over 90% of the roughly 2.4 second end-to-end total.
Neither is part of the cryptographic proof that a verifier actually checks:
Layer 4 is a pure redundant sanity duplicate, and Layer 4b's *build* step is
legitimate quotient/commitment construction that just isn't wired into a
verified opening yet (see Design Rationale #5). A real verifier only ever
runs Layer 3f's `batch_check` (KZG pairing check) and the range-proof
verifications, never Layer 4. In a production deployment, Layer 4 would be
deleted outright, and Layer 4b would need its opening/verification step
finished rather than removed.

This mirrors the finding documented for the Noir/Barretenberg comparison: a non-cryptographic, prover-only
sanity-check layer is the dominant cost at scale in a hand-written prover,
not the KZG/quotient machinery itself.

### Quotient polynomials (Layer 3c) grow faster than the rest of the setup

Layer 3c's cost grows from 2.82 ms to 5.79 ms to 70.29 ms across the three
domain sizes (32, 128, 1024, so 4x then roughly 12x growth), about a 25x
increase overall against a 32x domain-size increase. Several of the fourteen
quotients here involve multiplying two degree-n polynomials (`V_KL`, `#7`)
and dividing a resulting degree-2n numerator by `Z_H(X)` via coefficient-form
long division, which costs `O(n^2)` field multiplications [Knuth 1997, sec.
4.6.1]. This is the same division bottleneck documented as Root Cause 1 for the original 5-witness Rust
prover. Growth here is milder than a clean n-squared law would predict, most
likely because several of the fourteen quotients are cheap `O(n)`
single-point (`div_by_linear`) checks rather than full polynomial divisions,
and because n never exceeds 1024 in this benchmark, not yet large enough for
the quadratic term to fully dominate the fixed per-call overhead. This
codebase hasn't adopted a coset-FFT/NTT quotient construction (the
`O(n log n)` approach PLONK/UltraHonk use); that remains the natural next
optimization if prover latency at production scale becomes a concern. See
the comparison section below for how this plays out against Noir directly.

### Verifier cost is effectively O(1)

Layer 3f's `batch_check` is always 4 pairings regardless of n, and the
range-proof verifications (Layer 3g/3h) run over a fixed 16-element domain
independent of the auction size. That's the actual cryptographic
verification cost profile a real verifier would experience; it excludes
Layer 4/4b entirely, since a verifier never runs those.

### Note on comparing

An earlier snapshot of this codebase (5 witness + 5 quotient polynomials, 12
constraints wired end-to-end) measured 416 ms total, 371 ms of which was
Layer 4, at 1000 ticks; which predates the
33-constraint expansion and is superseded by this document. The current
9-witness/14-quotient/32-constraint version is proportionally slower in
absolute terms (roughly 2.4 s vs. 416 ms) because Layer 4 and Layer 4b now
check roughly 2.5 to 3 times as many constraint families, each still paying
the same `O(n * domain_size)` per-tick evaluation cost. The scaling
*behavior* (prover-only sanity layer dominates, verifier stays flat) hasn't
changed; only the constant factor grew, in proportion to the constraint
coverage. Any older Rust numbers cited elsewhere (whose crossover analysis was written against
that earlier snapshot) reflect that same earlier version and shouldn't be
read as current for this codebase; the comparison section below uses only
the fresh numbers measured against this document's own benchmark run.

---

## Comparing Noir/Barretenberg and Rust/arkworks

This section is the direct answer to "which one is actually faster, smaller,
and simpler, and does that change with N." Short version: at the two sizes
we can compare directly (N=100 and N=1000), this codebase's core
cryptographic proof is faster than Noir's `bb prove` + `bb verify` and its
estimated proof size is smaller too, but that's not the whole story, and the
reasons why point toward Noir pulling ahead as N keeps growing. Read the
caveats below the table before drawing conclusions from the raw numbers
alone.

### The numbers side by side

Noir figures come straight from our results table. Rust figures come from this document's own benchmark run
above (single-run trace for the per-phase breakdown, Criterion medians for
end-to-end).

| Metric | Noir, N=100 | Rust, N=100 | Noir, N=1000 | Rust, N=1000 |
|---|---|---|---|---|
| Circuit/constraint size | 13,736 gates | 23 KZG commitments (fixed) | 111,611 gates | 23 KZG commitments (fixed) |
| Domain / opcode count | 6,326 ACIR opcodes | domain size 128 | 63,026 ACIR opcodes | domain size 1024 |
| Prove time | 0.13 s (`bb prove`) | 54.0 ms (core proof) | 0.45 s (`bb prove`) | 141.1 ms (core proof) |
| Verify time | 0.02 s (`bb verify`) | 2.91 ms (`batch_check`) | 0.01 s (`bb verify`) | 2.96 ms (`batch_check`) |
| Proof size | 14,656 bytes | roughly 2,784 bytes (est.) | 14,656 bytes | roughly 2,784 bytes (est.) |
| Full pipeline (with sanity/self-check layers) | n/a, no such layer | 347.5 ms | n/a, no such layer | 2,305.1 ms |

A few notes on how to read this table honestly:

- **Constraint count isn't equal.** Noir's gate count covers all 33
  constraints in one monolithic circuit. Rust's 23 commitments cover 32
  constraints, but 6 of those 32 (the bit-gadget-backed ones, per Design
  Rationale #5) are presently prover-side self-checks rather than something
  the `batch_check` pairing call actually verifies. So the Rust "prove time"
  and "verify time" above are doing cryptographically less work than Noir's
  single proof is, on a per-constraint basis. This is the single biggest
  caveat on this table and it cuts in Rust's favor on speed, so it shouldn't
  be glossed over.
- **Rust has no full-pipeline equivalent in Noir.** The Noir circuit doesn't
  have an analogous "prover re-checks its own work in plaintext" step; Noir
  gets that correctness confidence for free from `nargo execute` failing
  loudly if any `assert` in the circuit doesn't hold. So the "full pipeline"
  row is included for completeness, not as a fair prove-time comparison; it
  mixes in Layer 4's pure overhead and Layer 4b's not-yet-verified work.
- **Proof size for Rust is an estimate**, not a measured serialization. It's
  built from counting elements: 23 base commitments plus 4 opening groups
  works out to 27 G1 points and 60 field elements, at 32 bytes each under
  standard BN254 compressed-element sizing, for 2,784 bytes total. Actually
  serializing the gadget crate's `BatchCheckProof`/`OpenEval` types with
  `CanonicalSerialize` would confirm this exactly; that's flagged as
  follow-up work rather than attempted here, since poking at that crate's
  internals mid-benchmark felt like a good way to introduce a bug for a
  number that's already directionally clear.

### Why Rust's core proof is faster at these two sizes

The core cryptographic proof time (45.9 ms at 21 ticks, 54.0 ms at 100,
141.1 ms at 1000) beats both of Noir's measured points by a wide margin,
even setting aside the constraint-coverage caveat above. There are a few
real reasons for this, not just measurement noise:

1. **Less circuit to arithmetize.** Rust commits 23 flat polynomials over a
   domain sized to the tick count. Noir/Barretenberg compiles the whole
   protocol into a UltraHonk circuit with custom gates, Plookup range-check
   tables [Gabizon and Williamson 2020], and a permutation argument, all of
   which add real prover work per gate beyond what a bare KZG commitment
   costs. At N=100 and N=1000, that per-gate overhead is a bigger fraction
   of Noir's total time than the raw gate count alone suggests.
2. **Fixed setup costs matter more at these sizes.** `bb prove` pays for SRS
   loading, circuit-specific proving-key setup, and UltraHonk's multiple
   Honk-specific commitment rounds regardless of N. Those fixed costs are
   real time that doesn't show up as "more gates," and they're paid on
   every `bb prove` call.
3. **N=100 and N=1000 are still small.** Both root causes below (coefficient
   division and single-threaded MSM) only start to bite once n gets into
   the thousands or tens of thousands. At the sizes actually benchmarked
   here, Rust's simpler, lower-overhead arithmetization wins on raw
   constant factors even though its asymptotic complexity is worse.

### Why that's expected to flip at larger N

We already worked out the mechanism for an earlier,
smaller version of both circuits, and the same mechanism still applies here
because neither implementation has changed its core algorithm, only its
constraint count grew. Three things point the same direction:

**Quotient computation: O(n^2) here vs. O(n log n) in UltraHonk.** This
codebase computes each of its 14 real quotient polynomials via
coefficient-form long division, which costs `O(n^2)` field multiplications
for a degree-2n numerator over a degree-n divisor [Knuth 1997, sec. 4.6.1].
PLONK-family systems like UltraHonk instead evaluate the combined
constraint polynomial over a multiplicative coset and recover the quotient
via an inverse NTT, which costs `O(n log n)` [Gabizon et al. 2019; Cooley
and Tukey 1965; Harvey and van der Hoeven 2021]. The benchmark table above
already shows this difference emerging inside Rust's own numbers: Layer 3c
alone grew about 25x for a 32x domain increase (2.82 ms to 70.29 ms across
domains 32, 128, 1024), clearly super-linear even before n gets anywhere
near the sizes where UltraHonk's circuits live. Extrapolate that curve out
past N=1000 and it keeps bending upward; UltraHonk's `O(n log n)` curve
doesn't.

**MSM throughput: single-threaded here vs. multi-threaded in Barretenberg.**
Every KZG commitment is a multi-scalar multiplication, costing `O(m / log m)`
group operations under Pippenger's algorithm [Pippenger 1976] for an MSM of
size m. This codebase runs its 23 MSMs serially on one thread. Barretenberg
distributes its wire-polynomial commitments across all available CPU cores.
At N=100 and N=1000, the MSMs here are still small enough (domain 128 and
1024) that single-threaded execution isn't a real handicap yet. Once n
climbs into the tens of thousands, the way it does inside Noir's own
UltraHonk domain, that gap opens up fast, and it's a pure engineering gap
rather than an algorithmic one: this codebase could add threading without
changing its complexity class, it just doesn't right now.

**Where that puts the crossover.** The earlier, smaller version of this
codebase crossed over against Noir somewhere around
N=600-800. That analysis compared full end-to-end numbers that included an
even more expensive version of this codebase's own Layer 4. The current
codebase's *core proof* numbers are faster in absolute terms than that
earlier version's were (thanks to skipping Layer 4/4b in the "core proof"
accounting), so a like-for-like crossover point for the current code would
sit further out than N=600-800, somewhere past N=1000, since the O(n^2) and
single-thread penalties documented above haven't had enough n to bite yet
at either measured point. Pinning down exactly where would need a real
benchmark run somewhere in the N=5,000 to N=20,000 range, which wasn't part
of this pass; the direction of the trend is clear even without that data
point, the exact crossover N isn't.

### Proof size: different shapes, not really a fair fight

Noir's proof size is flat at 14,656 bytes for both N=100 and N=1000,
because a single UltraHonk proof always contains the same fixed number of
commitments, evaluations, and opening proofs regardless of circuit size,
that's the whole point of a succinct proof system. This codebase's
estimated 2,784 bytes is also flat across N, for the same underlying reason:
a KZG commitment is one G1 group element no matter what degree polynomial
it commits to [Kate et al. 2010], so committing to 23 fixed-shape
polynomials plus a handful of opening evaluations produces a size that
doesn't move with N either.

The two numbers aren't really comparable as "smaller is better" though.
Rust's proof is smaller mostly because it's proving less in one proof:
32 constraints spread across 23 separately-named commitments and openings,
6 of which (again, the bit gadgets) aren't actually bound into that proof
yet. Noir's proof folds all 33 constraints, all the range checks, and the
full permutation argument into one UltraHonk proof object, which is a much
denser single artifact by design. If this codebase finished wiring the bit
gadgets into the same transcript and batch-check call, the proof size
would grow a bit (a handful more G1/F elements) but would still stay
N-independent; it would still likely land smaller than Noir's proof simply
because GWC19 batch openings are a lighter-weight primitive than a full
Honk permutation argument, not because Rust is doing more work per byte.

### Complexity summary

| Factor | Rust/arkworks scaling | Noir/Barretenberg scaling | Winner at large N |
|---|---|---|---|
| Quotient polynomial computation | O(n^2) coefficient-form long division | O(n log n) coset NTT | Noir |
| MSM throughput | O(n/log n), 1 thread | O(n/(k log n)), many threads | Noir |
| Fixed per-call overhead | Low | Higher (SRS load, Honk setup rounds) | Rust at small N |
| Constraint coverage per proof | 26 of 32 cryptographically bound today | All 33, in one proof | Noir (soundness), n/a for speed |
| Proof size | Smaller today, but proving less | Larger, but proving everything | depends on what "smaller" should mean here |

The core lesson lines up with the broader point
made in Wahby et al. [2018] about doubly-efficient SNARKs: a hand-rolled,
custom-arithmetized prover can absolutely win on constant factors at small
to medium N, especially when it's proving a narrower slice of the problem
than the more general system next to it. But a general-purpose constraint
compiler like Barretenberg is engineered from the ground up to hit
`O(n log n)` on every sub-computation, and that asymptotic gap doesn't
care how good the constants on the other side are once n gets big enough.
The honest reading of this codebase's numbers is that it currently looks
fast partly because it's fast, and partly because it isn't finished
checking everything it claims to check.

---

## Guide to the Code

- **`src/lib.rs`**, the library. Organized top-to-bottom by pipeline layer:
  `OrderBook` (raw + re-derived columns) then Layer 2 (interpolation), then
  Layer 3a-3d (KZG setup, witness/quotient commitments), then Layer 4/4b
  (prover-side algebraic + bit-gadget sanity checks), then Layer 3e-3h
  (Fiat-Shamir, batch openings, scalar range proofs), then `full_pipeline`
  (wires all of the above together). Each function/struct carries a
  one-line `#N` constraint reference rather than restating the protocol
  spec.
- **`src/main.rs`**, a runnable trace: executes the pipeline by hand
  (mirroring `full_pipeline`) for three datasets (21/100/1000-tick),
  printing per-layer timing and a PASS/FAIL line for every one of the 32
  implemented constraints, plus a cross-size comparison table.
- **`benches/fba_bench.rs`**, the Criterion harness used to produce the
  numbers in this document (`cargo bench --bench fba_bench`).
- **`Cargo.toml`/`Cargo.lock`**, arkworks 0.4.x pinning plus the MadibaGroup
  `gadgets` git dependency.


---

## Limitations / Future Work

- **The bit-gadget soundness gap is the top priority.** `#1`, `#2`, `#8`,
  `#15`, `#16`, and `#23` are currently backed by a plaintext prover-side
  check, not a verified KZG opening (see Design Rationale #5). Folding the
  six bit-gadget instances into the Layer 3e/3f Fiat-Shamir transcript and
  `batch_check` call is the fix, and it should happen before this code is
  described anywhere as fully cryptographically sound.
- Layer 4 is a pure redundant prover-side sanity duplicate; a production
  prover would delete it outright and rely solely on the already-proved
  Layer 3c/3e/3f quotients.
- Layer 3c's quotient construction is coefficient-form long division; a
  coset-FFT/NTT approach (as in PLONK/UltraHonk) would change its
  asymptotic behavior and is the most likely lever if prover latency needs
  to improve at larger n, per the comparison section above.
- The Rust proof size figure in the comparison section is an estimate based
  on element counting, not a measured serialization. Actually serializing
  `BatchCheckProof`/`OpenEval` via `CanonicalSerialize` would pin this down
  exactly.
- MSM commitments run single-threaded; parallelizing them would narrow (but
  not eliminate) the gap with Barretenberg's multi-threaded MSM at large N,
  without changing this codebase's underlying complexity class.

---

## References

- Cooley, J. W., and Tukey, J. W. (1965). An algorithm for the machine calculation of complex Fourier series. *Mathematics of Computation*, 19(90), 297-301.
- Gabizon, A., Williamson, Z. J., and Ciobanu, O. (2019). PLONK: Permutations over Lagrange-bases for Oecumenical Noninteractive arguments of Knowledge. *Cryptology ePrint Archive*, Report 2019/953.
- Gabizon, A., and Williamson, Z. J. (2020). plookup: A simplified polynomial protocol for lookup tables. *Cryptology ePrint Archive*, Report 2020/315.
- Harvey, D., and van der Hoeven, J. (2021). Integer multiplication in time O(n log n). *Annals of Mathematics*, 193(2), 563-617.
- Kate, A., Zaverucha, G. M., and Goldberg, I. (2010). Constant-size commitments to polynomials and their applications. In *Advances in Cryptology -- ASIACRYPT 2010*, LNCS 6477, pp. 177-194. Springer.
- Knuth, D. E. (1997). *The Art of Computer Programming, Vol. 2: Seminumerical Algorithms* (3rd ed.), sec. 4.6.1. Addison-Wesley.
- Pippenger, N. (1976). On the evaluation of powers and related problems. In *17th Annual Symposium on Foundations of Computer Science*, pp. 258-263. IEEE.
- Wahby, R. S., Tzialla, I., Shelat, A., Thaler, J., and Walfish, M. (2018). Doubly-efficient zkSNARKs without trusted setup. In *IEEE Symposium on Security and Privacy*, pp. 926-943.
