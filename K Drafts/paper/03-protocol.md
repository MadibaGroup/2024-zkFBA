# 3 The zk-FBA Protocol

## 3.1 Overview

The exchange receives orders, maintains the book, and clears each batch. Traders check their order receipts. Anyone can verify the outcome using the commitments and proofs on the public board.

One batch runs in four phases (Figure 1).

1. **Open.** The exchange publishes commitments to an empty book and a proof that it is empty.
2. **Collect.** For each order, the exchange updates the commitments, appends the change to the board, and returns an update receipt to the trader. The trader checks the receipt against the board.
3. **Close.** The exchange runs the clearing rule of Section 2.1 on the book. It publishes either a clearing receipt with a *clearing proof*, or a *no-cross proof* if no bid meets an ask. It also publishes an *update-validity proof* that every logged change is a valid order.
4. **Verify.** The verifier checks the opening, the chain of updates, and the two proofs against the final commitments.

<img width="937" height="441" alt="image" src="https://github.com/user-attachments/assets/227bc790-8ab3-4672-a70a-a66388df47e8" />

**Figure 1.** One batch of the zk-FBA. Solid arrows are messages; dashed arrows are reads of the public board.

**What is public.** The board exposes order counts and arrival times, and the outcome is public. Section 6 states the secrecy claim relative to these disclosures. The receipt holds eight values: the maximum executable volume $V_{max}$, the plateau $[c,d]$, the clearing tick $p^*$, the imbalance $V_{min\Delta}$ at $p^*$, its side $s$ (0 if demand is long, 1 if supply is long), the long side's volume $P$ at exactly $p^*$, and the eligible imbalance-only volume $J$ on the short side. When the batch is balanced, the side flag and both rationing volumes are zero.

## 3.2 Encoding the Book

**Domain.** Let $\mathbb{F}$ be the scalar field of BN254 and $H = \{1, \omega, \ldots, \omega^{n-1}\} \subset \mathbb{F}$ a multiplicative subgroup, with $n$ a power of two. The book has $N \le n$ price ticks, and tick $i$ sits at $\omega^i$. Ticks $N, \ldots, n-1$ are padding. A *column* is a polynomial of degree below $n$ given by its values on $H$; we write $B[i]$ for $B(\omega^i)$. A constraint $F(X) = 0$ means that $F$ vanishes on $H$, that is, $Z_H(X) = X^n - 1$ divides $F$. $L_i(X)$ is the Lagrange polynomial that is 1 at $\omega^i$ and 0 on the rest of $H$.

**Orders.** The book is four columns: limit bids $B$, limit asks $A$, imbalance-only bids $IOB$, and imbalance-only asks $IOA$. An order of quantity $q$ at tick $i$ adds $q$ to one entry of one column. We record a market buy at the highest tick $N-1$ and a market sell at tick 0, so that it is eligible at every price.

**Widths.** We fix two bit widths at setup. Every entry of the book is below $2^{k_{in}}$, and every accumulated quantity is below $2^k$ with $k \ge k_{in} + \lceil \log_2 N \rceil$. We use $k_{in} = 32$ and $k = 42$ for $N = 1000$.

**Derived columns.** From the book the exchange computes nine further columns (Table 1). Figure 2 shows them on a book of eight ticks, which we use as the running example.

**Table 1.** Committed columns and public polynomials. For $n = 1024$ and $k_{in} = 32$ there are 88 columns.

| Column | Value at tick $i$ | Count |
|---|---|---|
| $B$, $A$, $IOB$, $IOA$ | The book | 4 |
| $AccB$, $IOAccB$ | Demand depth: sum of bids at ticks $\ge i$ | 2 |
| $AccA$, $IOAccA$ | Supply depth: sum of asks at ticks $\le i$ | 2 |
| $Min$ | Executable volume $\min(AccA[i], AccB[i])$ | 1 |
| $SurpB$, $SurpA$ | $AccB[i] - Min[i]$ and $AccA[i] - Min[i]$ | 2 |
| $\Delta$ | Imbalance $\lvert AccA[i] - AccB[i] \rvert$ | 1 |
| $Slack$ | Gap witnesses for strict inequalities | 1 |
| Limbs | 8-bit limbs of the nine range-checked quantities | 46 |
| $Mult$, $h_1, \ldots, h_{26}$, $U$, $S$ | Lookup columns (Section 3.4) | 29 |
| **Public polynomial** | **Definition** | |
| $Pad$ | $\sum_{i=N}^{n-1} L_i$ | |
| $Mask_P$ | $\sum_{i=c}^{d} L_i$, the plateau | |
| $Mask_C$ | $L_{c-1} + L_{d+1}$, omitting a term whose index leaves $[0, n-1]$ | |
| $Mask_T$ | $L_{p^*-1}$ if $p^* > c$ and ($s = 1$ or $V_{min\Delta} = 0$), else 0 | |
| $T$ | $\sum_{i < 256} i \cdot L_i$, the range table | |

The verifier derives the selectors from the receipt and the padding mask and range table from the public parameters.

![Worked book](../figures/fig2-worked-book.svg)

**Figure 2.** A book of eight ticks. Bids of 3, 10, 2 rest at ticks 2, 5, 6 and asks of 4, 6, 5, 3 at ticks 1, 2, 6, 7; a sell IO order of 1 rests at tick 4. The executable volume reaches $V_{max} = 10$ on the plateau $[2,5]$. The imbalance is 5 at tick 2 and 2 at ticks 3 to 5, with demand long. Three ticks tie, and the tie-break selects $p^* = 5$. The receipt is $(V_{max}, c, d, p^*, V_{min\Delta}, s, P, J) = (10, 2, 5, 5, 2, 0, 10, 1)$.

## 3.3 Walkthrough of a Constraint

In Phase 1 we require $Min[i] = \min(AccA[i], AccB[i])$ at every tick. We encode the minimum through two surplus columns.

$$SurpB(X) - \big(AccB(X) - Min(X)\big) = 0, \qquad SurpA(X) - \big(AccA(X) - Min(X)\big) = 0 \tag{F9, F10}$$

$$SurpA(X) \cdot SurpB(X) = 0 \tag{F11}$$

$$SurpB[i] \in [0, 2^k), \qquad SurpA[i] \in [0, 2^k) \quad \text{for all } i \tag{F12, F13}$$

F11 makes one surplus zero at each tick, so $Min$ equals one of the two depths. F12 and F13 make both surpluses non-negative, so $Min$ is not the larger one. A negative integer appears in $\mathbb{F}$ as an element near the field size, far outside $[0, 2^k)$, which is why a range check expresses non-negativity. At tick 3 of Figure 2, $AccB = 12$ and $AccA = 10$: the only assignment that satisfies the five constraints is $Min = 10$, $SurpB = 2$, $SurpA = 0$.

Without F9 and F10 the prover could set both surpluses to zero, and F11 to F13 would say nothing about $Min$: every derived column needs a constraint that ties it to its definition.

The same surpluses give the imbalance without an absolute-value gadget. Since one surplus is zero and the other is the difference of the depths,

$$\Delta(X) - \big(SurpA(X) + SurpB(X)\big) = 0 \tag{F18}$$

forces $\Delta[i] = \lvert AccA[i] - AccB[i] \rvert$.

## 3.4 Range Checks

Every inequality in the protocol is a range check, and we use one gadget for all of them. To prove $Y[i] \in [0, 2^b)$ for a column $Y$ at every tick, the prover commits to limb columns $\ell_0, \ldots, \ell_{m-1}$ with $m = \lceil b/8 \rceil$ and shows

$$Y(X) - \sum_{l=0}^{m-1} 2^{8l}\, \ell_l(X) = 0,$$

together with the claim that every limb value lies in the table $\{0, \ldots, 255\}$ (for a domain smaller than 256 we use narrower limbs). When $b$ is not a multiple of 8, the top limb must be below $2^r$ with $r = b \bmod 8$; this holds exactly when $2^{8-r}$ times the top limb is also in the table, so we look that value up as well. The range proved is exactly $[0, 2^b)$.

We prove table membership for all limbs of all nine quantities with one logarithmic-derivative lookup [Haböck]. Let $f_1, \ldots, f_E$ be the looked-up columns and $T$ the table column. The prover commits to a multiplicity column $Mult$, where $Mult[v]$ counts the cells equal to $v$. After a challenge $\beta$, it commits to helper columns defined on $H$ by

$$h_j = \frac{1}{\beta - f_{2j-1}} + \frac{1}{\beta - f_{2j}}, \qquad U = \frac{Mult}{\beta - T}, \qquad S[i+1] = S[i] + \sum_j h_j[i] - U[i].$$

Three identities fix them:

$$h_j \,(\beta - f_{2j-1})(\beta - f_{2j}) - (\beta - f_{2j-1}) - (\beta - f_{2j}) = 0, \qquad U\,(\beta - T) - Mult = 0,$$

$$S(\omega X) - S(X) - \Big(\sum_j h_j(X) - U(X)\Big) = 0. \tag{F23}$$

The last identity holds on all of $H$, including the step from $\omega^{n-1}$ back to 1. Summed over $H$ its left side telescopes to zero, which gives

$$\sum_{i=0}^{n-1} \sum_{e=1}^{E} \frac{1}{\beta - f_e[i]} = \sum_{i=0}^{n-1} \frac{Mult[i]}{\beta - T[i]}.$$

As an identity of rational functions in $\beta$, this holds only if every $f_e[i]$ is a table value. The limbs and $Mult$ are committed before $\beta$ is drawn, so a prover with a cell outside the table passes with probability at most $(2E+2)\,n/|\mathbb{F}|$ (Appendix C.7). The running sum needs no initial condition, since only its differences are constrained.

A first version of the protocol committed to one column per bit, with a booleanity constraint on each [Plonkbook]. For 32-bit volumes that is 351 columns. With limbs and the lookup we have 46 limb columns and 29 lookup columns, and the proof falls from 22.9 KB to 6.1 KB. The lookup costs a second prover round and raises the degree of the quotient from about $n$ to about $2n$.

## 3.5 Clearing Constraints

Table 2 lists the constraints by the phase of the rule they enforce. Appendix B states each one in full.

**Table 2.** The clearing constraints. A range check is a limb decomposition whose limbs enter the lookup F23.

| Group | Constraints | What they force |
|---|---|---|
| Book | F1, F2, F24, F25 (range); F3, F4, F26, F27 ($Pad \cdot B = 0$, and likewise) | Entries in $[0, 2^{k_{in}})$; padding ticks empty |
| Depth | F5–F8, F28–F31 | $AccB$, $IOAccB$ are suffix sums; $AccA$, $IOAccA$ are prefix sums |
| Executable volume | F9–F13 | $Min = \min(AccA, AccB)$ |
| Phase 1: plateau | F14 (range of $V_{max} - Min$); F15, F16 ($Min = V_{max}$ at $c$ and $d$); F17 (cliffs) | $V_{max}$ is the maximum of $Min$ and $[c,d]$ is exactly where it is attained |
| Phase 2: clearing tick | F18 ($\Delta$); F19 (range of $\Delta - V_{min\Delta}$ on the plateau); F20 (pins at $p^*$); F21 (tie-break); F22 (range of $Slack$) | $V_{min\Delta}$ is the minimum imbalance on the plateau, attained at $p^*$ on side $s$; no lower tick qualifies |
| Phase 3: rationing | F32, F33 (pins at $p^*$) | $P$ and $J$ are the at-price long volume and the eligible IO volume |
| Lookup | F23 | Every limb is in $\{0, \ldots, 255\}$ |

**Depth.** The two accumulators run in opposite directions:

$$L_{n-1}(X)\big(AccB(X) - B(X)\big) = 0, \qquad (X - \omega^{n-1})\big[AccB(X) - AccB(\omega X) - B(X)\big] = 0, \tag{F5, F7}$$

$$L_{0}(X)\big(AccA(X) - A(X)\big) = 0, \qquad (X - \omega^{n-1})\big[AccA(\omega X) - AccA(X) - A(\omega X)\big] = 0. \tag{F6, F8}$$

The factor $(X - \omega^{n-1})$ switches the recurrence off at the point where the domain wraps around. The demand recurrence adds the current tick's bids. The supply recurrence adds the next tick's asks. Both run through the padding, where F3 and F4 force the book to zero; without F3, volume hidden on a padding tick would act as a bid above every real price.

**Phase 1.** F14 gives $Min \le V_{max}$ everywhere, and F15 and F16 pin $Min = V_{max}$ at the two endpoints. The cliff constraint makes the ticks just outside strictly lower:

$$Mask_C(X) \cdot \big(V_{max} - Min(X) - 1 - Slack(X)\big) = 0, \tag{F17}$$

with $Slack \ge 0$ by F22. These four constraints pin the whole plateau, because $Min$ is the pointwise minimum of a non-increasing and a non-decreasing sequence: for $i < j < l$ we have $Min[j] \ge \min(Min[i], Min[l])$. Every tick between $c$ and $d$ therefore attains $V_{max}$, and a tick beyond a cliff that attained it would lift the cliff to $V_{max}$ as well. The executable volume at the cliffs stays hidden behind $Slack$; opening it would disclose depth that the receipt does not contain. $Mask_C$ drops a cliff that does not exist ($c = 0$ or $d = n-1$), so no index wraps around the domain.

**Phase 2.** F19 proves $\Delta \ge V_{min\Delta}$ on the plateau, and the two pins

$$L_{p^*}(X)\big(SurpB(X) - (1-s)\,V_{min\Delta}\big) = 0, \qquad L_{p^*}(X)\big(SurpA(X) - s\,V_{min\Delta}\big) = 0 \tag{F20}$$

show that the minimum is attained at $p^*$ and fix which side is long.

Several plateau ticks can attain the minimum, as ticks 3, 4, and 5 do in Figure 2. The rule must then select one, or the exchange could choose among them. Phase 3 fills every eligible ordinary order priced strictly better than $p^*$ and rations only the long side's orders at exactly $p^*$. In Figure 2, clearing at tick 3 would leave the 12 bids at ticks 5 and 6 strictly better than the price while the asks can fill only 10 of them. We require the at-price long volume to cover the imbalance before admitting IO orders, $V_{min\Delta} \le P$. We take this as the tertiary rule:

> Among the plateau ticks with minimum imbalance, $p^*$ is the lowest one whose long-side volume at that tick covers the imbalance.

In Figure 2 this is tick 5, where $P = 10 \ge 2$. For a positive imbalance there are at most two candidates, the highest demand-long minimizer and the lowest supply-long minimizer. At least one candidate exists. In a balanced batch every minimum-imbalance tick is a candidate (Appendix A). The verifier checks $V_{min\Delta} \le P$ on the receipt. A lower qualifying tick can exist only when supply is long or the batch is balanced, and in that case one constraint excludes it:

$$Mask_T(X) \cdot \big(\Delta(X) - V_{min\Delta} - 1 - Slack(X)\big) = 0. \tag{F21}$$

The tick $p^* - 1$ lies inside the plateau and the cliffs lie outside it, so one $Slack$ column serves F17 and F21.

**Phase 3.** Two pins bind the remaining receipt values to the book, with $\nu = 1$ if $V_{min\Delta} > 0$ and 0 otherwise:

$$\nu\, L_{p^*}(X)\big((1-s)\,B(X) + s\,A(X) - P\big) = 0, \qquad \nu\, L_{p^*}(X)\big((1-s)\,IOAccA(X) + s\,IOAccB(X) - J\big) = 0. \tag{F32, F33}$$

Everything else is public arithmetic on the receipt. The admitted IO volume is $I = \min(J, V_{min\Delta})$, the residual is $R = V_{min\Delta} - I$, and the traded volume is $V_{max} + I$. A long-side order at exactly $p^*$ fills at the ratio $(P - R)/P$, and an eligible IO order on the short side fills at $I/J$ when $J>0$. Every other eligible limit or market order fills in full. In Figure 2, $I = 1$, $R = 1$, the bids at tick 5 fill at $9/10$, and 11 units trade. A trader computes their unrounded fill from the receipt and their order.

**Scalar checks.** Before any polynomial check, the verifier rejects a receipt unless $0 \le c \le p^* \le d \le N-1$, $1 \le V_{max} < 2^k$, $s \in \{0,1\}$, $0 \le V_{min\Delta} \le P < 2^k$, $0 \le J < 2^k$, and $s = P = J = 0$ when $V_{min\Delta} = 0$. No polynomial constraint places $p^*$ inside the plateau; the scalar check does.

**Uniqueness.** The scalar checks and F1–F33 hold for a book if and only if the receipt is the output of the clearing rule on that book (Theorems 1 and 2, Appendix C). A book has one valid receipt, so the exchange has no freedom in the outcome.

## 3.6 The No-Cross Statement

A no-cross batch has $Min=0$ at every tick and fails the clearing receipt's requirement that $V_{max}>0$. We prove this case as a separate, smaller statement. The exchange commits to $B$, $A$, $AccB$, $AccA$ and the limbs of $B$ and $A$, and proves F1–F8, the lookup, and

$$AccA(X) \cdot AccB(X) = 0.$$

Both depths are sums of range-checked entries, so they are non-negative, and the product vanishes at a tick exactly when $\min(AccA, AccB) = 0$ there. No receipt value is disclosed. Imbalance-only orders take no part, since they cannot create a cross.

## 3.7 From Constraints to a Proof

We compile the constraints with the standard PIOP steps [PLONK], with two rounds of witness commitments followed by a quotient commitment.

1. The prover commits to the value columns, the limb columns, and $Mult$. The book columns are already committed on the board (Section 4).
2. The verifier sends $\beta$. The prover commits to the lookup helpers.
3. The verifier sends $\alpha$. The prover combines all identities into $F(X) = \sum_t \alpha^t F_t(X)$, computes the quotient $Q(X) = F(X)/Z_H(X)$, and commits to it.
4. The verifier sends $\zeta$. The prover opens every column at $\zeta$, and the columns that appear shifted ($AccB$, $AccA$, $IOAccB$, $IOAccA$, $A$, $IOA$, $S$) at $\omega\zeta$ as well.
5. The verifier evaluates the public polynomials at $\zeta$, recomputes $F(\zeta)$ from the openings, and checks $F(\zeta) = Q(\zeta)\, Z_H(\zeta)$.

We commit with KZG10 [KZG] over BN254 in its hiding form, batch all openings at one point into one group element with a random linear combination, and verify both points with two pairings. We remove the verifier's messages with the Fiat–Shamir transform over a SHA-256 transcript that absorbs the parameters, the receipt, and every commitment. For zero knowledge, the prover adds to each column a random multiple of $Z_H$, which changes no value on $H$ and makes the openings at $\zeta$ and $\omega\zeta$ independent of the column.

The pair identities of the lookup have degree 3 in the columns and every other constraint has degree at most 2, so the quotient has degree about $2n$ and the reference string must reach $2n + 8$. For $n = 1024$ and 32-bit volumes, the proof holds 89 commitments (88 columns and the quotient) and their openings in 6,124 bytes. At fixed limb and witness widths, the number of commitments, openings, and pairings is fixed. The verifier also evaluates public polynomials. In our implementation the plateau mask costs time proportional to its width (Appendix D).
