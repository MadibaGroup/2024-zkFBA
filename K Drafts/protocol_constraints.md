# Constraints

---

## A Note on Column Order

Here we follow the left-to-right column order of our dummy example, easier for editing purposes. The trade-off is that some constraints on a given column reference polynomials that are formally introduced later, specifically the non-negativity proofs that complete the $Min(X)$ correctness argument appear in the Bid/Ask Surplus sections rather than next to $Min(X)$ itself. Those forward references are flagged inline.

---

## 1. Notation and Setting

The evaluation domain is $H = \{1, \omega, \omega^2, \ldots, \omega^{n-1}\}$ where $\omega$ is a primitive $n$-th root of unity and each point $\omega^i$ corresponds to price tick $i$. The vanishing polynomial $Z_H(X) = X^n - 1$ is zero at every point of $H$ and nowhere else.

A vanishing equation $V(X) = 0$ means $V(X)$ is divisible by $Z_H(X)$, so the constraint holds at every price tick simultaneously. The prover produces a quotient polynomial $Q(X) = V(X)/Z_H(X)$ and commits to it; the verifier checks $V(\zeta) = Q(\zeta) \cdot Z_H(\zeta)$ at a random challenge $\zeta \notin H$.

Commitments are written $\mathsf{cm}[P]$ for the KZG commitment to polynomial $P(X)$. Public scalars and verifier-computable polynomials (Lagrange masks) carry no commitment.

The public scalars disclosed as part of the clearing receipt are:

- $V_{max}$: global maximum executable volume (the plateau height)
- $V_{min\Delta}$: minimum absolute imbalance inside the plateau (the valley depth)
- $c, d$: first and last tick indices of the plateau
- $p^*$: the clearing price tick index

**Column array-to-polynomial-to-commitment table:**

| Spreadsheet column | Role | Polynomial | Commitment |
|---|---|---|---|
| Bids++ | Raw bid volume at each tick | $B(X)$ | $\mathsf{cm}[B]$ |
| Asks++ | Raw ask volume at each tick | $A(X)$ | $\mathsf{cm}[A]$ |
| Bid Depth | Cumulative demand, high to low | $AccB(X)$ | $\mathsf{cm}[AccB]$ |
| Ask Depth | Cumulative supply, low to high | $AccA(X)$ | $\mathsf{cm}[AccA]$ |
| Min(Bid,Ask) | Executable volume at each tick | $Min(X)$ | $\mathsf{cm}[Min]$ |
| Selector (plateau) | 1 inside $[c,d]$, 0 outside | $Mask_P(X)$ | $\mathsf{cm}[Mask_P]$ or public |
| In {0, MCV} | $V_{max}$ inside plateau, 0 outside | $InMCV(X)$ | $\mathsf{cm}[InMCV]$ |
| Bid Surplus++ | Demand leftover after matching | $SurpB(X)$ | $\mathsf{cm}[SurpB]$ |
| Ask Surplus++ | Supply leftover after matching | $SurpA(X)$ | $\mathsf{cm}[SurpA]$ |
| Abs(Delta) | Absolute bid-ask imbalance | $\Delta(X)$ | $\mathsf{cm}[\Delta]$ |
| Check on Delta | 1 where $\Delta = V_{min\Delta}$, 0 otherwise | $ChkD(X)$ | $\mathsf{cm}[ChkD]$ |
| Selector (valley) | 1 at $p^*$, 0 otherwise | $Mask_V(X)$ | $\mathsf{cm}[Mask_V]$ or public |
| S*Delta in {0, surplus} | Valley-masked imbalance | $SD(X)$ | $\mathsf{cm}[SD]$ |
| Selector (cliff) | 1 at $c-1$ and $d+1$, 0 elsewhere | $Mask_C(X)$ | $\mathsf{cm}[Mask_C]$ |
| MCV (constant column) | $V_{max}$ at every tick | public scalar | - |
| Cliff value | $Min$ opened at cliff ticks | KZG opening of $Min(X)$ | - |
| 1 (cliff point indicator) | Single-point Lagrange mask | $L_{c-1}(X)$, $L_{d+1}(X)$ | public |
| Slack++ | Slack absorbing the cliff gap | $Slack_L(X)$, $Slack_R(X)$ | $\mathsf{cm}[Slack_L]$, $\mathsf{cm}[Slack_R]$ |

Bit-decomposition witness columns $B_j(X)$ are introduced implicitly wherever a range proof is needed, each has commitment $\mathsf{cm}[B_j]$.

---

## 2. Shared Proof Gadgets

Two proof patterns appear repeatedly. They are defined once here and referenced by name throughout.

### Bit-Decomposition

To prove a committed value $P(X)$ is non-negative and at most $2^k - 1$ at every tick, the prover supplies $k$ witness columns $B_0(X), \ldots, B_{k-1}(X)$ satisfying:

$$P(X) - \sum_{j=0}^{k-1} 2^j \cdot B_j(X) = 0$$

$$B_j(X) \cdot (B_j(X) - 1) = 0 \quad \text{for each } j$$

The booleanity equations force each $B_j$ to be 0 or 1. A sum of $k$ binary-weighted bits can only represent integers in $[0, 2^k - 1]$, so the reconstruction equation forces $P(X)$ into that range. This is the only mechanism available for inequalities in a prime-order field, where "negative" values appear as large integers near $q$ and have no special representation.

This gadget is used for the surplus columns, the global max ceiling, the cliff slack, and the Delta floor.

### Plookup Range Check

To prove a private polynomial $f(X)$ takes only values from a public table $t(X)$ (e.g. valid order sizes $\{0, 1, \ldots, N_{max}\}$), the prover supplies a grand product $Z(X)$ and a sorted interleaving $s(X)$ satisfying three equations:

$$L_1(X) \cdot (Z(X) - 1) = 0$$

$$(X - \omega^{n-1}) \cdot \big[Z(\omega X)(\gamma + s(X) + \beta s(\omega X)) - Z(X)(\gamma(1+\beta) + f(X) + \beta t(X))\big] = 0$$

$$L_n(X) \cdot (Z(X) - 1) = 0$$

The first and third equations are boundary conditions: the grand product must open at 1 at both the start and the end of the domain. The middle equation is the membership check traversing every tick. Both boundary conditions are required,  without the end condition, a prover could satisfy the start and then quietly substitute values outside the table partway through without detection.

---

## 3. Bids++ and Asks++, $B(X)$ and $A(X)$

These are the private raw inputs: how many shares were bid or offered at exactly each price tick, before any accumulation.

Before building the accumulators, the protocol checks that every order volume is a legitimate non-negative integer and not a field element near $q$ wrapping around from a negative value. The Plookup range check (Section 2) is applied to both $B(X)$ and $A(X)$ with public table $t_{in}(X)$ spanning $\{0, 1, \ldots, N_{max}\}$ where $N_{max} < (q-1)/2$. This boundary ensures no valid order size can be misread as a negative number in the field.

Catching bad inputs at this stage prevents all downstream columns from inheriting corrupt values.

---

## 4. Bid Depth and Ask Depth, $AccB(X)$ and $AccA(X)$

$AccB(X)$ is the cumulative demand at each tick: the total volume willing to buy at that price or higher. It is built by summing bid volumes downward from the highest price tick.

$AccA(X)$ is the cumulative supply: the total volume willing to sell at that price or lower. It is built by summing ask volumes upward from the lowest price tick.

**Initialization.**

The demand accumulator starts at the highest tick, where cumulative demand equals the raw bid volume there:

$$\big(AccB(X) - B(X)\big) \cdot \frac{Z_H(X)}{X - \omega^{n-1}} = 0$$

The supply accumulator starts at the lowest tick:

$$\big(AccA(X) - A(X)\big) \cdot \frac{Z_H(X)}{X - \omega^0} = 0$$

The factor $Z_H(X)/(X - \omega^j)$ is zero at every domain point except $\omega^j$, so each equation pins exactly one absolute value,  the base case of the accumulator. Without this anchor, a shifted accumulator would look internally consistent (all relative increments correct) yet report the wrong totals everywhere.

**Transition.**

The demand transition enforces that moving one tick down in price, cumulative demand grows by the current bid volume:

$$(X - \omega^{n-1}) \cdot \big[AccB(X) - AccB(\omega X) - B(X)\big] = 0$$

The supply transition enforces that moving one tick up, cumulative supply grows by the current ask volume:

$$(X - \omega^{n-1}) \cdot \big[AccA(\omega X) - AccA(X) - A(X)\big] = 0$$

The factor $(X - \omega^{n-1})$ disables both equations at the last domain point, where $\omega X$ would wrap around to $\omega^0$ and create a meaningless circular relationship.

These transitions have a structural consequence used later: $AccB(X)$ is forced non-increasing as price rises and $AccA(X)$ is forced non-decreasing. Their minimum is therefore single-peaked, which is what makes the plateau endpoint-only proof in Section 7 valid.

---

## 5. Min(Bid,Ask),  $Min(X)$

$Min(X)$ is the executable volume at each tick: the smaller of cumulative supply and demand. It is the most important intermediate column,  the plateau, cliffs, surplus, and tie-break all derive from it.

**Mutual exclusivity.** $Min(X)$ must equal one of the two accumulators at every tick:

$$\big(AccA(X) - Min(X)\big) \cdot \big(AccB(X) - Min(X)\big) = 0$$

This product is zero exactly when $Min = AccA$ or $Min = AccB$. On its own it does not force the smaller side to be chosen,  it would accept $Min = AccA$ even when $AccA > AccB$. The surplus non-negativity constraints in Section 9 close that gap; see the note there.

**Global max ceiling.** The publicly disclosed $V_{max}$ must be an upper bound on $Min(X)$ everywhere in the book:

$$(V_{max} - Min(X)) - \sum_{j=0}^{k-1} 2^j B^{ceil}_j(X) = 0$$

If any tick had $Min(X) > V_{max}$, the expression $V_{max} - Min(X)$ would be negative,  a large field element near $q$,  which cannot be expressed as a sum of $k$ bits. This prevents the auctioneer from understating $V_{max}$ to exclude legitimate plateau ticks from the clearing interval.

---

## 6. Plateau Selector,  $Mask_P(X)$

$Mask_P(X)$ is 1 at every tick inside the plateau $[c, d]$ and 0 everywhere else. It gates all plateau-specific constraints, restricting them to the clearing interval.

**Booleanity.** Every entry must be exactly 0 or 1:

$$Mask_P(X) \cdot (Mask_P(X) - 1) = 0$$

**Position well-formedness.** Booleanity alone does not constrain which ticks are marked. Two methods are available for proving the 1s fall exactly at positions $c$ through $d$.

*Option A,  Permutation argument.* PLONK's copy-constraint mechanism proves that the multiset of positions where $Mask_P = 1$ is an exact rearrangement of the public set $\{c, c+1, \ldots, d\}$. A permutation $\sigma$ is hardcoded into the proving key, and a grand product argument checks consistency. This reuses standard PLONK infrastructure but requires the permutation to be fixed at setup time.

*Option B,  Shuffle argument (Habock 2022).* Using a random verifier challenge $\gamma$, two compressed products are computed:

$$\mathrm{LHS} = \prod_{i=0}^{n-1} \big[(1 - Mask_P(\omega^i)) \cdot \gamma + Mask_P(\omega^i) \cdot (\omega^i + \gamma)\big]$$

$$\mathrm{RHS} = \gamma^{n-(d-c+1)} \cdot \prod_{i=c}^{d} (\omega^i + \gamma)$$

LHS contributes a neutral factor $\gamma$ for each 0-entry and a position-encoding factor $(\omega^i + \gamma)$ for each 1-entry. RHS encodes the expected set. These are equal if and only if the 1-positions of $Mask_P$ are exactly $\{c, \ldots, d\}$. The argument is encoded as a running product column $Z(X)$ with a transition equation and two boundary conditions (same Plookup boundary pattern as Section 2). No permutation is wired into the proving key.

| Property | Permutation | Shuffle |
|---|---|---|
| Extra witness column | 1 grand product $Z(X)$ | 1 grand product $Z(X)$ |
| Proving key dependency | Yes, $\sigma$ hardcoded at setup | None |
| What it proves | Position-exact correspondence | Multiset equality |

Since $c$ and $d$ are publicly disclosed, the cleanest design keeps $Mask_P(X)$ as a verifier-computable public polynomial,  a sum of public Lagrange basis polynomials $\sum_{i=c}^{d} L_i(X)$,  requiring no commitment and no position argument. Either approach is valid if the mask is treated as a committed witness.

**Plateau endpoint pins.** Once $Mask_P$ is established, the endpoints of the plateau are pinned by confirming $Min$ equals $V_{max}$ at $\omega^c$ and $\omega^d$:

$$\big(Min(X) - V_{max}\big) \cdot L_c(X) = 0$$

$$\big(Min(X) - V_{max}\big) \cdot L_d(X) = 0$$

where $L_c(X)$ and $L_d(X)$ are the public Lagrange basis polynomials at those ticks, computed by the verifier. Checking only the two endpoints is sufficient because $Min(X)$ is provably single-peaked (see Section 4): a unimodal function whose endpoints both equal $V_{max}$ cannot dip below $V_{max}$ in between without violating the ceiling of Section 5.

---

## 7. In {0, MCV},  $InMCV(X)$

$InMCV(X)$ is $V_{max}$ inside the plateau and 0 outside. It encodes "what $Min$ must equal at plateau positions" in polynomial form so it can appear in product constraints alongside $Mask_P$.

**Inside the plateau**, $InMCV$ must equal $V_{max}$:

$$\big(InMCV(X) - V_{max}\big) \cdot Mask_P(X) = 0$$

**Outside the plateau**, $InMCV$ must equal 0:

$$InMCV(X) \cdot (1 - Mask_P(X)) = 0$$

These two together imply the membership constraint that $InMCV$ takes only values in $\{0, V_{max}\}$:

$$InMCV(X) \cdot (InMCV(X) - V_{max}) = 0$$

---

## 8. Bid Surplus++ and Ask Surplus++,  $SurpB(X)$ and $SurpA(X)$

$SurpB(X) = AccB(X) - Min(X)$ is the demand leftover,  volume that wanted to buy at this price but found no matching supply. $SurpA(X) = AccA(X) - Min(X)$ is the supply leftover.

At any tick, exactly one surplus is zero (the side that was the binding constraint on trade volume) and the other captures the excess. These columns serve two roles: they complete the proof that $Min$ is the true minimum (below), and they feed the Phase 3 pro-rata rationing calculation at $p^*$.

**Definition and non-negativity.**

$$SurpB(X) - \sum_{j=0}^{k-1} 2^j B^{sB}_j(X) = 0$$

$$SurpA(X) - \sum_{j=0}^{k-1} 2^j B^{sA}_j(X) = 0$$

Both surplus columns are bit-decomposed, proving they are non-negative at every tick.

**Completing the Min proof.** Recall from Section 5 that mutual exclusivity only proves $Min$ equals one of the two accumulators, not the smaller one. The bit-decompositions here close that gap: if the auctioneer set $Min = AccA$ at a tick where $AccA > AccB$, then $SurpB = AccB - AccA$ would be a true negative number,  an enormous field element near $q$,  which cannot be expressed as a sum of $k$ bits. The proof would fail at exactly that tick. Together, mutual exclusivity plus non-negativity on both sides uniquely forces $Min(X) = \min(AccA(X), AccB(X))$ at every tick.

---

## 9. Abs(Delta),  $\Delta(X)$

$\Delta(X)$ is the absolute imbalance at each tick: the total unmatched volume on both sides combined.

$$\Delta(X) - \big(SurpA(X) + SurpB(X)\big) = 0$$

Since exactly one of $SurpA$, $SurpB$ is zero at every tick (established by the surplus non-negativity in Section 8 combined with mutual exclusivity), their sum automatically equals $|AccA(X) - AccB(X)|$. This definition avoids the signed difference $AccA - AccB$ directly: that raw difference is a legitimate negative field element at ticks where supply exceeds demand, which would cause the floor constraint in Section 11 to reject honest data.

---

## 10. Check on Delta,  $ChkD(X)$

$ChkD(X)$ is 1 at every tick inside the plateau where $\Delta = V_{min\Delta}$ (the minimum imbalance), and 0 elsewhere. In the typical case where the minimum is achieved at multiple consecutive ticks,  a valley plateau,  this column spans all of them.

**Booleanity:**

$$ChkD(X) \cdot (ChkD(X) - 1) = 0$$

**Correctness**,  every tick marked by $ChkD$ must have $\Delta = V_{min\Delta}$:

$$\big(\Delta(X) - V_{min\Delta}\big) \cdot ChkD(X) = 0$$

**Containment**,  $ChkD$ cannot fire outside the plateau:

$$ChkD(X) \cdot (1 - Mask_P(X)) = 0$$

---

## 11. Valley Selector,  $Mask_V(X)$

$Mask_V(X)$ marks the specific clearing price $p^*$ within the valley. It is 1 at a single tick (or a contiguous range if a tertiary tie-break covers multiple ticks) and 0 everywhere else.

**Booleanity:**

$$Mask_V(X) \cdot (Mask_V(X) - 1) = 0$$

**Containment**,  the clearing price must be drawn from within the valley:

$$Mask_V(X) \cdot (1 - ChkD(X)) = 0$$

**Delta floor.** No tick inside the plateau may have a smaller imbalance than $V_{min\Delta}$:

$$Mask_P(X) \cdot \Big((\Delta(X) - V_{min\Delta}) - \sum_{j=0}^{k-1} 2^j B^{flr}_j(X)\Big) = 0$$

The plateau mask restricts this check to $[c, d]$,  imbalance outside the plateau is irrelevant to the tie-break. The bit-decomposition proves $\Delta(X) - V_{min\Delta} \geq 0$ at every plateau tick: if any tick had $\Delta < V_{min\Delta}$, the difference would be negative and un-bit-decomposable.

**Valley pin.** The floor proves nothing dips below $V_{min\Delta}$ but does not prove that value is actually reached at the claimed clearing price. This constraint completes the argument:

$$\big(\Delta(X) - V_{min\Delta}\big) \cdot L_{p^*}(X) = 0$$

where $L_{p^*}(X)$ is the public Lagrange polynomial at $\omega^{p^*}$. Together, the floor and the pin establish that $V_{min\Delta}$ is the minimum imbalance inside the plateau and that $p^*$ is a tick where it is achieved.

As with $Mask_P$, the position of the 1-entries in $Mask_V$ can be proved via a shuffle or permutation argument if $Mask_V$ is a committed witness, or it can be kept as a verifier-computable public polynomial from the disclosed $p^*$.

---

## 12. S*Delta in {0, surplus},  $SD(X)$

$SD(X)$ is the product of the valley selector and $\Delta$. It is $V_{min\Delta}$ where $Mask_V$ fires and 0 everywhere else.

**Definition:**

$$SD(X) - Mask_V(X) \cdot \Delta(X) = 0$$

**Membership**,  $SD$ takes only values in $\{0, V_{min\Delta}\}$:

$$SD(X) \cdot (SD(X) - V_{min\Delta}) = 0$$

---

## 13. Cliff Selector and Cliff Mechanism,  $Mask_C(X)$, Cliff Value, $L_{c-1}$/$L_{d+1}$, $Slack_L(X)$/$Slack_R(X)$

This group of columns,  the cliff selector, the constant MCV column, the cliff value, the single-point Lagrange indicators, and the slack,  all belong to the same argument: proving that $[c, d]$ is the entire plateau and not a sub-interval that the auctioneer chose to report.

**What these columns are:**

- $Mask_C(X)$: committed selector, 1 at the two cliff points $c-1$ and $d+1$, 0 elsewhere.
- MCV constant column: the scalar $V_{max}$ at every tick. This is a public constant; the verifier substitutes it directly and no polynomial commitment is needed.
- Cliff value column: the openings of $Min(X)$ at the cliff ticks. These are not a separately committed polynomial,  they are scalar evaluations of the already-committed $Min(X)$, revealed in plaintext and tied to $\mathsf{cm}[Min]$ via KZG evaluation proofs.
- $L_{c-1}(X)$, $L_{d+1}(X)$: public Lagrange basis polynomials at the cliff ticks. Verifier-computable from the disclosed $c$ and $d$.
- $Slack_L(X)$, $Slack_R(X)$: private witness columns that absorb the gap between the cliff volume and $V_{max} - 1$.

**Cliff selector booleanity:**

$$Mask_C(X) \cdot (Mask_C(X) - 1) = 0$$

**Containment**,  cliff ticks must be outside the plateau:

$$Mask_C(X) \cdot Mask_P(X) = 0$$

**The cliff equations.** A strict inequality $Min(\omega^{c-1}) < V_{max}$ is encoded algebraically by requiring a non-negative slack $Slack_L$ such that:

$$\big(V_{max} - Min(X) - 1 - Slack_L(X)\big) \cdot L_{c-1}(X) = 0$$

$$\big(V_{max} - Min(X) - 1 - Slack_R(X)\big) \cdot L_{d+1}(X) = 0$$

The $-1$ term enforces a strict gap: even when $Slack = 0$, the cliff volume is at most $V_{max} - 1$, not $V_{max}$. Without this, the auctioneer could report a plateau that starts one tick too late, quietly excluding a tick whose orders deserved to execute.

**Slack non-negativity.** The slack must be genuinely non-negative:

$$Slack_L(X) - \sum_{j=0}^{k-1} 2^j B^{slkL}_j(X) = 0$$

$$Slack_R(X) - \sum_{j=0}^{k-1} 2^j B^{slkR}_j(X) = 0$$

Without the bit-decomposition, a "negative" slack (a large field element near $q$) could satisfy the cliff equation even when the cliff volume actually equals $V_{max}$, defeating the argument entirely.

Slack bit-width: in the worst case $Slack = V_{max} - 1$, so $k = \lceil \log_2 V_{max} \rceil$ bits suffice,  the same width used for the ceiling in Section 5.

---

## 14. Full Constraint List

| # | Name | Equation | Active domain |
|---|---|---|---|
| 1 | Bid range check | Plookup on $B(X)$ vs $t_{in}$ | All ticks |
| 2 | Ask range check | Plookup on $A(X)$ vs $t_{in}$ | All ticks |
| 3 | Demand init | $(AccB(X) - B(X)) \cdot Z_H(X)/(X - \omega^{n-1}) = 0$ | Single point $\omega^{n-1}$ |
| 4 | Supply init | $(AccA(X) - A(X)) \cdot Z_H(X)/(X - \omega^0) = 0$ | Single point $\omega^0$ |
| 5 | Demand transition | $(X-\omega^{n-1})[AccB(X) - AccB(\omega X) - B(X)] = 0$ | All ticks except last |
| 6 | Supply transition | $(X-\omega^{n-1})[AccA(\omega X) - AccA(X) - A(X)] = 0$ | All ticks except last |
| 7 | Min mutual exclusivity | $(AccA(X)-Min(X))(AccB(X)-Min(X)) = 0$ | All ticks |
| 8 | Ceiling | $(V_{max} - Min(X)) - \sum 2^j B^{ceil}_j = 0$ | All ticks |
| 9 | $Mask_P$ booleanity | $Mask_P(X)(Mask_P(X)-1) = 0$ | All ticks |
| 10 | $Mask_P$ position | Shuffle or permutation argument | All ticks |
| 11 | Plateau left endpoint | $(Min(X) - V_{max}) \cdot L_c(X) = 0$ | Single point $\omega^c$ |
| 12 | Plateau right endpoint | $(Min(X) - V_{max}) \cdot L_d(X) = 0$ | Single point $\omega^d$ |
| 13 | $InMCV$ inside plateau | $(InMCV(X) - V_{max}) \cdot Mask_P(X) = 0$ | All ticks |
| 14 | $InMCV$ outside plateau | $InMCV(X) \cdot (1 - Mask_P(X)) = 0$ | All ticks |
| 15 | $SurpB$ non-negativity | $SurpB(X) - \sum 2^j B^{sB}_j = 0$ | All ticks |
| 16 | $SurpA$ non-negativity | $SurpA(X) - \sum 2^j B^{sA}_j = 0$ | All ticks |
| 17 | Delta definition | $\Delta(X) - (SurpA(X) + SurpB(X)) = 0$ | All ticks |
| 18 | $ChkD$ booleanity | $ChkD(X)(ChkD(X)-1) = 0$ | All ticks |
| 19 | $ChkD$ correctness | $(\Delta(X) - V_{min\Delta}) \cdot ChkD(X) = 0$ | All ticks |
| 20 | $ChkD$ containment | $ChkD(X) \cdot (1 - Mask_P(X)) = 0$ | All ticks |
| 21 | $Mask_V$ booleanity | $Mask_V(X)(Mask_V(X)-1) = 0$ | All ticks |
| 22 | $Mask_V$ containment | $Mask_V(X) \cdot (1 - ChkD(X)) = 0$ | All ticks |
| 23 | Delta floor | $Mask_P(X)[(\Delta(X) - V_{min\Delta}) - \sum 2^j B^{flr}_j] = 0$ | Plateau only |
| 24 | Valley pin | $(\Delta(X) - V_{min\Delta}) \cdot L_{p^*}(X) = 0$ | Single point $\omega^{p^*}$ |
| 25 | $SD$ definition | $SD(X) - Mask_V(X) \cdot \Delta(X) = 0$ | All ticks |
| 26 | $SD$ membership | $SD(X)(SD(X) - V_{min\Delta}) = 0$ | All ticks |
| 27 | $Mask_C$ booleanity | $Mask_C(X)(Mask_C(X)-1) = 0$ | All ticks |
| 28 | $Mask_C$ containment | $Mask_C(X) \cdot Mask_P(X) = 0$ | All ticks |
| 29 | Left cliff | $(V_{max} - Min(X) - 1 - Slack_L(X)) \cdot L_{c-1}(X) = 0$ | Single point $\omega^{c-1}$ |
| 30 | Right cliff | $(V_{max} - Min(X) - 1 - Slack_R(X)) \cdot L_{d+1}(X) = 0$ | Single point $\omega^{d+1}$ |
| 31 | $Slack_L$ non-negativity | $Slack_L(X) - \sum 2^j B^{slkL}_j = 0$ | Cliff tick only |
| 32 | $Slack_R$ non-negativity | $Slack_R(X) - \sum 2^j B^{slkR}_j = 0$ | Cliff tick only |
| 33 | Booleanity (all bit columns) | $B_j(X)(B_j(X)-1) = 0$ | All ticks, each bit column |
