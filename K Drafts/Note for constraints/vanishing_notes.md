

## 1. Accumulator Polynomials — `AccA(X)`, `AccB(X)`

### 1.1 Initialization constraints have their target points swapped

The factor $Z_H(X)/(X-\omega^j)$ vanishes at **every** domain point *except* $\omega^j$ — it is PLONKbook's "Zero all but first/last" pattern (`zero1`: $P(X)\cdot Z_H(X)/(X-\omega^0)$ forces equality only at $\omega^0$; $P(X)\cdot Z_H(X)/(X-\omega^{n-1})$ forces it only at $\omega^{n-1}$). So as written, the *Supply* equation only constrains index $n-1$ (the **highest** tick), and the *Demand* equation only constrains index $0$ (the **lowest** tick), exactly backwards from what the prose says, and from what the data requires: in both your example tables, `AskDepth` at price-index 0 equals the ask volume at that tick (price 0, Ask=0, AskDepth=0), and `BidDepth` at the *highest* tick equals the bid volume there.

This same swap (`VAccA,I` uses $\omega^{n-1}$, `VAccB,I` uses $\omega^0$), it's been carried forward consistently.

**Fix:**
$$V_{AccA,init}(X) := (AccA(X) - Ask(X)) \cdot \frac{Z_H(X)}{X-\omega^{0}} = 0 \qquad \text{(supply starts at lowest tick)}$$
$$V_{AccB,init}(X) := (AccB(X) - Bid(X)) \cdot \frac{Z_H(X)}{X-\omega^{n-1}} = 0 \qquad \text{(demand starts at highest tick)}$$

### 1.2 Recursive-sum equations have sign/indexing bugs

correction in form:
$$V_{AccA}(X) := (X-\omega^{n-1})\cdot\big[AccA(\omega X) - AccA(X) - Ask(X)\big] = 0$$
$$V_{AccB}(X) := (X-\omega^{n-1})\cdot\big[AccB(X) - AccB(\omega X) - Bid(X)\big] = 0$$
This matches PLONKbook's `rotate` gadget pattern directly ($\mathsf{Poly}_{Arr'}(X) = \mathsf{Poly}_{Arr}(X\omega)$, no need to invoke $\omega^{-1}$), and the disabling factor $(X-\omega^{n-1})$ correctly turns the constraint off only at the wrap-around point, per `zero1`.

Introducing $\omega^{-1}$ and ends up with a sign error:
$$V_{Acc\_A,2}(X) = \big(AccA(X) - Ask(X) + AccA(\omega X)\big)\cdot(X-\omega^{n-1}) = 0$$
Expanding: $AccA(\omega X) = Ask(X) - AccA(X)$, i.e. the supply accumulator would *alternate sign* every tick instead of accumulating. The `AccB` version has the same problem, plus it evaluates `ArrB` at the wrong index. Neither reproduces the data (e.g. `AccA` going $0\to20\to50\to100$ as Ask volumes $0,20,30,50$ are added — a strictly additive recurrence, never alternating).

**Fix:** drop the $\omega^{-1}$ re-indexing.

---

## 2. Minimum Selection, `Min(X)`

### 2.1 "Mutual Exclusivity" alone does not prove a minimum

$$V_{KL}(X) = (AccA(X)-Min(X))\cdot(AccB(X)-Min(X)) = 0$$

This only proves $Min(X)$ equals **one** of the two sides, it says nothing about which one, or whether it's the *smaller* one. A dishonest auctioneer could set $Min(X)=AccA(X)$ at a tick where $AccB(X) < AccA(X)$: the product is still zero, the proof still passes, and the reported trade volume is now *larger than the demand actually available*, precisely the "inventing shares" attack the documents say this constraint is meant to prevent.

This exact uncertainty without resolving it (*"checked in our vanishing polynomial OR is it possible for it to even become a negative number?"* / *"If we correctly prove that Constraint B holds, we have indirectly proven that Constraint A is also true"*, this second claim is the gap; B does **not** imply A). This is the most important fix in the whole protocol, since everything downstream (ceiling, plateau, cliffs, tie-break) is computed from `Min(X)`.

**Fix: adding explicit non-negativity to both sides**, reusing the same bit-decomposition gadget already used for `Vceiling` (§3):
$$K(X) := AccA(X) - Min(X), \qquad L(X) := AccB(X) - Min(X)$$
$$V_{K,nonneg}(X) := K(X) - \sum_{j=0}^{k-1} 2^j B^K_j(X) = 0, \qquad B^K_j(X)\cdot(B^K_j(X)-1)=0$$
$$V_{L,nonneg}(X) := L(X) - \sum_{j=0}^{k-1} 2^j B^L_j(X) = 0, \qquad B^L_j(X)\cdot(B^L_j(X)-1)=0$$
Together with $V_{KL}$, these three constraints fully pin down $Min(X)=\min(AccA(X),AccB(X))$: $K,L\ge 0$ rules out picking the *larger* side, and $K\cdot L=0$ rules out picking something *smaller than both*.

($K(X)$ and $L(X)$ are exactly the `Bid Surplus`/`Ask Surplus` columns in example tables)

---

## 3. Global Maximum Ceiling: one dependency to note

$$V_{ceiling}(X) = Mask_{plateau}(X)\cdot\Big((V_{max}-Min(X)) - \sum_{j=0}^{k-1}2^j B_j(X)\Big) = 0$$

The bit-decomposition logic is sound and matches PLONKbook's `range` gadget reasoning directly. One note: this should *not* be gated by `Mask_plateau(X)`, the ceiling has to hold for **every** tick in the whole book, not just inside the candidate plateau (otherwise nothing stops volume from exceeding $V_{max}$ outside $[c,d]$). Drop the mask factor here; it belongs on the plateau-pinning equations, not the ceiling.

---

## 4. Plateau Boundaries (Cliffs)

### 4.1 Endpoint-only plateau pinning: valid, but state the dependency explicitly

Only *because* `Min(X)` is provably unimodal: since `Ask(X)` is range-checked non-negative and `AccA` only ever adds it, `AccA` is forced non-decreasing; symmetrically `AccB` is forced non-increasing. The min of a non-decreasing and a non-increasing curve is single-peaked, so endpoints at $V_{max}$ plus a global ceiling (§3) forces the entire interior to also equal $V_{max}$, no dip is possible.

A reviewer who doesn't reconstruct the unimodality argument will read the endpoint-only check as an unjustified shortcut rather than the (correct) consequence of §1's accumulator monotonicity.

### 4.2 🆕 Missing: explicit decomposition equations for the cliff slacks

The cliff equations themselves are fine:
$$V_{cliff,L}(X) = (V_{max}-Min(X)-1-Slack_L(X))\cdot Mask_{c-1}(X) = 0$$
$$V_{cliff,R}(X) = (V_{max}-Min(X)-1-Slack_R(X))\cdot Mask_{d+1}(X) = 0$$
"Since Slack must be ≥0 (enforced by bit-checks)", and that bit-check is never actually written down for `Slack_L`/`Slack_R` specifically; only the generic Booleanity constraint ($B_j\cdot(B_j-1)=0$) is listed in the Foundational Integrity section, with no equation connecting it to these two particular slack variables. Without the *decomposition* equation (not just Booleanity of generic bits), a malicious prover could set $Slack_L$ to a field element that "wraps around" to look non-negative — defeating the entire cliff argument.

**Fix — (mirroring §3's pattern):**
$$Slack_L(X) - \sum_{j=0}^{k-1}2^j B^{Slack_L}_j(X) = 0, \qquad Slack_R(X) - \sum_{j=0}^{k-1}2^j B^{Slack_R}_j(X) = 0$$
each paired with Booleanity on $B^{Slack_L}_j$, $B^{Slack_R}_j$.

---

## 5. Surplus and Tie-Breaking
### 5.1 Naming collision: two different things are both called "Surplus"

- 'surplusB = DepthB − Min', 'surplusA = DepthA − Min' — these are the **leftover volume on each side at the clearing price**, used for Phase-3 pro-rata rationing of the long side. These are exactly $K(X), L(X)$ from §2.1 above.
- `Surplus(X) := AccA(X) − AccB(X)` — the **net imbalance between the two sides**, used for the Phase-2 tie-break.

These are conceptually different quantities reused under the same name across documents. Recommend renaming for clarity: keep **`SurpA(X)`/`SurpB(X)`** exclusively for the pro-rata leftover quantities ($K$/$L$), and use **`Delta(X)`** exclusively for the tie-break imbalance — which also matches the literal `Delta (Δ)` column header already used in your own example table.

### 5.2 Sign convention

The *sign* of the imbalance is what Phase 3 uses to decide which side is short and which is long (and therefore which side's IO orders are eligible). If this feeds into IO-eligibility logic downstream, the flipped sign would route IO orders to the wrong side. ($Delta(X) := AccB(X) - AccA(X)$).

### 5.3 The signed imbalance can't be bit-decomposed directly — and you've already half-solved this without naming it

Both `Vflr` and the "Valley Proof" (`Vsurp`) bit-decompose `Surplus(X)` directly. But `Surplus(X) = AccA(X)-AccB(X)` is a **signed** quantity — within the plateau it's typically positive on one side of the true clearing price and negative on the other (it crosses zero exactly where the tie-break should land). Bit-decomposition only proves a value is non-negative and bounded; applying it directly to a value that's legitimately negative at some valid points will make the proof **fail at those points even though the auctioneer is being honest**, the same modular-wraparound problem the "Half-Field Equator" logic was built to avoid for the raw order columns, just re-introduced here for the tie-break column.

The fix: In both example tables, the column `Delta` is **not** `AccA − AccB`, it's `Bid Surplus + Ask Surplus`. Checking the numbers (`Plateau necessity...md`, price 90: $SurpB=300, SurpA=0$, Delta listed = $300$; price 100: $SurpB=0,SurpA=700$, Delta listed = $700$) confirms:
$$Delta(X) = SurpB(X) + SurpA(X) = K(X)+L(X) = |AccA(X)-AccB(X)|$$
This is automatically non-negative — because exactly one of $K,L$ is zero at every tick (proven already by $V_{KL}$ in §2) and the other equals the true absolute gap — with **no extra sign-bit or range-check machinery needed beyond what §2.1 already adds.**

**Fix:**
$$V_{Delta,def}(X) := Delta(X) - \big(SurpA(X)+SurpB(X)\big) = 0$$
$$V_{flr}(X) := Mask_{plateau}(X)\cdot\Big((Delta(X)-V_{min\Delta}) - \sum_{j=0}^{k-1}2^j B^{flr}_j(X)\Big) = 0,\quad B^{flr}_j(1-B^{flr}_j)=0$$
$$V_{valley}(X) := (Delta(X)-V_{min\Delta})\cdot Mask_{p^*}(X) = 0$$
($Mask_{p^*}(X) = Z_H(X)/(X-\omega^{p^*})$, the single-point mask, pinning the minimum to the disclosed clearing price specifically — replacing the document's `Vsurp` equation, which as written defines a slack array over the whole plateau rather than pinning a single winning point.)

---

## 6. Range / Lookup Checks: missing end-boundary condition

The Plookup-style equations for the raw Bid/Ask "equator" range check (and any other lookup-based range check used elsewhere) only give:
$$L_1(X)(Z_{in}(X)-1) = 0 \qquad \text{(start)}$$
plus the transition equation. PLONKbook's own Plookup specification (/docs/gadgets/lookup2) requires **both** boundary conditions on the grand product:
$$\mathsf{Poly}_Z(\omega^0) = \mathsf{Poly}_Z(\omega^{\kappa-1}) = 1$$
enforced together via $[\mathsf{Poly}_Z(X)-1]\cdot Z_H(X)/\big((X-\omega^0)(X-\omega^{\kappa-1})\big) = 0$. Without the end condition, the grand product is never checked to actually complete a full, consistent pass through the sorted multiset — a prover could get the start right and still cheat partway through. It should be promoted from comment to constraint:

**Fix: add, for every Plookup-style check in the protocol (raw-input equator check, and the surplus-membership lookup if we keep that variant instead of the bit-decomposition version in §5.3):**
$$L_n(X)\cdot(Z(X) - 1) = 0$$
(target value is **1**, matching the start condition, not a generic "Target", which PLONKbook leaves undefined only because the basic, non-halo2 Plookup variant always closes back to 1.)

---

## 7. Mask Polynomials: a design inconsistency to resolve

Treating masks ($Mask_{[c,d]}$, $Mask_i = Z_H(X)/(X-\omega^i)$) as **public, verifier-computable helpers** built directly from disclosed indices ($c,d,c-1,d+1,p^*$), no commitment, no extra proof of well-formedness needed, since the verifier can just compute them. Might require additional well-formedness constraints (that no document currently provides) proving each mask is genuinely a 0/1 indicator over the right interval.

**????:** keep masks public: it's strictly cheaper (no commitment, no extra constraint) and the values $c,d$ are opened in plaintext elsewhere in the protocol anyway, so nothing is lost (Fix Table 2 by removing Mask entries from the list of committed polynomials.).

---

## 8. Consolidated Corrected Constraint Set

**Accumulators**
$$(AccA(X)-Ask(X))\cdot\tfrac{Z_H(X)}{X-\omega^0}=0 \qquad (AccB(X)-Bid(X))\cdot\tfrac{Z_H(X)}{X-\omega^{n-1}}=0$$
$$(X-\omega^{n-1})[AccA(\omega X)-AccA(X)-Ask(X)]=0 \qquad (X-\omega^{n-1})[AccB(X)-AccB(\omega X)-Bid(X)]=0$$

**Minimum**
$$(AccA(X)-Min(X))(AccB(X)-Min(X))=0$$
$$K(X)-\textstyle\sum 2^jB^K_j(X)=0,\ K(X)=AccA(X)-Min(X)\qquad L(X)-\textstyle\sum 2^jB^L_j(X)=0,\ L(X)=AccB(X)-Min(X)$$

**Ceiling (whole book, no plateau mask)**
$$(V_{max}-Min(X))-\textstyle\sum 2^jB_j(X)=0$$

**Plateau / cliffs**
$$(Min(X)-V_{max})\cdot Mask_c(X)=0 \qquad (Min(X)-V_{max})\cdot Mask_d(X)=0$$
$$(V_{max}-Min(X)-1-Slack_L(X))\cdot Mask_{c-1}(X)=0 \qquad Slack_L(X)-\textstyle\sum 2^jB^{Slack_L}_j(X)=0$$
$$(V_{max}-Min(X)-1-Slack_R(X))\cdot Mask_{d+1}(X)=0 \qquad Slack_R(X)-\textstyle\sum 2^jB^{Slack_R}_j(X)=0$$

**Tie-break**
$$Delta(X) := AccB(X)-AccA(X)\ \text{(signed; for sign/long-short logic only)} \qquad Delta_{abs}(X) - (K(X)+L(X)) = 0$$
$$Mask_{plateau}(X)\cdot\big((Delta_{abs}(X)-V_{min\Delta})-\textstyle\sum 2^jB^{flr}_j(X)\big)=0 \qquad (Delta_{abs}(X)-V_{min\Delta})\cdot Mask_{p^*}(X)=0$$

**Booleanity (every $B_j$ introduced above, including the four new families):**
$$B_j(X)\cdot(B_j(X)-1)=0$$

**Lookup boundary (every Plookup-style check, e.g. the raw Bid/Ask equator range check):**
$$L_1(X)(Z(X)-1)=0 \qquad L_n(X)(Z(X)-1)=0 \qquad \text{(+ transition equation)}$$

---

## 9. Open items?

- **Bit-width $k$ for the new decomposition equations** (§2.1, §4.2, §5.3) — should match whatever $k$ already fixed for `Vceiling`, since all are bounding quantities of the same order of magnitude (order sizes / depths).
- **Range-check style consistency**: the protocol currently mixes bit-decomposition (ceiling/floor) with Plookup-table lookups (raw-input equator check). Both are individually sound (validated against PLONKbook's `range` and `lookup2` gadgets respectively): whether to standardize on one for the whole protocol is a cost/clarity tradeoff, not a bug.
- restricting the tie-break search to $p\in[c,d]$ only; I'd treat the published paper as the authoritative version of this rule.
