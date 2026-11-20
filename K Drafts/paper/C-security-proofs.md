# C Security Proofs

## C.1 Overview

We take knowledge soundness and zero knowledge for the polynomial argument as assumptions (Section C.2). We prove what the constraints imply about the logged book and clearing receipt, then give a simulation argument relative to a trusted auction with the same public outputs.

## C.2 Cryptographic Assumptions

We use the following as black boxes, in the algebraic group model with the $q$-discrete-logarithm assumption and a random oracle.

- **(K1) Knowledge soundness.** Let $\mathsf{Acc}$ be the event that the verifier accepts a proof of our polynomial protocol and $\mathsf{Vanish}$ the statement that every constraint polynomial vanishes on the domain for the columns behind the commitments. Then $\mathsf{Acc} \Rightarrow_\epsilon \mathsf{Vanish}$: an accepted proof implies $\mathsf{Vanish}$ except with probability $\epsilon$. This is the soundness guarantee of PLONK-style arguments over KZG [PLONK; Marlin].
- **(K2) Zero knowledge.** There is a simulator that, given a true statement, outputs a transcript indistinguishable from an honest one [Sefranek; Lipmaa]. It requires a hiding commitment and, for each committed polynomial, more random off-domain coefficients than opened points.
- **(K3) Binding.** A KZG commitment determines the polynomial behind it, and an equation between commitments that the adversary makes hold is an identity between those polynomials [KZG].
- **(K4) Lookup.** If every identity of the logarithmic-derivative lookup vanishes on the domain, every looked-up value is in the table, except with probability $\epsilon$ over its challenge [Haböck].
- **(K5)** The Schnorr proof is a proof of knowledge [Schnorr]. **(K6)** The hash is collision resistant.

Our argument is PLONK's technique with two changes that K1 and K2 must cover: a second prover round for the lookup helpers, and four columns whose commitments exist before the proof starts. Section C.8 says what this leaves open.

## C.3 The Reference Auction and Its Properties

**The reference auction.** A trusted exchange receives orders, acknowledges each one to its trader, and at the close publishes the outcome $\mathsf{Clear}(\text{book})$ of Definition 2. The public sees the number of orders, when each arrived, and the outcome. A trader also knows their own orders.

A batch auction has the verification problem of an election: many private inputs, one public result, and an operator who computes it. The properties we need have the same shape as end-to-end verifiability and ballot secrecy. Definition 1 states them and Table 7 sets each beside its election counterpart: recorded as submitted, no removal, cleared as recorded, no discretion, and order secrecy.

Each trader checks that their own order was recorded as submitted. Anyone can check non-negative updates, correct clearing, and uniqueness against the board. Order secrecy is against observers and traders; the exchange sees the book in the reference auction as well.

## C.4 Integrity

Let $\mathsf{Bd}$ be the board, $\mathsf{Rc}$ the set of receipts that pass their traders' checks against $\mathsf{Bd}$, and $R$ the published outcome. By K3 each board entry $\Phi_j$ determines a vector $u_j$ of values on $H$, its *contribution*. Let $\text{Book}^* = \sum_j u_j$, arranged by column.

We state the properties as predicates on $(\mathsf{Bd}, \mathsf{Rc}, R)$ and show, for each, which accepted checks imply it.

$$\psi_0:\quad \mathsf{Bd} \text{ is one sequence of commitments starting from an empty book.}$$

One published digest fixes the sequence by K6. For the start, the accepted opening gives a Schnorr proof for $C_r$ and the pairing equation $e(C_0 - C_r, [1]_2) = e(C_\rho, [Z_H(\tau)]_2)$. By K5 and K3, $C_r$ carries no values, and the equation is then the identity that the polynomial behind $C_0$ is a multiple of $Z_H$: every tick of every column starts at zero. The last commitment of a column therefore commits, on $H$, to the sum of that column's contributions.

$$\psi_1 \text{ (recorded as submitted)}:\quad \forall (\text{column}, \ell, q, \rho, C_{before}, C_{after}) \in \mathsf{Rc}:\ \text{the step } C_{after} - C_{before} \text{ has contribution } q \text{ at tick } \ell \text{ and } 0 \text{ elsewhere.}$$

The trader's check is the equation $C_{after} - C_{before} = q\,[L_\ell(\tau)]_1 + \rho\,\Gamma$ between public commitments. By K3 the polynomial behind the step is $q L_\ell$, up to blinding. The receipt is checked against consecutive entries of $\mathsf{Bd}$, so the step is one of the $\Phi_j$.

$$\psi_2 \text{ (no removal)}:\quad \forall j\ \exists\, q_j \in [0, 2^{k_{in}}),\ \ell_j:\quad u_j = q_j\, e_{\ell_j} \ \text{ or } \ u_j = 0.$$

This is the statement of the update-validity proof, and its argument is specific to our construction. Assume the proof is accepted; by K1 the constraints U1–U4 vanish on $K$. U1 with K4 gives $Qty[j] \in [0, 2^{k_{in}})$. U2–U4 are a running sum whose two sides telescope to

$$\sum_{j<\mu} \frac{\eta^j\, Qty[j]}{\beta - Tick[j]} = \sum_{i<n} \frac{w[i]}{\beta - \omega^i},$$

where $w$ is the vector of values on $H$ behind $W$. Both sides were fixed before $\beta$ was drawn, so, except with probability $2(M+n)/|\mathbb{F}|$, they agree as rational functions: for each tick $i$, the steps whose $Tick$ entry is $\omega^i$ have $\eta$-weighted quantities summing to $w[i]$, and the weighted quantities at each off-domain label sum to zero. The verifier computes $W = \Phi_{mask} + \sum_j \eta^j \Phi_j$ from $\mathsf{Bd}$, and $\Phi_{mask}$ is empty by the argument of $\psi_0$, so by K3 $w = \sum_j \eta^j u_j$. Define $e_{\ell_j}$ as the tick basis vector when $Tick[j]\in H$ and the zero vector otherwise. Then $\sum_j \eta^j (u_j - Qty[j]\, e_{\ell_j}) = 0$. For any off-domain label $t$ we also get $\sum_{j:\,Tick[j]=t}\eta^j Qty[j]=0$. The coefficients in these polynomials were fixed before $\eta$ was drawn. If any coefficient condition fails, fix one nonzero polynomial before drawing $\eta$. Its degree is below $\mu$, so it vanishes with probability at most $\mu/|\mathbb{F}|$. Thus each contribution equals its claimed tick vector, and an off-domain label can carry only zero quantity. An exchange that logs a step subtracting an acknowledged order violates $\psi_2$ and cannot produce this proof.

$$\psi_3 \text{ (cleared as recorded)}:\quad R = \mathsf{Clear}(\text{Book}^*).$$

Assume the clearing proof is accepted against the last commitments of $\mathsf{Bd}$. By K1 and K4 the constraints F1–F33 vanish on $H$ for a witness whose book columns are, by $\psi_0$, $\text{Book}^*$. Theorem 1 identifies the accepted receipt as $\mathsf{Clear}$ of the book in the witness. Hence $R = \mathsf{Clear}(\text{Book}^*)$. For a no-cross outcome, Proposition C.3 gives $V = 0$ in the same way.

$$\psi_4 \text{ (no discretion)}:\quad \mathsf{Bd} \text{ determines } R.$$

$\mathsf{Clear}$ is a function and is defined on every book, because a candidate always exists (Appendix A.2). With $\psi_3$, two accepted outcomes for one board are equal.

**Theorem 3 (integrity).** If the verifier accepts a batch, then except with probability at most $\varepsilon_{\mathrm{KZG}} + \varepsilon_{\mathrm{clear}} + \varepsilon_{\mathrm{upd}} + \varepsilon_{\mathrm{hash}}$ (Section C.7), the predicates $\psi_0$ to $\psi_4$ hold. In particular the outcome is the clearing of a book that contains every order with a verified receipt, at its tick and with at least its quantity.

*Proof.* The predicates hold by the arguments above. By $\psi_1$ a receipted order contributes $q$ at tick $\ell$, and by $\psi_2$ every other contribution to that tick is non-negative, so $\text{Book}^*$ has at least $q$ there. By $\psi_3$ the outcome is the clearing of $\text{Book}^*$. $\blacksquare$

**What the rule guarantees traders.** Proposition C.4 shows that each applicable fill ratio lies in $[0,1]$ and buyers and sellers trade equal volumes before rounding.

## C.5 The Constraints and the Clearing Rule

**Range fact.** We require $2^{k+1} < |\mathbb{F}|$. If a field element has a limb decomposition whose limbs are all in the table, it is the image of an integer in $[0, 2^b)$ for the width $b$ of the constraint. If a field element is the image of an integer $y$ with $|y| < 2^k$ and lies in $[0, 2^k)$, then $y \ge 0$: a negative $y$ has image $|\mathbb{F}| + y > 2^k$.

**Theorem 1 (soundness of the constraints).** Suppose the scalar checks hold for a receipt and a witness satisfies F1–F33 at every point of $H$. Then the columns $B, A, IOB, IOA$ of the witness form a book, and the receipt equals $\mathsf{Clear}$ of that book. In particular the book crosses.

*Proof.* We treat a value that the range fact makes an integer as that integer.

*Step 1: the book.* F1, F2, F24, F25 with F23 put the four book columns in $[0, 2^{k_{in}})$, and F3, F4, F26, F27 make them zero for $i \ge N$.

*Step 2: depth.* The factor $X - \omega^{n-1}$ of F7 is non-zero except at $i = n-1$, so $AccB_i = AccB_{i+1} + b_i$ for $i \le n-2$, and F5 gives $AccB_{n-1} = b_{n-1}$. By induction $AccB_i = D_i$. In the same way F6, F8 give $AccA_i = S_i$, and F28–F31 give the two IO depths.

*Step 3: executable volume.* By F9 and F10, $SurpB_i = D_i - Min_i$ and $SurpA_i = S_i - Min_i$. By F11 one is zero. If $SurpB_i = 0$ then $Min_i = D_i$ and $SurpA_i = S_i - D_i$; F13 and the range fact give $S_i \ge D_i$, so $Min_i = V_i$. The other case is symmetric with F12. Hence $Min_i = V_i$, $SurpB_i = D_i - V_i$, $SurpA_i = S_i - V_i$, and $\Delta_i = \delta_i$ by F18.

*Step 4: the ceiling.* $V_{max} < 2^k$ by the scalar checks, so F14 and the range fact give $V_i \le V_{max}$ for every $i$.

*Step 5: the plateau.* We use *unimodality*: for $i < j < l$, $V_j \ge \min(V_i, V_l)$, since $D_j \ge D_l \ge V_l$ and $S_j \ge S_i \ge V_i$. F15 and F16 give $V_c = V_d = V_{max}$ with $c \le d$. With Step 4, $V_{max} = V$, and $V \ge 1$ by the scalar checks. By unimodality $V_j = V$ for $c \le j \le d$. If $c \ge 1$, $Mask_C$ is 1 at $c - 1$ and F17 reads $V - V_{c-1} - 1 = Slack_{c-1}$; F22 makes the right side non-negative, so $V_{c-1} \le V - 1$. A tick $i < c-1$ with $V_i = V$ would give $V_{c-1} \ge V$ by unimodality. The right end is symmetric. Hence $\Pi = \{c, \ldots, d\}$.

*Step 6: the minimum imbalance and the side.* $Mask_P$ is 1 exactly on $\Pi$, so F19 gives $\delta_i \ge v$ on $\Pi$. The scalar checks put $p^*$ in $\Pi$, and F20 gives $SurpB_{p^*} = (1-s)v$ and $SurpA_{p^*} = sv$, so $\delta_{p^*} = v$ and $v = \min_{i \in \Pi} \delta_i$. If $v > 0$, the non-zero surplus identifies the long side, so $s = [g_{p^*} > 0]$. If $v = 0$, the scalar checks force $s = P = J = 0$.

*Step 7: $P$, $J$, and $p^*$ is a candidate.* If $v > 0$ then $\nu = 1$, F32 gives $P = \lambda_{p^*}$, and F33 gives $J$. The scalar check $v \le P$ says $\lambda_{p^*} \ge v$. If $v = 0$, $p^*$ is a candidate because $\delta_{p^*} = 0$.

*Step 8: no candidate lies below $p^*$.* Let $i \in \Pi$ with $i < p^*$; recall $g_{t+1} - g_t = a_{t+1} + b_t$.

- *$v > 0$, $s = 0$.* $g_{p^*} = -v$, so $g_i \le -v$. If $\delta_i = v$ then $g$ is constant on $[i, p^*]$, so $b_i = 0$; tick $i$ is demand long with $\lambda_i = 0 < v$. No constraint is needed, and $Mask_T = 0$.
- *$v > 0$, $s = 1$.* $Mask_T$ is 1 at $p^* - 1$, and F21 with F22 gives $\delta_{p^*-1} \ge v + 1$. Since $g_{p^*-1} \le g_{p^*} = v$, this forces $g_{p^*-1} \le -(v+1)$, so $\delta_i > v$.
- *$v = 0$.* F21 gives $\delta_{p^*-1} \ge 1$ with $g_{p^*-1} \le 0$, so $g_i \le -1$ and $\delta_i > v$.

The tick $p^* - 1$ is in $\Pi$ and the cliff ticks are not, so $Slack$ is constrained at most once per tick. $\blacksquare$

**Lemma C.1 (a candidate exists).** For every book that crosses, $\Pi$ contains a candidate.

*Proof.* If $v = 0$ every minimizer is a candidate. Let $v > 0$. For $i \in \Pi$, a demand-long tick has $S_i = V$ and $D_i = V + \delta_i$, and a supply-long tick has $D_i = V$ and $S_i = V + \delta_i$. Suppose the set of minimizers contains a demand-long tick and let $j$ be the highest, so $D_j = V + v$. If $j = d$, either $d = n - 1$ and $b_d = D_d \ge v$, or $V_{d+1} < V$ with $S_{d+1} \ge V$, so $D_{d+1} < V$ and $b_d > v$. If $j < d$, tick $j + 1 \in \Pi$ is not balanced, and it is not demand long, since it would then be a demand-long minimizer above $j$; so $D_{j+1} = V$ and $b_j = v$. In each case $j$ is a candidate. If every minimizer is supply long, the lowest one is a candidate by the symmetric argument. $\blacksquare$

**Theorem 2 (completeness of the constraints).** For every book that crosses, $\mathsf{Clear}$ of the book passes the scalar checks, and there is a witness satisfying F1–F33.

*Proof.* Take $AccB = D$, $AccA = S$, $Min_i = V_i$, the surpluses and $\Delta$ by their definitions, and $Slack$ zero except $Slack_{c-1} = V - V_{c-1} - 1$, $Slack_{d+1} = V - V_{d+1} - 1$ where those ticks exist, and $Slack_{p^*-1} = \delta_{p^*-1} - v - 1$ when $Mask_T \ne 0$. Padding ticks have $D_i = 0$, so $\Pi \subseteq [0, N-1]$ and the scalar checks hold; $P \ge v$ because $p^*$ is a candidate. Every constraint holds by construction once the three slack values are non-negative. The cliff ticks are outside $\Pi$. For $p^* - 1$ with $v = 0$: it is in $\Pi$ and not a candidate, so $\delta_{p^*-1} \ge 1$. For $p^* - 1$ with $v > 0$ and $s = 1$, suppose $\delta_{p^*-1} = v$. If $g_{p^*-1} = v$ then $a_{p^*} = 0 < v$, contradicting that $p^*$ is a candidate. If $g_{p^*-1} = -v$ then $p^* - 1$ is a demand-long minimizer whose upper neighbour is supply long, so $b_{p^*-1} = v$ as in Lemma C.1, and $p^* - 1$ is a lower candidate. $\blacksquare$

**Proposition C.3 (no-cross).** A witness satisfies F1–F8, F23, and $AccA \cdot AccB = 0$ if and only if its $B$ and $A$ form a book with $V = 0$.

*Proof.* As in Steps 1 and 2, $AccB = D$ and $AccA = S$ are integers in $[0, 2^k)$. A field has no zero divisors, so the product vanishes at $i$ exactly when $\min(D_i, S_i) = 0$. $\blacksquare$

**Proposition C.4 (execution).** If the receipt is $\mathsf{Clear}$ of the book, no fill ratio of Section 3.5 lies outside $[0,1]$, and before rounding the volume bought equals the volume sold equals $V + \min(J, v)$.

*Proof.* Let $I = \min(J, v)$ and $R = v - I$. If $v = 0$, $D_{p^*} = S_{p^*} = V$ and both sides fill $V$. If demand is long, $S_{p^*} = V$ and $D_{p^*} = V + v$. Sold: the asks at or below $p^*$, $V$, plus the admitted IO, $I$. Bought: the bids strictly above $p^*$, $V + v - P$, plus the at-price bids at ratio $(P - R)/P$, that is $P - v + I$; in total $V + I$. Since $P \ge v \ge R$, the long-side fill ratio lies in $[0,1]$. When $J>0$, the IO ratio lies in $[0,1]$ because $I\le J$. The supply-long case is symmetric. $\blacksquare$


## C.6 Order Secrecy

We follow Zeeperio's notion of non-expansivity: the zk-FBA reveals nothing beyond what the reference auction reveals.

**Lemma C.2 (non-expansive secrecy under transcript augmentation).** Let $\Pi$ be a protocol with adversary view $T$, and $\Pi^+$ an augmentation with view $(T, Z)$ on the same execution. If there is an efficient $\mathsf{Sim}$ such that $(T, Z)$ and $(T, \mathsf{Sim}(T))$ are indistinguishable, then for any secrecy experiment that depends on the protocol only through the adversary's view, an adversary against $\Pi^+$ yields one against $\Pi$ with negligibly different success.

**Theorem 4 (order secrecy).** Let $\Pi$ be the reference auction and $T$ the view of the observers and of a set of colluding traders: the number and arrival times of the orders, the outcome, and the colluding traders' own orders. Let $Z$ be what the zk-FBA adds: the opening, the board, the colluding traders' receipts, and the proofs. Assume an honest exchange produces at most one clearing proof per opened book. Under K1–K6 and the assumptions on the joint transcript in Section C.8, the zk-FBA is a non-expansive augmentation of $\Pi$.

*Proof.* $\mathsf{Sim}(T)$ produces $Z$ in three parts.

*Opening.* The opening does not depend on any order. The simulator generates it as the exchange does.

*Board.* A step is $\Phi = q\,[L_\ell(\tau)]_1 + \rho\,\Gamma$ on the order's column and $\rho'\Gamma$ on each other column, with fresh uniform $\rho, \rho'$. Each is a uniform group element whatever the order is. For every order of an honest trader, the simulator outputs four uniform elements, at the time given in $T$. For every order of a colluding trader, it computes the steps and the receipt from the order in $T$. Because all four columns move at every order, the simulator does not need the order's side.

*Proofs.* The outcome in $T$ is a true statement about some book, so by K2 the simulator of the polynomial protocol outputs transcripts for the clearing proof and the update-validity proof. K2 has two conditions, and they hold for columns committed inside a proof by fresh randomness. For the four book columns the randomness was fixed at the opening, and part of it is already public. The mask $\rho_0$ has four coefficients, and the public values that depend on it are $C_\rho$ and the column's evaluations at $\zeta$ and, for $A$ and $IOA$, at $\omega\zeta$: at most three linear functionals at distinct points, which are jointly uniform. The blinding polynomial likewise has four coefficients against at most three functionals, and the blindings known to colluding traders only shift its constant term by known amounts. Both conditions therefore hold with one coefficient to spare. $\blacksquare$

**Why one proof per book.** A second clearing proof of the same book at another $\zeta$ would add two functionals on $\rho_0$, five or six against four coefficients, and the evaluations would no longer be uniform. This condition has no counterpart in a proof system whose columns are all committed inside the proof; it comes from committing to the book before the batch closes.

**What the reference auction already reveals.** Non-expansivity is relative to the outcome. Our outcome holds the plateau $[c,d]$ and the volumes $P$ and $J$, which a closing auction does not all publish (Section 7). Hiding those values would require a clearing statement with fewer public inputs.

## C.7 Error Terms

K1 and K4 are stated with an error $\epsilon$. For our protocol we count it as follows; the counts are ours and follow the standard analysis.

*Lookup.* If a looked-up cell is outside the table, the difference of the two sides of the lookup sum is a non-zero rational function of $\beta$ whose numerator has degree below $(E+1)n$. The identities can hold only if $\beta$ is a root of it, or equals one of at most $En$ cell values, or one of at most $n$ table values, where a vanishing denominator frees a helper. The error is at most $(2E+2)\,n/|\mathbb{F}|$.

*Clearing proof.* With $t_F$ combined identities and $t_C$ commitments: the evaluation point contributes $(3n+9)/|\mathbb{F}|$, the combination $t_F/|\mathbb{F}|$, the opening batch $t_C/|\mathbb{F}|$, and the lookup the term above. For $t_F + t_C + 9 \le 2n$ this gives $\varepsilon_{\mathrm{clear}} \le (2E+7)\,n/|\mathbb{F}|$.

*Update-validity proof.* The polynomial protocol on $K$ contributes at most $5M/|\mathbb{F}|$, the range lookup of $Qty$ $(2E_Q+2)\,M/|\mathbb{F}|$, the challenge $\beta$ of the histogram identity $2(M+n)/|\mathbb{F}|$, and the challenge $\eta$ $\mu/|\mathbb{F}|$. With $n, \mu \le M$ this gives $\varepsilon_{\mathrm{upd}} \le (2E_Q+12)\,M/|\mathbb{F}|$.

Each term is per random-oracle query. With $|\mathbb{F}| \approx 2^{254}$, $n = 2^{10}$, $M = 2^{14}$, $E = 51$, and $E_Q = 4$, both are below $2^{-230}$.
## C.8 What Is Assumed and What Is Open

- **K1 and K2 for our variant.** The cited results concern PLONK-style arguments. We assume K1 and K2 for our protocol with its lookup round and commitments carried over from the log. K2 must cover the joint transcript of the clearing and update-validity proofs, including the public commitments and the receipts of colluding traders. The masking count alone does not establish this joint simulation. Section C.7 bounds the algebraic checks.
- **The exchange's own orders.** $\text{Book}^*$ may contain contributions with no receipt holder. $\psi_2$ makes them non-negative: the exchange can trade in its own auction, and it cannot remove an acknowledged order.
- **A refused order.** No predicate covers an order the exchange never logs. The trader holds no receipt and knows it.
- **One board.** $\psi_0$ needs the digest published where all parties read the same value.
- **Rounding.** The fill ratios are proved; the rounding of individual fills is a venue rule.
