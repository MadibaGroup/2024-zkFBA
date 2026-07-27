# ZK-FBA full-protocol circuit: design rationale, code guide, results

This document covers the Noir
circuits implementing all 33 constraints from (Section 14's Full Constraint List) for the zero-knowledge frequent batch
auction (FBA) protocol. It explains *why* the circuit is built the way it
is and *what* each part of `protocol_main_template.nr` does, plus the
benchmark results.

## What the circuit proves

Given private per-tick bid/ask volumes `B(X)`/`A(X)` over `N` price ticks,
the circuit proves that a disclosed clearing receipt (`V_max`,
`V_min_delta`, `c`, `d`, `p*`) is the correct output of the FBA clearing
algorithm, without revealing any individual bid or ask value. `N` is
fixed per package (100 or 1000) via a Noir `global`, since Noir array
sizes must be compile-time constants; `protocol_main_template.nr` is the
shared source generated into both packages with `sed 's/__N_VALUE__/N/'`.

## Guide to the code

Each block below corresponds to a labeled section in
`protocol_main_template.nr`.

**Section 3: bid/ask range check (constraints #1, #2).**
Asserts every raw `b[i]`/`a[i]` is below `N_MAX`, an order-size ceiling.
The doc's Section 3 proposes a Plookup-style table lookup for this; here
it's just a `u64` comparison because Noir's `u64` is already
range-checked natively, so there's nothing left for a lookup gadget to
add.

**Section 4: accumulator init + transition (#3-#6).**
`acc_b` (Bid Depth) is a *backward* running sum: it starts at `b[N-1]`
and accumulates leftward, because demand depth at tick `i` is "how much
would buy at price >= i". `acc_a` (Ask Depth) is a *forward* running sum
starting at `a[0]`, since supply depth is "how much would sell at price
<= i". The `assert(acc_b[i] == acc_b[i+1] + b[i])` / `assert(acc_a[i] ==
acc_a[i-1] + a[i])` lines are the recurrence; Noir's checked `u64`
addition means these can't silently overflow.

**Section 5: Min(X) mutual exclusivity + ceiling (#7, #8).**
`min_x[i]` must equal `min(acc_a[i], acc_b[i])`. Rather than a `min()`
primitive, this is expressed as: `min_x[i]` is `<=` both accumulators
(so it's a valid lower bound), and the *difference* on at least one side
must be zero (`da * db == 0`, mutual exclusivity, it can't be strictly
less than *both*). This is the standard "prove `min` via a product-is-
zero trick" pattern, and it's exact because `da`/`db` are cast to `Field`
so the multiplication has no ambiguity. `min_x[i] <= v_max` is the
ceiling: no tick's executable volume can exceed the global max.

**Section 6/7: plateau endpoints (#9-#14).**
The doc's `Mask_P` (which ticks are in the plateau) and `InMCV` (is this
tick at the max) are witnessed polynomials in the general PLONK
construction. Here they are *not* witnessed at all, they're computed
in-circuit from the disclosed `c`/`d` (e.g. `i >= c && i <= d`). The only
constraints that survive are the two endpoint checks: `min_x[c] ==
v_max` and `min_x[d] == v_max`, i.e. the plateau's first and last ticks
actually hit the max. See "Why masks are computed, not witnessed" below
for the privacy argument that makes this safe.

**Section 8/9: surplus + Delta (#15-#17).**
`surp_b[i] + min_x[i] == acc_b[i]` and `surp_a[i] + min_x[i] ==
acc_a[i]` define surplus as depth minus executable volume. Writing them
as additions (rather than `surp_b[i] = acc_b[i] - min_x[i]`) means Noir's
checked-subtraction-via-addition enforces non-negativity as a side
effect, this is exactly the property the doc's bit-decomposition
gadget (Section 2) exists to prove in a raw prime field, so no separate
non-negativity gadget is needed. `delta[i] == surp_a[i] + surp_b[i]` is
the imbalance at that tick.

**Section 10/11: valley pin (#18-#24).**
`ChkD` and `Mask_V` (which tick is the valley/clearing price) are, like
`Mask_P`, computed rather than witnessed: `p_in_plateau` checks `p*` is
inside `[c, d]` (#22, "Mask_V containment"). The loop enforces the
"Delta floor" (#23), every tick inside the plateau must have `delta[i]
>= v_min_delta`, i.e. `v_min_delta` really is the minimum imbalance in
the plateau. The final `assert(delta[p_star] == v_min_delta)` (#24, the
"valley pin") ties the disclosed clearing tick `p*` to that minimum.

**Section 12: SD membership (#25, #26).**
`sd_p * (sd_p - v_min_delta) == 0` asserts `delta[p_star]` is either 0 or
exactly `v_min_delta`, a boolean-style membership check via the same
product-is-zero trick as Section 5. This is mathematically implied by
the valley pin directly above it (which already forces `delta[p_star] ==
v_min_delta` exactly), so this line is redundant in this construction.
It's kept anyway for one-to-one traceability against the doc's numbered
constraint list, in case a reviewer is checking constraints off by
number.

**Section 13: cliff exhaustiveness (#27-#32).**
The doc requires proving the plateau doesn't extend further, i.e. the
ticks just outside `[c, d]` (the "cliffs" at `c-1` and `d+1`) are
strictly below `v_max`, encoded as `slack + 1 + min_x[cliff] == v_max`
(so `slack >= 0` iff `min_x[cliff] < v_max`). This only has meaning if a
cliff tick actually exists inside the domain, if `c == 0` there is no
`c-1` tick, and if `d == N-1` there is no `d+1` tick. `has_left_cliff` /
`has_right_cliff` guard against computing `c - 1` when `c == 0`, which
would otherwise underflow a `u32` and panic witness generation
unconditionally (verified empirically: Noir does not eagerly evaluate an
untaken `if` branch, so the guard is sufficient and correct).

## Why masks are computed, not witnessed

`Mask_P`/`Mask_V`/`Mask_C` are derived in-circuit from the disclosed
public scalars `c`, `d`, `p_star` instead of being separate witnessed
polynomials proved correct via a shuffle/permutation argument. This is
the doc's own Section 6 "cleanest design" footnote.

The reason this doesn't leak anything: the clearing receipt (Section 1)
already discloses `V_max`, `V_min_delta`, `c`, `d`, `p*` regardless of
which masking approach is used. A shuffle argument would still need to
open these same scalars at the end, it would just spend extra
constraints proving that a separately-witnessed 0/1 polynomial is
*consistent* with `c`/`d`/`p*`, without changing what's disclosed.
Individual `B(X)`/`A(X)` values are never opened either way. So the
computed-mask approach collapses constraints #9, #10, #13, #14, #18,
#20-#22, #27, #28 to hold by construction, at no privacy cost.

## Why no manual bit-decomposition or Plookup gadgets

Noir's `u64` type is natively range-checked and its arithmetic is
checked (underflow fails witness generation), which is exactly what the
doc's Sections 2/3 gadgets exist to simulate in a raw prime field. Every
constraint that cites those gadgets (#8, #15, #16, #23, #31, #32, and
#33's booleanity) is enforced via a direct `u64` equation/comparison
instead of a manual bit-decomposition or lookup-table circuit.

## Design decisions on witness data

CSV columns are never trusted as circuit input. Only `Bids++` and
`Asks++` are read from the CSV as ground truth. Every other column
(`AccB`, `AccA`, `Min`, `SurpB`, `SurpA`, `Delta`, `Slack_L`, `Slack_R`,
`V_max`, `V_min_delta`, `c`, `d`, `p*`) is recomputed from scratch by
`protocol_prover_gen.py` and independently cross-checked against the
CSV's own precomputed columns (`Bid_Depth`, `Ask_Depth`, `Min_Bid_Ask`,
`Bid_Surplus`, `Ask_Surplus`, `Abs_Delta`, `MCV`) purely as a sanity
guard, the circuit itself re-derives and constrains all of it, so a
tampered CSV column cannot silently pass through as witness data.

## Regenerating witness data

```
python3 protocol_prover_gen.py
```

Reads both CSVs, recomputes every derived column from raw `Bids++`/
`Asks++`, cross-checks against the CSV's own columns, and overwrites
`fba_protocol_100/Prover.toml` / `fba_protocol_1000/Prover.toml`.

`protocol_main_template.nr` is the single source of truth for the
circuit logic, both packages' `src/main.nr` are generated from it via
`sed 's/__N_VALUE__/100|1000/'`. Edit the template and regenerate rather
than editing the two `main.nr` files independently, to avoid drift.

## Results (2026-07-23, Apple M-series, nargo 1.0.0-beta.21 / bb 5.0.0-nightly.20260505)

| | fba_protocol_100 | fba_protocol_1000 |
|---|---|---|
| n (price ticks) | 100 | 1000 |
| ACIR opcodes | 6,326 | 63,026 |
| Circuit size (gates) | 13,736 | 111,611 |
| Plateau `[c, d]` | [44, 45] | [427, 429] |
| $V_{max}$ | 5,849 | 55,940 |
| $V_{min\Delta}$, $p^*$ | 0, tick 44 | 0, tick 427 |
| Proof size | 14,656 bytes | 14,656 bytes |
| `bb prove` wall time | 0.13 s | 0.45 s |
| `bb verify` | PASS (0.02 s) | PASS (0.01 s) |

Both derivations were independently cross-checked against the CSVs' own
precomputed columns before compiling (`python3 protocol_prover_gen.py`
prints `cross-check OK`). A negative-control test (incrementing `v_max`
by 1 in `fba_protocol_100/Prover.toml`) correctly fails at the plateau
endpoint constraint (`assert(min_x[c] == v_max)`), confirming the
constraints are load-bearing rather than vacuous.

Proof size is backend-fixed (UltraHonk) regardless of circuit size, which
is why both rows show 14,656 bytes despite the 10x gate-count difference, consistent with UltraHonk's constant verifier cost.
