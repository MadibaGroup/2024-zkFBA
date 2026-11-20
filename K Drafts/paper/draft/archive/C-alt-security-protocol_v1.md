# C (alternative) Security of the zk-FBA as an Auction Protocol

*Draft in Markdown. An alternative to `C-security-proofs.md`, organised as Zeeperio's security appendix is: properties of the application, proved for this protocol, with the properties of the proof system assumed and cited. Notation is that of Sections 3 and 4 and Appendices A and B.*

## C.1 Overview

The zk-FBA is a component of a market, not a market. We keep the argument narrow to what it adds: evidence, checkable by anyone, that a batch was cleared by the rule from the orders that were acknowledged, and nothing about those orders beyond what the auction itself announces.

We separate two kinds of claim, as Zeeperio does. A *cryptographic* claim says that an accepted proof implies that certain polynomials vanish, or that a transcript reveals nothing beyond that. These are properties of polynomial arguments in general. We assume them and cite them (Section C.2). A *semantic* claim says that the vanishing of our polynomials means what the auction requires. These are specific to this protocol, and we prove them (Sections C.4 and C.5).

The reference point is an auction run by a trusted exchange. We show that the zk-FBA is as correct as that auction without the trust, and reveals no more than it.

## C.2 Cryptographic Assumptions

We use the following as black boxes, in the algebraic group model with the $q$-discrete-logarithm assumption and a random oracle.

- **(K1) Knowledge soundness.** Let $\mathsf{Acc}$ be the event that the verifier accepts a proof of our polynomial protocol and $\mathsf{Vanish}$ the statement that every constraint polynomial vanishes on the domain for the columns behind the commitments. Then $\mathsf{Acc} \Rightarrow_\epsilon \mathsf{Vanish}$: an accepted proof implies $\mathsf{Vanish}$ except with probability $\epsilon$. This is the soundness guarantee of PLONK-style arguments over KZG [PLONK; Marlin].
- **(K2) Zero knowledge.** There is a simulator that, given a true statement, outputs a transcript indistinguishable from an honest one [Sefranek; Lipmaa]. It requires a hiding commitment and, for each committed polynomial, more random off-domain coefficients than opened points.
- **(K3) Binding.** A KZG commitment determines the polynomial behind it, and an equation between commitments that the adversary makes hold is an identity between those polynomials [KZG].
- **(K4) Lookup.** If every identity of the logarithmic-derivative lookup vanishes on the domain, every looked-up value is in the table, except with probability $\epsilon$ over its challenge [Haböck].
- **(K5)** The Schnorr proof is a proof of knowledge [Schnorr]. **(K6)** The hash is collision resistant.

Our argument is PLONK's technique with two changes that K1 and K2 must cover: a second prover round for the lookup helpers, and four columns whose commitments exist before the proof starts. Section C.6 says what this leaves open.

## C.3 The Reference Auction and Its Properties

**The reference auction.** A trusted exchange receives orders, acknowledges each one to its trader, and at the close publishes the outcome $\mathsf{Clear}(\text{book})$ of Definition 2. The public sees the number of orders, when each arrived, and the outcome. A trader also knows their own orders.

A batch auction has the verification problem of an election: many private inputs, one public result, and an operator who computes it. The properties we need have the same shape as end-to-end verifiability and ballot secrecy.

| Election property | Auction property | Statement |
|---|---|---|
| Recorded as cast | **Recorded as submitted** | Every order that a trader holds a verified receipt for is in the book that is cleared, with its tick and quantity |
| No ballot removed | **No removal** | No entry on the board reduces the volume of any order |
| Counted as recorded | **Cleared as recorded** | The published outcome is $\mathsf{Clear}$ of the book that the board records |
| A unique tally | **No discretion** | The board determines the outcome; the exchange chooses nothing |
| Ballot secrecy | **Order secrecy** | The public record, and the views of colluding traders, reveal nothing about other orders beyond what the reference auction reveals |

The first two are individual checks, made by each trader with a receipt. The next two are universal, made by anyone. Order secrecy is against observers and traders; the exchange sees the book in the reference auction as well.

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

where $w$ is the vector of values on $H$ behind $W$. Both sides were fixed before $\beta$ was drawn, so, except with probability $2(M+n)/|\mathbb{F}|$, they agree as rational functions: for each tick $i$, the steps whose $Tick$ entry is $\omega^i$ have $\eta$-weighted quantities summing to $w[i]$, and steps whose entry is not a tick contribute nothing. The verifier computes $W = \Phi_{mask} + \sum_j \eta^j \Phi_j$ from $\mathsf{Bd}$, and $\Phi_{mask}$ is empty by the argument of $\psi_0$, so by K3 $w = \sum_j \eta^j u_j$. Hence $\sum_j \eta^j (u_j - Qty[j]\, e_{\ell_j}) = 0$. The $u_j$, $Qty$, and $Tick$ were fixed before $\eta$ was drawn, so every term is zero, except with probability $\mu/|\mathbb{F}|$. An exchange that logs a step subtracting an acknowledged order violates $\psi_2$ and cannot produce this proof.

$$\psi_3 \text{ (cleared as recorded)}:\quad R = \mathsf{Clear}(\text{Book}^*).$$

Assume the clearing proof is accepted against the last commitments of $\mathsf{Bd}$. By K1 and K4 the constraints F1–F33 vanish on $H$ for a witness whose book columns are, by $\psi_0$, $\text{Book}^*$. The implication from vanishing to $\psi_3$ is not cryptographic. We read each column as a sequence of integers (a range-checked value is a non-negative integer, since $2^{k+1} < |\mathbb{F}|$) and match the constraints to the clauses of Definition 2.

- *Book.* F1–F4 and F24–F27: entries in $[0, 2^{k_{in}})$, padding empty.
- *Depth.* F5–F8, F28–F31: $AccB_i = D_i$, $AccA_i = S_i$, and the IO depths, by induction along the recurrences.
- *Executable volume.* F9–F13: one surplus is zero and both are non-negative, so $Min_i = \min(D_i, S_i) = V_i$, and $\Delta_i = |S_i - D_i|$ by F18.
- *Maximum and plateau.* F14: $V_i \le V_{max}$. F15, F16: equality at $c$ and $d$. Since $V_j \ge \min(V_i, V_l)$ for $i < j < l$, equality holds on $[c,d]$, and F17 with F22 makes the neighbouring ticks strictly lower, which excludes every tick outside. So $V_{max} = V$ and $[c,d] = \Pi$.
- *Minimum imbalance.* F19: $\Delta_i \ge V_{min\Delta}$ on $\Pi$. F20 and the scalar check $c \le p^* \le d$: equality at $p^*$, on side $s$.
- *Candidate.* F32 and the scalar check $V_{min\Delta} \le P$: the long side's volume at $p^*$ covers the imbalance.
- *Least candidate.* If demand is long, a lower minimizer $i$ has $g_i = g_{p^*}$, hence $b_i = 0$, and is not a candidate. If supply is long or the batch is balanced, F21 with F22 gives $\Delta_{p^*-1} > V_{min\Delta}$, and monotonicity of $g$ extends this to every lower plateau tick.
- *Rationing inputs.* F32, F33: $P$ and $J$ are the at-price long volume and the eligible IO depth at $p^*$.

Each clause of Definition 2 is matched, so $R = \mathsf{Clear}(\text{Book}^*)$. For a no-cross outcome, F1–F8 and $AccA \cdot AccB = 0$ give $\min(D_i, S_i) = 0$ at every tick, which is $V = 0$.

$$\psi_4 \text{ (no discretion)}:\quad \mathsf{Bd} \text{ determines } R.$$

$\mathsf{Clear}$ is a function and is defined on every book, because a candidate always exists (Appendix A.2). With $\psi_3$, two accepted outcomes for one board are equal.

**Theorem C.A (integrity).** If the verifier accepts a batch, then except with probability at most the sum of the errors of K1 and K4 for the two proofs, of K3, K5, and K6, and $(2M + 2n + \mu)/|\mathbb{F}|$, the predicates $\psi_0$ to $\psi_4$ hold. In particular the outcome is the clearing of a book that contains every order with a verified receipt, at its tick and with at least its quantity.

*Proof.* The predicates hold by the arguments above. By $\psi_1$ a receipted order contributes $q$ at tick $\ell$, and by $\psi_2$ every other contribution to that tick is non-negative, so $\text{Book}^*$ has at least $q$ there. By $\psi_3$ the outcome is the clearing of $\text{Book}^*$. $\blacksquare$

**What the rule guarantees traders.** Integrity of the computation is not yet a guarantee about fills. By Proposition C.8, at the clearing tick of Definition 2 both fill ratios lie in $[0,1]$ and the two sides trade the same volume. The candidate condition is exactly what makes this allocation exist.

## C.5 Order Secrecy

We follow Zeeperio's notion of non-expansivity: the zk-FBA reveals nothing beyond what the reference auction reveals.

**Lemma C.B (non-expansive secrecy under transcript augmentation).** Let $\Pi$ be a protocol with adversary view $T$, and $\Pi^+$ an augmentation with view $(T, Z)$ on the same execution. If there is an efficient $\mathsf{Sim}$ such that $(T, Z)$ and $(T, \mathsf{Sim}(T))$ are indistinguishable, then for any secrecy experiment that depends on the protocol only through the adversary's view, an adversary against $\Pi^+$ yields one against $\Pi$ with negligibly different success.

**Theorem C.C (order secrecy).** Let $\Pi$ be the reference auction and $T$ the view of the observers and of a set of colluding traders: the number and arrival times of the orders, the outcome, and the colluding traders' own orders. Let $Z$ be what the zk-FBA adds: the opening, the board, the colluding traders' receipts, and the proofs. If the exchange publishes one clearing proof per opened book, the zk-FBA is a non-expansive augmentation of $\Pi$.

*Proof.* $\mathsf{Sim}(T)$ produces $Z$ in three parts.

*Opening.* The opening does not depend on any order. The simulator generates it as the exchange does.

*Board.* A step is $\Phi = q\,[L_\ell(\tau)]_1 + \rho\,\Gamma$ on the order's column and $\rho'\Gamma$ on each other column, with fresh uniform $\rho, \rho'$. Each is a uniform group element whatever the order is. For every order of an honest trader, the simulator outputs four uniform elements, at the time given in $T$. For every order of a colluding trader, it computes the steps and the receipt from the order in $T$. Because all four columns move at every order, the simulator does not need the order's side.

*Proofs.* The outcome in $T$ is a true statement about some book, so by K2 the simulator of the polynomial protocol outputs transcripts for the clearing proof and the update-validity proof. K2 has two conditions, and they hold for columns committed inside a proof by fresh randomness. For the four book columns the randomness was fixed at the opening, and part of it is already public. The mask $\rho_0$ has four coefficients, and the public values that depend on it are $C_\rho$ and the column's evaluations at $\zeta$ and, for $A$ and $IOA$, at $\omega\zeta$: at most three linear functionals at distinct points, which are jointly uniform. The blinding polynomial likewise has four coefficients against at most three functionals, and the blindings known to colluding traders only shift its constant term by known amounts. Both conditions therefore hold with one coefficient to spare. $\blacksquare$

**Why one proof per book.** A second clearing proof of the same book at another $\zeta$ would add two functionals on $\rho_0$, five or six against four coefficients, and the evaluations would no longer be uniform. This condition has no counterpart in a proof system whose columns are all committed inside the proof; it comes from committing to the book before the batch closes.

**What the reference auction already reveals.** Non-expansivity is relative to the outcome. Our outcome holds the plateau $[c,d]$ and the volumes $P$ and $J$, which a closing auction does not all publish (Section 7). A venue that wants to reveal less must change the reference, which means changing the statement and not the proof.

## C.6 What Is Assumed and What Is Open

- **K1 and K2 for our variant.** The cited results are for PLONK. Our protocol uses the same technique with a second round and with externally committed columns. We have not proved K1 and K2 for this exact variant; Appendix C gives the error terms we expect, as sketches.
- **The exchange's own orders.** $\text{Book}^*$ may contain contributions with no receipt holder. $\psi_2$ makes them non-negative: the exchange can trade in its own auction, and it cannot remove an acknowledged order.
- **A refused order.** No predicate covers an order the exchange never logs. The trader holds no receipt and knows it.
- **One board.** $\psi_0$ needs the digest published where all parties read the same value.
- **Rounding.** The fill ratios are proved; the rounding of individual fills is a venue rule.
