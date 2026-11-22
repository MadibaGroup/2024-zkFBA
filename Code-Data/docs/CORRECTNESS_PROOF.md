# Correctness of the clearing rule

Assume the public scalar checks pass, the private lookups establish the stated ranges, and the committed witnesses satisfy the polynomial identities. Under those hypotheses the price selected by the [constraints](fixedConstraints.md) is the unique clearing tick of a crossing book. Completeness assumes nonexceptional lookup challenges. The [security analysis](SECURITY_BOOK.md) states the additional assumptions needed to infer those hypotheses from a verified transcript.

## 1. The clearing rule as a predicate

A **book** on $n$ ticks, of which the first $N \le n$ are real, is four sequences of integers $b_i, a_i, b^{io}_i, a^{io}_i$ for $0 \le i < n$ with

$$0 \le b_i, a_i, b^{io}_i, a^{io}_i < 2^{k_{in}}, \qquad \text{all four zero for } i \ge N. \tag{B}$$

$b_i$ is the bid volume with limit exactly tick $i$, $a_i$ the ask volume; market buys are recorded at tick $N-1$ and market sells at tick $0$. The $io$ sequences are the imbalance-only orders.

Define, for $0 \le i < n$,

$$D_i = \sum_{j \ge i} b_j, \qquad S_i = \sum_{j \le i} a_j, \qquad E_i = \min(D_i, S_i), \qquad g_i = S_i - D_i, \qquad \delta_i = |g_i|,$$

and likewise $D^{io}_i = \sum_{j \ge i} b^{io}_j$, $S^{io}_i = \sum_{j \le i} a^{io}_j$. $D_i$ is the demand willing to trade at price $i$, $S_i$ the supply, $E_i$ the volume that can execute at $i$. $D$ is non-increasing, $S$ and $g$ are non-decreasing. All depths and volumes, and the absolute imbalance, are below $N\cdot 2^{k_{in}}\le 2^k$ because padding rows are zero.

Tick $i$ is *demand long* if $g_i < 0$, *supply long* if $g_i > 0$, *balanced* if $g_i = 0$. The *at-price long volume* is $\lambda_i = b_i$ if $g_i < 0$ and $\lambda_i = a_i$ if $g_i > 0$.

**Definition (Clear).** Let $V = \max_i E_i$.

- If $V = 0$, $\mathsf{Clear}(\text{book}) = \textsf{no-cross}$.
- Otherwise let $\Pi = \{ i : E_i = V \}$ (the *plateau*), $c = \min \Pi$, $d = \max \Pi$, $v = \min_{i \in \Pi} \delta_i$. A *candidate* is a tick $i \in \Pi$ with $\delta_i = v$ and ($v = 0$ or $\lambda_i \ge v$). Let $p^*$ be the least candidate. Then

$$\mathsf{Clear}(\text{book}) = (V, c, d, p^*, v, s, P, J)$$

with $s = P = J = 0$ if $v = 0$, and otherwise $s = [g_{p^*} > 0]$, $P = \lambda_{p^*}$, and $J = S^{io}_{p^*}$ if $s = 0$, $J = D^{io}_{p^*}$ if $s = 1$.

This is the price-selection rule in [the specification](fixedConstraints.md). Lemma 6 shows a candidate always exists, so $\mathsf{Clear}$ is defined for every book.

---

## 2. Statement

A **witness** assigns to every tick the values of the thirteen value columns and of the limb columns. The **receipt** is $(V_{max}, c, d, p^*, V_{min\Delta}, s, P, J)$. We write $v$ for $V_{min\Delta}$ when no confusion arises.

**Theorem 1 (soundness).** Suppose the scalar checks of the [public receipt checks](fixedConstraints.md#public-receipt) hold for the receipt, and a witness satisfies F1-F33 at every point of $H$. Then the columns $B, A, IOB, IOA$ of the witness form a book in the sense of (B), and the receipt equals $\mathsf{Clear}$ of that book. In particular the book crosses.

**Theorem 2 (completeness).** For every book satisfying (B) with $\mathsf{Clear}(\text{book}) \ne \textsf{no-cross}$, the receipt $\mathsf{Clear}(\text{book})$ passes the scalar checks and there is a witness satisfying F1-F33.

**Proposition 3 (no-cross).** For nonexceptional lookup challenges, the ordinary columns admit witnesses for F1–F8, their range lookups and $AccA\cdot AccB=0$ exactly when $B$ and $A$ satisfy the entry and padding bounds (B) and $V=0$. This statement imposes no bound on the two final IO columns.

**Proposition 4 (execution).** If the receipt is $\mathsf{Clear}(\text{book})$, the pro-rata fill entitlements are well defined (no fill ratio exceeds 1 or is negative) and, before rounding, the volume bought equals the volume sold equals $V + \min(J, v)$.

**Corollary 5 (uniqueness).** For a given book at most one receipt can be proved, since $\mathsf{Clear}$ is a function.

---

## 3. From field elements to integers

The constraints are equations in the field $\mathbb{F}_q$; the rule is about integers. One fact bridges them. We use $2^{k+1} < q$.

**Range fact.** If a field element $x$ has a limb decomposition with all limbs in the table (F23), then $x$ is the image of an integer in $[0, 2^{k_{in}})$ or $[0, 2^k)$, as the constraint specifies. If in addition $x$ is known to be the image of an integer $y$ with $|y| < 2^k$, then $y \ge 0$: a negative $y$ has image $q + y > q - 2^k > 2^k$, which is outside $[0, 2^k)$.

From here on a column value that the range fact makes an integer is treated as that integer.

---

## 4. Proof of Theorem 1

Assume the scalar checks and F1-F33.

**Step 1: the book.** F1, F2, F24, F25 with F23 put $B_i, A_i, IOB_i, IOA_i$ in $[0, 2^{k_{in}})$. F3, F4, F26, F27 make them zero for $i \ge N$. This is (B). Write $b_i = B_i$ and so on.

**Step 2: depth.** F7 holds at every point of $H$, and its factor $X - \omega^{n-1}$ is non-zero except at $i = n-1$, so $AccB_i = AccB_{i+1} + b_i$ for $0 \le i \le n-2$. F5 gives $AccB_{n-1} = b_{n-1}$. By induction downward, $AccB_i = D_i$. In the same way F6 and F8 give $AccA_i = S_i$, and F28-F31 give $IOAccB_i = D^{io}_i$, $IOAccA_i = S^{io}_i$. These are equalities of field elements whose right sides are integers below $2^k$.

**Step 3: executable volume.** By F9, F10, $SurpB_i = D_i - Min_i$ and $SurpA_i = S_i - Min_i$ in the field. By F11 one of them is zero. If $SurpB_i = 0$ then $Min_i = D_i$ and $SurpA_i = S_i - D_i$, the image of an integer of absolute value below $2^k$; F13 with the range fact gives $S_i \ge D_i$, so $Min_i = D_i = \min(D_i, S_i)$. The case $SurpA_i = 0$ is symmetric, using F12. Hence

$$Min_i = E_i, \qquad SurpB_i = D_i - E_i, \qquad SurpA_i = S_i - E_i, \qquad \Delta_i = \delta_i \ \text{(F18)}.$$

**Step 4: the ceiling.** $V_{max} < 2^k$ by the scalar checks, so $V_{max} - E_i$ is an integer of absolute value below $2^k$; F14 with the range fact gives $E_i \le V_{max}$ for every $i$.

**Step 5: the plateau.** We use:

> **Unimodality.** For $i < j < l$: $E_j \ge \min(E_i, E_l)$. Indeed $D_j \ge D_l \ge E_l$ and $S_j \ge S_i \ge E_i$.

F15 and F16 give $E_c = E_d = V_{max}$, and $c \le d$ by the scalar checks. With Step 4, $V_{max} = \max_i E_i = V$, and $V \ge 1$ by the scalar check $V_{max} \ge 1$: the book crosses. By unimodality and Step 4, $E_j = V$ for all $c \le j \le d$.

Left end. If $c = 0$ there is no tick below. If $c \ge 1$, $Mask_C$ is 1 at $c-1$ and F17 reads $V - E_{c-1} - 1 = Slack_{c-1}$. The left side is an integer in $[-1, 2^k)$ and F22 makes it non-negative, so $E_{c-1} \le V - 1$. If some $i < c-1$ had $E_i = V$, unimodality would give $E_{c-1} \ge \min(E_i, E_c) = V$. So no tick below $c$ is in $\Pi$.

Right end. The scalar checks give $d \le N-1 \le n-1$. If $d = n-1$ there is no tick above. Otherwise $Mask_C$ is 1 at $d+1$ and the same argument shows no tick above $d$ is in $\Pi$.

Hence $\Pi = \{c, \ldots, d\}$: the receipt's $c$ and $d$ are those of $\mathsf{Clear}$.

**Step 6: the minimum imbalance and the side.** $Mask_P$ is 1 exactly on $\Pi$, so F19 with the range fact gives $\delta_i \ge v$ for all $i \in \Pi$ ($v < 2^k$ by the scalar checks). The scalar checks put $p^*$ in $[c, d] = \Pi$. F20a and F20b give $SurpB_{p^*} = (1-s)v$ and $SurpA_{p^*} = sv$, so $\delta_{p^*} = v$ by Step 3. Hence $v = \min_{i \in \Pi} \delta_i$.

If $v > 0$: when $s = 0$, $D_{p^*} - E_{p^*} = v > 0$, so $g_{p^*} < 0$; when $s = 1$, $g_{p^*} > 0$. So $s = [g_{p^*} > 0]$. If $v = 0$, the scalar checks force $s = P = J = 0$, as in $\mathsf{Clear}$.

**Step 7: $P$, $J$, and $p^*$ is a candidate.** If $v > 0$ then $\nu = 1$ and F32 gives $P = b_{p^*}$ when $s = 0$ and $P = a_{p^*}$ when $s = 1$; by Step 6 this is $\lambda_{p^*}$. F33 gives $J = S^{io}_{p^*}$ or $D^{io}_{p^*}$ accordingly. The scalar check $v \le P$ says $\lambda_{p^*} \ge v$. So $p^*$ is a candidate. If $v = 0$, $p^*$ is a candidate because $\delta_{p^*} = 0$.

**Step 8: no candidate lies below $p^*$.** Let $i \in \Pi$ with $i < p^*$. We show $i$ is not a candidate. Recall $g$ is non-decreasing and $g_{t+1} - g_t = a_{t+1} + b_t$.

*Case $v > 0$, $s = 0$.* Here $g_{p^*} = -v$, so $g_i \le -v$. If $\delta_i \ne v$, $i$ is not a candidate. If $\delta_i = v$ then $g_i = -v = g_{p^*}$, so $g$ is constant on $[i, p^*]$ and $a_{i+1} + b_i = 0$, giving $b_i = 0$. Tick $i$ is demand long, so $\lambda_i = b_i = 0 < v$: not a candidate. No constraint is needed for this case, and $Mask_T = 0$.

*Case $v > 0$, $s = 1$.* Here $g_{p^*} = v$ and $p^* > c$ (because $i$ exists), so $Mask_T$ is 1 at $p^* - 1$ and F21 reads $\delta_{p^*-1} - v - 1 = Slack_{p^*-1}$. The left side is an integer in $[-1, 2^k)$ by Step 6, and F22 makes it non-negative: $\delta_{p^*-1} \ge v + 1$. Since $g_{p^*-1} \le g_{p^*} = v$, this forces $g_{p^*-1} \le -(v+1)$. Then $g_i \le g_{p^*-1} \le -(v+1)$, so $\delta_i > v$: not a candidate.

*Case $v = 0$.* Here $g_{p^*} = 0$, $p^* > c$, $Mask_T$ is 1 at $p^* - 1$ and F21 gives $\delta_{p^*-1} \ge 1$. Since $g_{p^*-1} \le 0$, $g_{p^*-1} \le -1$, so $g_i \le -1$ and $\delta_i \ge 1 > v$: not a candidate.

The tick $p^* - 1$ is in $\Pi$ and the cliff ticks $c - 1$, $d + 1$ are not, so the single $Slack$ column is constrained at most once at each tick.

**Conclusion.** $V_{max} = V$ (Step 5), $c$ and $d$ are the ends of $\Pi$ (Step 5), $v$ is the minimum imbalance over $\Pi$ (Step 6), $p^*$ is the least candidate (Steps 7, 8), and $s$, $P$, $J$ are as in $\mathsf{Clear}$ (Steps 6, 7). $\qquad\blacksquare$

---

## 5. Proof of Theorem 2

Let the book satisfy (B) and cross. We first show $\mathsf{Clear}$ is defined.

**Lemma 6 (a candidate exists).** If $v = 0$ every minimizer is a candidate. Let $v > 0$ and let $M = \{ i \in \Pi : \delta_i = v \}$, which is not empty. For $i \in \Pi$: if $i$ is demand long then $S_i = V$ and $D_i = V + \delta_i$; if supply long then $D_i = V$ and $S_i = V + \delta_i$.

*(a) $M$ contains a demand-long tick.* Let $j$ be the highest one, so $D_j = V + v$.
- If $j = d$: either $d = n-1$, and then $b_d = D_d = V + v \ge v$; or $E_{d+1} < V$, and since $S_{d+1} \ge S_d = V$ it must be that $D_{d+1} < V$, so $b_d = D_d - D_{d+1} > v$. Either way $j$ is a candidate.
- If $j < d$: tick $j+1 \in \Pi$ is not balanced (as $v > 0$). If it were demand long, $V + v \le D_{j+1} \le D_j = V + v$ would make it a demand-long minimizer above $j$. So it is supply long, $D_{j+1} = V$, and $b_j = D_j - D_{j+1} = v$: $j$ is a candidate.

*(b) Every tick of $M$ is supply long.* Let $i$ be the lowest, so $S_i = V + v$.
- If $i = c$: either $c = 0$, and then $a_0 = S_0 = V + v \ge v$; or $E_{c-1} < V$, and since $D_{c-1} \ge D_c = V$ it must be that $S_{c-1} < V$, so $a_c > v$. Either way $i$ is a candidate.
- If $i > c$: tick $i - 1 \in \Pi$ is not balanced. If it were supply long, $V + v \le S_{i-1} \le S_i = V + v$ would make it a supply-long minimizer below $i$. So it is demand long, $S_{i-1} = V$, and $a_i = v$: $i$ is a candidate. $\qquad\blacksquare$

**The witness.** Take $AccB = D$, $AccA = S$, $Min = E$, $SurpB = D - E$, $SurpA = S - E$, $\Delta = \delta$, the IO depths likewise, and $Slack$ zero except: $Slack_{c-1} = V - E_{c-1} - 1$ if $c \ge 1$; $Slack_{d+1} = V - E_{d+1} - 1$ if $d + 1 < n$; $Slack_{p^*-1} = \delta_{p^*-1} - v - 1$ if $p^* > c$ and ($s = 1$ or $v = 0$). Limbs are the base-$2^w$ digits. This is what `clear` builds.

**Scalar checks.** For $i \ge N$, $D_i = 0$ by (B), so $E_i = 0 < V$ and $\Pi \subseteq [0, N-1]$: $0 \le c \le p^* \le d \le N-1$. $1 \le V < 2^k$, $v < 2^k$, $P < 2^{k_{in}} \le 2^k$, $J < 2^k$. If $v > 0$, $P = \lambda_{p^*} \ge v$ because $p^*$ is a candidate. If $v = 0$, $s = P = J = 0$ by definition.

**Constraints.** F1-F13, F18, F24-F31 hold by construction, with every range-checked quantity in range ($0 \le D - E, S - E < 2^k$). F14: $0 \le V - E_i < 2^k$. F15, F16: $c, d \in \Pi$. F19: $\delta_i \ge v$ on $\Pi$. F20a, F20b: at $p^*$ the surplus on the long side is $v$ and the other is zero, and $s$ names the long side. F32, F33: by definition of $P$ and $J$. It remains to show the three slack values are non-negative (F22); they are below $2^k$.

- $Slack_{c-1}$, $Slack_{d+1}$: these ticks are outside $\Pi$, so $E \le V - 1$ there.
- $Slack_{p^*-1}$ when $v = 0$: $p^* - 1 \in \Pi$ is not a candidate ($p^*$ is the least), so $\delta_{p^*-1} \ne 0$, that is $\delta_{p^*-1} \ge 1 = v + 1$.
- $Slack_{p^*-1}$ when $v > 0$, $s = 1$: suppose $\delta_{p^*-1} = v$ (it is at least $v$). If $g_{p^*-1} = v = g_{p^*}$, then $a_{p^*} + b_{p^*-1} = 0$, so $\lambda_{p^*} = a_{p^*} = 0 < v$, contradicting that $p^*$ is a candidate. If $g_{p^*-1} = -v$, then $p^* - 1$ is a demand-long minimizer whose upper neighbour in $\Pi$ is supply long, and the computation in Lemma 6(a) gives $b_{p^*-1} = v$: $p^* - 1$ is a candidate below $p^*$, a contradiction. So $\delta_{p^*-1} \ge v + 1$.

$Slack$ is assigned at most once per tick, as in Step 8. $\qquad\blacksquare$

---

## 6. Proofs of Propositions 3 and 4

**Proposition 3.** F1–F8 and the ordinary range lookups give (B) for $B$, $A$ and $AccB = D$, $AccA = S$ as in Steps 1 and 2; both are non-negative integers below $2^k < q$. A field has no zero divisors, so $AccA_i \cdot AccB_i = 0$ means one factor is zero in the field, hence zero as an integer: $E_i = \min(D_i, S_i) = 0$ for all $i$, that is $V = 0$. Conversely, if $V = 0$ the columns $D$, $S$ satisfy every constraint. $\qquad\blacksquare$

**Proposition 4.** Let $I = \min(J, v)$ and $R = v - I$.

*Balanced, $v = 0$.* $g_{p^*} = 0$ and $p^* \in \Pi$ give $D_{p^*} = S_{p^*} = V$. Every bid at or above $p^*$ and every ask at or below $p^*$ fills: $V$ bought, $V$ sold.

*Demand long, $s = 0$, $v > 0$.* $S_{p^*} = V$ and $D_{p^*} = V + v$. Sold: all asks at or below $p^*$, $V$, plus admitted sell IO, $I$. Bought: all bids strictly above $p^*$, $D_{p^*} - b_{p^*} = V + v - P$, plus the at-price bids at ratio $(P - R)/P$, that is $P - R = P - v + I$. Total bought $V + I$. The ratio $(P - R)/P$ lies in $[0, 1]$ exactly because $P \ge v \ge R$, which is the candidate condition; this is why the tie-break rule is the one under which the allocation exists. The IO ratio $I/J$ is in $[0,1]$ (and no IO is admitted when $J = 0$).

*Supply long* is symmetric. In every case the traded volume is $V + I = V_{exec}$. The Rust fill helper truncates each fractional result independently, outside the proof, so the realized volume on a rationed side can be lower by less than one unit per rationed order. $\qquad\blacksquare$

---

## 7. Evidence from tests

The Rust tests compare the implementation with the rule by direct search over small books.

| Statement | Test |
|---|---|
| Theorem 1 and Corollary 5, on small books | `only_the_true_receipt_satisfies_the_constraints`: for 300 random books on 6 ticks, every receipt with scalars up to the total volume is tried with the only witness that could fit it; exactly $\mathsf{Clear}(\text{book})$ is accepted, none for a no-cross book |
| Theorem 2 | `random_books_clear_and_check` and every honest proof test: the witness built by `clear` passes `check` |
| The code's `clear` is the $\mathsf{Clear}$ of Section 1 | `the_receipt_matches_the_written_rule`: 2,000 random books; the receipt is recomputed from the definitions of Section 1 by direct search and compared field by field |
| Lemma 6 | same test: a candidate is found for every crossing book |
| Proposition 4 | same test: the aggregate identities $S_{p^*} = V$, $D_{p^*} = V + v$ (and the symmetric ones), $P \ge v$, and bought = sold = $V + I$ |
| Proposition 3 | `no_cross`, `no_cross_batch_proves`, `a_crossing_book_cannot_be_declared_no_cross` |
| The polynomial system equals the integer constraints | `the_verifier_agrees_with_the_integer_checker` |
