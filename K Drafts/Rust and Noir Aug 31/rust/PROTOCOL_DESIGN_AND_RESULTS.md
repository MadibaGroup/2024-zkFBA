# ZK-FBA full-protocol prover (Rust/arkworks): design rationale, code guide, results

This document covers `zk_fba_csv_full` -- a hand-written Rust/arkworks
(BN254, KZG10) prover implementing 32 of the 33 constraints from
`protocol_constraints.md` (Section 14's Full Constraint List) for the
zero-knowledge frequent batch auction (FBA) protocol. It explains *why*
the prover is built the way it is and *what* each part of `src/lib.rs`
does, plus the benchmark results.

This is the arkworks counterpart to `~/zk_fba_noir`'s Noir circuits
(see that project's own `PROTOCOL_DESIGN_AND_RESULTS.md`). Both target
the same spec, but they're independent, hand-rolled implementations,
not one compiled from the other. If you want a side-by-side latency,
complexity, and proof-size comparison between the two, that lives in
`RATIONALE_AND_RESULTS.md` -- this file stays focused on the Rust side
by itself.

## What the prover proves

Given private per-tick bid/ask volumes `B(X)`/`A(X)` over `N` price
ticks, the prover shows that a disclosed clearing receipt (`V_max`,
`V_min_delta`, `c`, `d`, `p*`) is the correct output of the FBA clearing
algorithm, without revealing any individual bid or ask value. Unlike
Noir, where `N` has to be a compile-time constant, this is plain Rust,
so `N` is just whatever the CSV (or hardcoded dataset) contains --
`OrderBook::domain_size` picks the next power of two at runtime and
everything downstream (interpolation, KZG setup, quotients) sizes
itself off that.

## Guide to the code

Each block below corresponds to a section of `protocol_constraints.md`,
using the same numbering the Noir doc uses, so you can read the two
side by side. Function names refer to `src/lib.rs`.

**Section 3 -- bid/ask range check (#1, #2).**
Noir gets this for free from a native checked `u64` type. Rust's field
elements don't have that, so this codebase reuses the bit-decomposition
gadget (see "Why bit-decomposition, not Plookup" below) instead of
either a native check or an actual Plookup table:
`build_bit_gadget_set`'s `bid_range`/`ask_range` instances, `BIT_WIDTH =
32` bits each. One honest caveat, covered in more depth in the
Limitations section: as currently wired, `full_pipeline` commits these
polynomials but never opens/verifies them cryptographically, so this
range check (along with #8, #15, #16, #23 below) is presently a
prover-side self-check, not something a verifier independently confirms.

**Section 4 -- accumulator init + transition (#3-#6).**
`OrderBook::derive` computes `acc_b` (Bid Depth) as a backward running
sum seeded at the top tick, and `acc_a` (Ask Depth) as a forward running
sum seeded at the bottom tick, same direction logic as the Noir version.
The difference is where the *proof* of these lives: in Noir, the
`assert(acc_b[i] == acc_b[i+1] + b[i])` line *is* the constraint. Here,
`compute_quotients` turns each identity into a real KZG quotient --
`q_acc_a_init`/`q_acc_b_init` via `div_by_linear` (single-point checks
at `omega^0` / `omega^{n-1}`), `q_acc_a_rec`/`q_acc_b_rec` via
`poly_div_rem` against `Z_H(X)` after shifting the polynomial by `omega`
(`shift_omega`). A forged witness produces a nonzero remainder here,
which shows up as a nonzero Fiat-Shamir residue (`r1`-`r4` in
`fiat_shamir_prove`) and fails the GWC19 pairing check in Layer 3f.

**Section 5 -- Min(X) mutual exclusivity + ceiling (#7, #8).**
`#7` uses the same product-is-zero trick as Noir's Section 5:
`(AccA-Min)*(AccB-Min) = 0`, expressed as a real quotient (`q_kl`) since
`Min(X)` is a committed polynomial here rather than a plain-Field
assertion. `#8` (the ceiling `V_max - Min(X) >= 0`) isn't an equality,
so it doesn't go through the quotient system at all -- it's the
`ceiling` instance of the bit-decomposition gadget.

**Section 6/7 -- plateau endpoints (#9-#14).**
Same "compute it, don't prove it separately" idea as Noir's masks, just
phrased in KZG terms: `mask_p_poly`/`inmcv_poly` build `Mask_P`/`InMCV`
directly from the disclosed `c`/`d`/`V_max` rather than committing them
as witnesses. `#9` (Mask_P booleanity), `#13`/`#14` (InMCV
containment) hold automatically by construction -- `verify_all` still
checks them in the clear as a regression guard, but there's no crypto
backing them because there's nothing to forge. `#11`/`#12` (the plateau
endpoints, `Min(omega^c) == V_max` and `Min(omega^d) == V_max`) are real
quotients (`q_plateau_left`/`q_plateau_right`), since `Min(X)` is
committed.

**Section 8/9 -- surplus + Delta (#15-#17).**
This is the biggest structural difference from Noir. Noir writes
`surp_b[i] + min_x[i] == acc_b[i]` as an addition specifically so the
checked-subtraction-via-addition trick enforces non-negativity as a side
effect, for free. Rust's `DensePolynomial` coefficients live in a raw
prime field with no such side effect, so `SurpB`/`SurpA` non-negativity
needs its own gadget: the `surp_b_nn`/`surp_a_nn` bit-decomposition
instances. `#17` (`Delta == SurpA + SurpB`) is a real quotient
(`q_delta_def`), since it's an equality rather than a range check.

**Section 10/11 -- valley pin (#18-#24).**
`ChkD` stays a committed witness here (see "Why masks are computed, not
committed" below), so `#18` (booleanity), `#19` (correctness), and `#20`
(containment against `Mask_P`) are all real quotients
(`q_chkd_bool`/`q_chkd_correct`/`q_chkd_contain`). `Mask_V` is public
(`mask_v_poly`, built from the disclosed `p*`), so `#21` is automatic,
but `#22` (`Mask_V*(1-ChkD) = 0`) mixes a public polynomial with the
committed `ChkD`, so it's still a real quotient (`q_mask_v_contain`) --
this is the same reasoning as `#20`. `#24` (the valley pin,
`Delta(omega^{p*}) == V_min_delta`) is a real quotient
(`q_valley_pin`) via `div_by_linear`.

**Section 12 -- SD membership (#25, #26).**
`SD` is public too (`sd_poly`, built from the disclosed `V_min_delta`
and `p*`). `#25` (`SD - Mask_V*Delta = 0`) mixes it with the committed
`Delta`, so it's a real quotient (`q_sd_def`) -- it collapses to
"`Delta(p*)` really does equal `V_min_delta`," re-confirming the valley
pin through a second algebraic path. `#26` (membership in `{0,
V_min_delta}`) is automatic by construction, same redundant-but-
harmless treatment as `#9`/`#13`/`#14`.

**Section 13 -- cliff exhaustiveness (#27-#32).**
`Mask_C` is public (`mask_c_poly`, built from the disclosed cliff
positions `c-1`/`d+1`), so `#27` is automatic and `#28` (`Mask_C` and
`Mask_P` don't overlap) is checked in the clear. `#29`/`#30` (the cliff
`Min(X)` values) aren't separate quotients at all -- they're plaintext
openings of the already-committed `Min(X)` at `omega^{c-1}`/`omega^{d+1}`
via `batch_open`, feeding directly into `#31`/`#32` (cliff slack
non-negativity), which are proved with `gadgets::range` rather than the
bit-decomposition gadget -- the same scalar range-proof machinery used
for `V_max`'s ceiling bound. So this codebase actually ends up using two
different range-proof strategies for different constraints (bit
decomposition for per-row checks, `gadgets::range` for one-off scalar
checks), where Noir needs neither.

## Why masks are computed, not committed

`Mask_P`/`Mask_V`/`Mask_C`/`InMCV`/`SD` are derived directly from the
disclosed public scalars `c`, `d`, `p*`, `V_max`, `V_min_delta` instead
of being separate committed polynomials proved correct via a
shuffle/permutation argument. This is `protocol_constraints.md`'s own
Section 6/11 "cleanest design" footnote, same reasoning as the Noir
side.

Why it doesn't leak anything: the clearing receipt already discloses
these scalars no matter which approach you take. A shuffle argument
would still need to open the same values at the end -- it would just
spend extra KZG commitments and pairing checks proving that a
separately-committed 0/1 polynomial is *consistent* with `c`/`d`/`p*`,
without changing what gets revealed. Individual `B(X)`/`A(X)` values are
never opened either way. So the computed-mask approach makes `#9`,
`#10`, `#13`, `#14`, `#21`, `#26`, `#27`, `#28` hold by construction, at
no privacy cost, and `#10` specifically (Mask_P position via a
shuffle/permutation argument) becomes not applicable, since there's no
committed permutation left to argue about.

`ChkD` is the one polynomial in this family that's kept committed rather
than public. Publishing it would reveal every tick tied at the minimum
imbalance inside the plateau, not just `p*`, which is more than the
receipt discloses anywhere else. So `#18`-`#20` (and `#22`, which mixes
`ChkD` with the public `Mask_V`) stay real, KZG-backed constraints.

## Why bit-decomposition, not Plookup, and why Rust needs it where Noir doesn't

`protocol_constraints.md` Section 2/3 offers a bit-decomposition gadget
and a Plookup lookup table as two options for range and non-negativity
checks. Noir sidesteps needing either, because its native `u64` type is
already range-checked and its arithmetic already fails witness
generation on underflow -- the property those gadgets exist to
simulate is just built into the language there. Rust's `ark_bn254::Fr`
field elements have no such property: subtraction wraps around the
field modulus silently, so nothing stops a malicious prover from
claiming a negative `SurpB` "wrapped" into a huge positive field
element unless something explicitly checks the range.

This codebase picks bit decomposition over Plookup: `BIT_WIDTH = 32`
committed bit columns per instance, with reconstruction
(`sum 2^j * bit_j = value`) and per-bit booleanity both proved as
genuine `Z_H(X)`-divisibility rather than spot-checked. Proving
membership in `[0, 2^32)` is a looser bound than an exact Plookup table
membership check, but `32` bits is still far below half of BN254's
scalar field order, so no legitimate order size can wrap around and get
misread as negative, which is the actual property the check needs to
guarantee. Implementing Plookup's grand-product/sorted-interleaving
machinery was skipped in favor of staying consistent with the rest of
the codebase's gadget choices, and because it's a meaningfully bigger
implementation lift for the same guarantee at this scale.

`BIT_WIDTH` is 32, not 16 (`RANGE_BITS`, used for `V_max`/slack), because
`SurpB`/`SurpA` are differences against the *losing* side of the book
and can run up to the full cumulative order volume -- past 2^16 already
on the 1000-tick dataset -- whereas `V_max` is bounded by 16 bits since
it's a `min()` of two accumulators.

## Design decisions on witness data

Same policy as the Noir side: CSV columns are never trusted as input.
`OrderBook::from_csv` reads only `Price`/`Bids++`/`Asks++`, then
`OrderBook::derive` recomputes every other column
(`AccB`/`AccA`/`Min`/`SurpB`/`SurpA`/`Delta`/`ChkD`/`V_max`/
`V_min_delta`/`c`/`d`/`p*`) from those three columns alone, and
cross-checks the result against the CSV's own precomputed columns as a
guard against a preprocessing bug in the CSV generator -- not as a
source of witness data. A tampered CSV column can't silently pass
through as input; only the raw bid/ask volumes are trusted.
`OrderBook::hardcoded_21tick()` runs the same `derive` pipeline over a
fixed 21-tick example for a small baseline dataset with no CSV
dependency.

## Results (2026-07-24, Apple M-series, arkworks 0.4.x, ark-bn254)

| | 21-tick | 100-tick | 1000-tick |
|---|---|---|---|
| n (price ticks) | 21 | 100 | 1000 |
| Domain size | 32 | 128 | 1024 |
| Plateau `[c, d]` | [10, 16] | [44, 45] | [427, 429] |
| V_max | 5,000 | 5,849 | 55,940 |
| V_min_delta, p* | 0, tick 13 | 0, tick 44 | 0, tick 427 |
| KZG commitments (witness + quotient) | 23 | 23 | 23 |
| Main proof size (constant, see note) | ~2.78 KB | ~2.78 KB | ~2.78 KB |
| Core cryptographic proof time (L2-L3h, prove+verify) | 45.9 ms | 54.0 ms | 141.1 ms |
| Full pipeline time (incl. prototype sanity layers) | 204.3 ms | 347.5 ms | 2,305.1 ms |
| `batch_check` (4 pairings) | 3.10 ms | 2.91 ms | 2.96 ms |
| ALL PASS | YES | YES | YES |

"Core cryptographic proof time" is Layers 2 through 3h added together:
interpolation, KZG setup, witness/quotient commitments, Fiat-Shamir, and
GWC19 opening prove+verify -- the part of the pipeline that's fully
committed, quotient-checked, and pairing-verified end to end. It
excludes Layer 4 (a redundant prover-side re-check of the same
identities, kept for auditability while developing this prototype, not
needed by a real verifier) and Layer 4b (bit-gadget construction, which
is legitimate work but isn't wired into a verifiable opening yet -- see
Limitations). "Full pipeline time" is everything the binary currently
runs, redundant checks and unfinished wiring included; see
`RATIONALE_AND_RESULTS.md` for a full breakdown of where that time goes
and a comparison against the Noir/UltraHonk numbers.

Main proof size is derived from element counts, not measured
byte-for-byte: 23 base commitments (9 witness + 14 quotient) plus 4
opening groups (a 23-polynomial batch at `zeta`, a 3-polynomial batch at
`omega*zeta`, and two single-polynomial cliff-value opens), using the
standard 32-byte compressed size for both BN254 G1 elements and scalar
field elements. Because a KZG commitment is a single group element
regardless of the committed polynomial's degree, this figure doesn't
change with `n` -- same reason UltraHonk's proof size is constant on the
Noir side, just arrived at differently (many small proof objects here
vs. one monolithic proof there).

A negative-control test in `main.rs` (Layer 3g) proves a range proof for
`V_max + 1` and checks it against the real `V_max`, confirming it
correctly fails `verify_range16_bound` -- demonstrating the binding
check is load-bearing, not vacuous.

## Limitations

The bit-decomposition gadget instances (`#1`, `#2`, `#8`, `#15`, `#16`,
`#23`) are committed via `commit_bit_gadget` in `full_pipeline` and
`main.rs`, but the resulting commitments are discarded right after
(`let _ = commit_bit_gadget(...)`) rather than being opened and
pairing-verified the way the main 14 quotients are in Layer 3f. What
actually determines `bit_gadgets_ok` today is `recon_ok`/`bool_ok`,
computed inside `build_bit_gadget` by checking that a remainder
polynomial is zero *in the clear*, on the prover's own plaintext copy of
the polynomials -- the same category of check as Layer 4, not an
independent, verifier-checkable proof. In other words, as currently
wired, these six constraint families are self-checked by the prover but
not yet cryptographically bound for a verifier. Folding them into the
same Fiat-Shamir transcript and GWC19 batch-opening proof that Layer
3e/3f already runs for the other 14 constraints is the natural fix, and
is flagged as follow-up work rather than done here.

Layer 4 (`verify_all`) is a pure prover-side sanity duplicate of
identities already proved for real elsewhere in the pipeline. A
production prover would skip it; it's kept here for development-time
auditability (every `#N` gets its own PASS/FAIL line in `main.rs`).
