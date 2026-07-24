# Full-protocol circuits (fba_protocol_100 / fba_protocol_1000)

These two new workspace packages implement **all 33 constraints** from
`protocol_constraints.md` (Section 14's Full Constraint List), using CSV
price-tick data (`~/Downloads/order_book_100_log-normal.csv` and
`order_book_1000_log-normal.csv`) instead of the hardcoded 21-tick book.

They are **separate from, and do not modify**, `zk_fba_full/`,
`acc_circuit/`, `mcv_circuit/`, or any of `README.md` / `NOIR_VS_RUST.md` /
`ABSTRACT_AFT2026.md` -- those back the AFT2026 submission's reduced
5-constraint (C1-C5 + MCV) benchmark numbers and were left untouched.

## Notation mapping: old (Rust/reduced-Noir) -> protocol_constraints.md

| Old name (`zk_fba` Rust, `zk_fba_full` Noir) | New name (this circuit) | Doc symbol |
|---|---|---|
| `bid` | `b` | $B(X)$ -- Bids++ |
| `ask` | `a` | $A(X)$ -- Asks++ |
| `acc_b` | `acc_b` (unchanged) | $AccB(X)$ -- Bid Depth |
| `acc_a` | `acc_a` (unchanged) | $AccA(X)$ -- Ask Depth |
| `min_arr` | `min_x` | $Min(X)$ |
| `mcv` | `v_max` | $V_{max}$ |
| C1 (`V_AccA_init`) | Section 4, constraint #4 | Supply init |
| C2 (`V_AccA_rec`) | Section 4, constraint #6 | Supply transition |
| C3 (`V_AccB_init`) | Section 4, constraint #3 | Demand init |
| C4 (`V_AccB_rec`) | Section 4, constraint #5 | Demand transition |
| C5 (`V_KL`) | Section 5, constraint #7 | Min mutual exclusivity |
| *(none -- new)* | `surp_b`, `surp_a` | $SurpB(X)$, $SurpA(X)$ |
| *(none -- new)* | `delta` | $\Delta(X)$ |
| *(none -- new)* | `v_min_delta`, `c`, `d`, `p_star` | $V_{min\Delta}$, $c$, $d$, $p^*$ |
| *(none -- new)* | `slack_l`, `slack_r` | $Slack_L(X)$, $Slack_R(X)$ |

`Mask_P`, `InMCV`, `ChkD`, `Mask_V`, `SD`, `Mask_C` have **no witness
variables** in this circuit -- see design decision below.

## Design decisions

1. **Masks are computed, not witnessed.** `Mask_P`/`Mask_V`/`Mask_C` are
   derived in-circuit from the disclosed public scalars `c`, `d`, `p_star`
   (e.g. `mask_p[i] = (i >= c) & (i <= d)`), per the doc's own Section 6
   "cleanest design" footnote. Since the clearing receipt (Section 1)
   already discloses `V_max`, `V_min_delta`, `c`, `d`, `p*` regardless,
   this reveals nothing beyond what a shuffle/permutation argument would
   also require disclosing -- individual `B(X)`/`A(X)` values are never
   opened either way. This collapses constraints #9, #10, #13, #14, #18,
   #20-#22, #27, #28 to hold by construction.
2. **No manual bit-decomposition or Plookup gadgets.** Noir's `u64` type
   is natively range-checked and its arithmetic is checked (underflow
   fails witness generation), which is exactly what Sections 2/3's
   gadgets exist to simulate in a raw prime field. Every constraint that
   cites those gadgets (#8, #15, #16, #23, #31, #32, and #33's
   booleanity) is enforced via a direct `u64` equation/comparison instead.
   Full rationale is in the header comment of each `main.nr`.
3. **CSV columns are never trusted as circuit input.** Only `Bids++` and
   `Asks++` are read from the CSV as ground truth. Every other column
   (`AccB`, `AccA`, `Min`, `SurpB`, `SurpA`, `Delta`, `Slack_L`, `Slack_R`,
   `V_max`, `V_min_delta`, `c`, `d`, `p*`) is recomputed from scratch by
   `protocol_prover_gen.py` and independently cross-checked against the
   CSV's own precomputed columns (`Bid_Depth`, `Ask_Depth`, `Min_Bid_Ask`,
   `Bid_Surplus`, `Ask_Surplus`, `Abs_Delta`, `MCV`) purely as a sanity
   guard -- the circuit itself re-derives and constrains all of it.

## Regenerating witness data

```
python3 protocol_prover_gen.py
```

Reads both CSVs, recomputes every derived column from raw `Bids++`/`Asks++`,
cross-checks against the CSV's own columns, and overwrites
`fba_protocol_100/Prover.toml` / `fba_protocol_1000/Prover.toml`.

`protocol_main_template.nr` is the single source of truth for the circuit
logic -- both packages' `src/main.nr` are generated from it via
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
prints `cross-check OK`). A negative-control test (incrementing `v_max` by
1 in `fba_protocol_100/Prover.toml`) correctly fails at the plateau
endpoint constraint (`main.nr:112`, `assert(min_x[c] == v_max)`),
confirming the constraints are load-bearing rather than vacuous.

Proof size is backend-fixed (UltraHonk) regardless of circuit size, which
is why both rows show 14,656 bytes despite the 10x gate-count difference
-- consistent with the AFT2026 abstract's framing of UltraHonk's constant
verifier cost.
