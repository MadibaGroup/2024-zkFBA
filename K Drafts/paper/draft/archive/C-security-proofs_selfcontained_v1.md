# C Security Proofs

*Draft in Markdown. Label: `app:security`. These arguments are our own and have not been reviewed by a second person; see the note at the end.*

## C.1 Model and Notation

**Setup.** The reference string is $[\tau^i]_1$ for $i \le 2M + 8$, the blinding basis $[\gamma\tau^i]_1$ for $i \le 3$, and $[1]_2$, $[\tau]_2$, $[Z_H(\tau)]_2$, $[Z_K(\tau)]_2$, for secrets $\tau, \gamma$ that no party knows. $\Gamma = [\gamma]_1$.

**Algebraic adversary.** In the algebraic group model the adversary outputs every element of the first group with its representation over the reference string. We write a representation as $a(X) + \gamma\, a'(X)$ with $\deg a' \le 3$, and call $a$ the *value part* and $a'$ the *blinding part*. Under the $q$-discrete-logarithm assumption, an equation between group elements that the adversary makes hold is, except with probability $\varepsilon_{\mathrm{KZG}}$, an identity between the representations as polynomials in $\tau$ and $\gamma$. We use this repeatedly as *binding*.

**Range fact.** We require $2^{k+1} < |\mathbb{F}|$. If a field element has a limb decomposition whose limbs are all in the table, it is the image of an integer in $[0, 2^b)$ for the width $b$ of the constraint. If a field element is the image of an integer $y$ with $|y| < 2^k$ and lies in $[0, 2^k)$, then $y \ge 0$: a negative $y$ has image $|\mathbb{F}| + y > 2^k$.

**The clearing rule.** We use the notation and the function $\mathsf{Clear}$ of Appendix A: depths $D_i$, $S_i$, executable volume $V_i$, signed imbalance $g_i$ with $\delta_i = |g_i|$, at-price long volume $\lambda_i$, plateau $\Pi$, and candidates.

## C.2 The Polynomial Protocol

**Lemma C.1 (lookup).** Let the looked-up columns $f_1, \ldots, f_E$ and the multiplicity column $Mult$ be fixed before $\beta$ is drawn. If some cell $f_e[i]$ is not a table value, the three identities of F23 hold on $H$ with probability at most $(2E + 2)\,n/|\mathbb{F}|$ over $\beta$.

*Proof.* We use the following fact [Haböck, Lemma 5]: over a field of characteristic larger than the number of terms, $\sum_i 1/(X - a_i) = \sum_i m_i/(X - b_i)$ as rational functions, for some field elements $m_i$, if and only if $\{a_i\} \subseteq \{b_i\}$. The direction we need follows from uniqueness of partial fractions. The residue at a value $z$ on the left is the number of cells equal to $z$, a positive integer at most $En < |\mathbb{F}|$ and hence non-zero in the field; on the right it is an empty sum when $z$ is not a table value. So if a cell is outside the table,

$$\sum_{i,e} \frac{1}{X - f_e[i]} - \sum_i \frac{Mult[i]}{X - T[i]}$$

is a non-zero rational function whose numerator has degree below $(E+1)n$. The identities can hold on $H$ only if $\beta$ is a root of that numerator (fewer than $(E+1)n$ values), or equals a cell value (at most $En$ values; the pair identity then leaves a helper free at that point), or equals a table value (at most $n$ values; the table-helper identity may then leave $U$ free). Outside these sets every denominator is non-zero, the identities force the helpers to be the stated fractions on $H$, and the telescoped running sum is the non-zero function evaluated at $\beta$. $\blacksquare$

The multiplicities are arbitrary field elements; nothing requires them to be counts. The running sum needs no initial condition, because only its differences are constrained.

**Lemma C.2 (the constraints hold on $H$).** Let $t_F$ be the number of combined identities and $t_C$ the number of commitments, with $t_F + t_C + 9 \le 2n$. If the verifier accepts a clearing proof, then except with probability $\varepsilon_{\mathrm{KZG}} + (2E+7)\,n/|\mathbb{F}|$ per random-oracle query, the value parts of the committed columns, restricted to $H$, satisfy F1–F33.

*Proof sketch.* This is the standard argument for a KZG-based polynomial protocol [PLONK; Marlin]. By binding, each commitment fixes a polynomial, and each batched opening is consistent with those polynomials except with probability $t_C/|\mathbb{F}|$ over the batching challenge. The accepted check $F(\zeta) = Q(\zeta) Z_H(\zeta)$ then holds for the committed polynomials at a point drawn after they were fixed; $F - Q Z_H$ has degree at most $3n + 9$, so it is the zero polynomial except with probability $(3n+9)/|\mathbb{F}|$, and $Z_H$ divides $F = \sum_t \alpha^t F_t$. A combination with powers of $\alpha$ of polynomials not all divisible by $Z_H$ is divisible for at most $t_F$ values of $\alpha$. Lemma C.1 adds $(2E+2)\,n/|\mathbb{F}|$. The masks are multiples of $Z_H$ and change no value on $H$. $\blacksquare$

## C.3 The Constraints Imply Correct Clearing

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

**Lemma (a candidate exists).** For every book that crosses, $\Pi$ contains a candidate.

*Proof.* If $v = 0$ every minimizer is a candidate. Let $v > 0$. For $i \in \Pi$, a demand-long tick has $S_i = V$ and $D_i = V + \delta_i$, and a supply-long tick has $D_i = V$ and $S_i = V + \delta_i$. Suppose the set of minimizers contains a demand-long tick and let $j$ be the highest, so $D_j = V + v$. If $j = d$, either $d = n - 1$ and $b_d = D_d \ge v$, or $V_{d+1} < V$ with $S_{d+1} \ge V$, so $D_{d+1} < V$ and $b_d > v$. If $j < d$, tick $j + 1 \in \Pi$ is not balanced, and it is not demand long, since it would then be a demand-long minimizer above $j$; so $D_{j+1} = V$ and $b_j = v$. In each case $j$ is a candidate. If every minimizer is supply long, the lowest one is a candidate by the symmetric argument. $\blacksquare$

**Theorem 2 (completeness of the constraints).** For every book that crosses, $\mathsf{Clear}$ of the book passes the scalar checks, and there is a witness satisfying F1–F33.

*Proof.* Take $AccB = D$, $AccA = S$, $Min_i = V_i$, the surpluses and $\Delta$ by their definitions, and $Slack$ zero except $Slack_{c-1} = V - V_{c-1} - 1$, $Slack_{d+1} = V - V_{d+1} - 1$ where those ticks exist, and $Slack_{p^*-1} = \delta_{p^*-1} - v - 1$ when $Mask_T \ne 0$. Padding ticks have $D_i = 0$, so $\Pi \subseteq [0, N-1]$ and the scalar checks hold; $P \ge v$ because $p^*$ is a candidate. Every constraint holds by construction once the three slack values are non-negative. The cliff ticks are outside $\Pi$. For $p^* - 1$ with $v = 0$: it is in $\Pi$ and not a candidate, so $\delta_{p^*-1} \ge 1$. For $p^* - 1$ with $v > 0$ and $s = 1$, suppose $\delta_{p^*-1} = v$. If $g_{p^*-1} = v$ then $a_{p^*} = 0 < v$, contradicting that $p^*$ is a candidate. If $g_{p^*-1} = -v$ then $p^* - 1$ is a demand-long minimizer whose upper neighbour is supply long, so $b_{p^*-1} = v$ as in the Lemma, and $p^* - 1$ is a lower candidate. $\blacksquare$

**Proposition C.3 (no-cross).** A witness satisfies F1–F8, F23, and $AccA \cdot AccB = 0$ if and only if its $B$ and $A$ form a book with $V = 0$.

*Proof.* As in Steps 1 and 2, $AccB = D$ and $AccA = S$ are integers in $[0, 2^k)$. A field has no zero divisors, so the product vanishes at $i$ exactly when $\min(D_i, S_i) = 0$. $\blacksquare$

**Proposition C.8 (execution).** If the receipt is $\mathsf{Clear}$ of the book, no fill ratio of Section 3.5 lies outside $[0,1]$, and before rounding the volume bought equals the volume sold equals $V + \min(J, v)$.

*Proof.* Let $I = \min(J, v)$ and $R = v - I$. If $v = 0$, $D_{p^*} = S_{p^*} = V$ and both sides fill $V$. If demand is long, $S_{p^*} = V$ and $D_{p^*} = V + v$. Sold: the asks at or below $p^*$, $V$, plus the admitted IO, $I$. Bought: the bids strictly above $p^*$, $V + v - P$, plus the at-price bids at ratio $(P - R)/P$, that is $P - v + I$; in total $V + I$. The ratio lies in $[0, 1]$ exactly because $P \ge v \ge R$, which is the candidate condition. The supply-long case is symmetric. $\blacksquare$

This is why the tertiary rule is the one under which the allocation exists.

## C.4 The Book

**Lemma C.4 (the book starts empty).** If the opening verifies, the value part of each $C_0$ vanishes on $H$.

*Proof.* The Schnorr proof is a proof of knowledge of a polynomial $\sigma$ of degree at most 3 with $C_r = [\gamma\,\sigma(\tau)]_1$, by special soundness and rewinding in the random-oracle model. If the adversary's representation of $C_r$ had a non-zero value part, the two representations would give a non-trivial relation on $\tau, \gamma$; so by binding $C_r$ has value part zero. Write the representations of $C_\rho$ and $C_0$ as $a_\rho + \gamma a'_\rho$ and $a_0 + \gamma a'_0$. The pairing check gives $a_0(\tau) + \gamma\,(a'_0(\tau) - \sigma(\tau)) = (a_\rho(\tau) + \gamma\, a'_\rho(\tau))(\tau^n - 1)$, which by binding is an identity in $\tau$ and $\gamma$; its part free of $\gamma$ says $a_0(X) = a_\rho(X)\,Z_H(X)$. $\blacksquare$

**Lemma C.5 (a receipt fixes its step).** If a receipt $(\text{column}, \ell, q, \rho, C_{before}, C_{after})$ passes the trader's check, the value part of $C_{after} - C_{before}$ is $q\,L_\ell(X)$.

*Proof.* The check is $C_{after} - C_{before} = q\,[L_\ell(\tau)]_1 + \rho\,\Gamma$, an equation between public group elements whose right side has value part $q L_\ell$. By binding, the adversary's representation of the left side has the same value part. $\blacksquare$

**Lemma C.6 (the board).** Two different sequences of commitments with the same head digest give a collision of the hash, which occurs with probability at most $\varepsilon_{\mathrm{hash}}$.

## C.5 Update Validity

**Lemma C.7.** Let the logged steps $\Phi_0, \ldots, \Phi_{\mu-1}$ and the commitments to $Qty$ and $Tick$ be fixed before $\eta$ is drawn. If the update-validity proof verifies, then except with probability $\varepsilon_{\mathrm{KZG}} + (2E_Q + 12)\,M/|\mathbb{F}|$ per random-oracle query, for every $j$ there are an integer $q_j \in [0, 2^{k_{in}})$ and a tick $\ell_j$ such that the value part of $\Phi_j$, restricted to $H$, is $q_j$ at tick $\ell_j$ and zero elsewhere, or is zero everywhere.

*Proof.* Let $u_j$ be the vector of values on $H$ of the value part of $\Phi_j$; it is fixed by binding. By the argument of Lemma C.2 on the domain $K$, with error at most $5M/|\mathbb{F}|$ for the evaluation point, the combination, and the batching, the constraints U1–U4 hold on $K$ for the committed $Qty$, $Tick$ and for the polynomial behind $W$. Since the verifier computes $W = \Phi_{mask} + \sum_j \eta^j \Phi_j$ itself and $\Phi_{mask}$ vanishes on $H$ (Lemma C.4), the values of that polynomial on $H$ are $w = \sum_j \eta^j u_j$.

U1 and Lemma C.1 give $q_j = Qty[j] \in [0, 2^{k_{in}})$, with error $(2E_Q + 2)\,M/|\mathbb{F}|$. U2–U4 give, by the telescoping argument of Lemma C.1,

$$\sum_{j < \mu} \frac{\eta^j q_j}{\beta - Tick[j]} = \sum_{i < n} \frac{w[i]}{\beta - \omega^i}.$$

Both sides are fixed before $\beta$. As rational functions they agree only if the residues agree at every point: $\sum_{j : Tick[j] = \omega^i} \eta^j q_j = w[i]$ for every tick $i$, and $\sum_{j : Tick[j] = z} \eta^j q_j = 0$ for $z \notin H$. Otherwise the identity holds for at most $M + n$ values of $\beta$, and at most $M + n$ further values make a denominator vanish. Let $e_j$ be the unit vector at the tick named by $Tick[j]$, or zero if $Tick[j] \notin H$. Then

$$\sum_{j < \mu} \eta^j \big(u_j - q_j e_j\big) = 0$$

as vectors. The $u_j$, $q_j$, $e_j$ were fixed before $\eta$, so unless every term vanishes this holds for at most $M - 1$ values of $\eta$. Summing, the error is at most $(5M + (2E_Q + 2)M + 2(M + n) + M)/|\mathbb{F}| \le (2E_Q + 12)\,M/|\mathbb{F}|$. $\blacksquare$

No separate check that $Tick[j] \in H$ is needed: a step that names no tick is forced to add nothing. A step on a padding tick is not excluded here; F3, F4, F26, and F27 reject the batch in that case.

## C.6 Proof of Theorem 3: Soundness and Inclusion

*Completeness.* Theorem 2 gives a witness for every book that crosses and Proposition C.3 for every book that does not. The honest prover fails only if a challenge makes a lookup denominator vanish, with probability at most $(E+1)\,n/|\mathbb{F}|$. Receipts and the board verify by construction.

*Clearing soundness.* Suppose the verifier accepts. By Lemma C.6 the board is one sequence of commitments. By Lemma C.4 each opening commitment has a value part that vanishes on $H$, so the value part of the last commitment of a column, restricted to $H$, is the sum of the restrictions of its steps. By Lemma C.7 every step adds a quantity in $[0, 2^{k_{in}})$ at one tick or nothing. Let $\text{Book}^*$ be the sum. The verifier checks that the four book commitments of the clearing proof are the last commitments on the board, so by Lemma C.2 the constraints F1–F33 hold for a witness whose book columns are $\text{Book}^*$, and by Theorem 1 the receipt is $\mathsf{Clear}(\text{Book}^*)$. If the outcome is no-cross, Proposition C.3 gives $V = 0$ in the same way. The errors add to the bound of Theorem 3.

*Inclusion.* By Lemma C.5 the step of a verified receipt has value part $q L_\ell$, and it is on the board the proof is bound to. Every other step is non-negative by Lemma C.7. The entry of $\text{Book}^*$ at tick $\ell$ of that column is therefore at least $q$. $\blacksquare$

## C.7 Privacy

**Lemma C.9 (one update).** For any order, the four logged steps are uniform and independent group elements.

*Proof.* $\Phi = q\,[L_\ell(\tau)]_1 + \rho\,\Gamma$ with $\rho$ uniform and $\Gamma \ne 0$ is uniform whatever $(q, \ell)$ are, and the three other steps are $\rho'\Gamma$ with fresh $\rho'$. $\blacksquare$

**Lemma C.10 (the whole view).** Under the one-proof rule, the view of the observers and of a set of colluding traders is simulatable from the outcome, the number and times of the orders, and the colluding traders' orders.

*Proof sketch.* The simulator runs the opening honestly, since the opening does not depend on any order. For each step of an honest trader it outputs a uniform group element (Lemma C.9), and for each step of a colluding trader it computes the step from the order. It then runs the simulator of the polynomial protocol on the resulting commitments.

That simulator needs two conditions for each committed column: its evaluations at the opened points are uniform and independent of the column, and its blinding polynomial has enough free coefficients for the hiding-KZG openings. For a column committed inside the proof, both hold by fresh randomness. For a book column the randomness was fixed at the opening and part of it is public, so we count.

- *Mask.* $\rho_0$ has four uniform coefficients. What is public about it is $C_\rho = [\rho_0(\tau)]_1$ and the evaluations of the masked column at $\zeta$, and for $A$ and $IOA$ also at $\omega\zeta$. The map $\rho_0 \mapsto (\rho_0(\tau), \rho_0(\zeta), \rho_0(\omega\zeta))$ from $\mathbb{F}^4$ is linear and surjective for three distinct points, so the three values are jointly uniform. The opened evaluations are uniform given $C_\rho$, and one coefficient is spare.
- *Blinding.* The blinding polynomial is $r_0$ plus the update blindings in its constant term. $r_0$ has four uniform coefficients; public are $C_r$ and at most two evaluations inside the batched openings. Three functionals on four coefficients are jointly uniform. The $\rho$ values known to colluding traders are known shifts of the constant term.

The update-validity proof opens $W$ at one point; its mask $\Phi_{mask}$ is drawn fresh for each proof, with four coefficients against two public functionals. $Qty$ and $Tick$ are fresh masked columns. $\blacksquare$

A second clearing proof for the same book at a different $\zeta$ would raise the public functionals on $\rho_0$ to five or six against four coefficients. This is the reason for the one-proof rule.

## Note on the status of these arguments

Theorems 1 and 2 and the propositions of C.3 are elementary arguments over the integers, and tests support them: an exhaustive search over receipts on 300 random small books accepts exactly $\mathsf{Clear}$, and a direct-search implementation of the definition agrees with the clearing code on 2,000 random books. Lemmas C.2, C.7, and C.10 rely on the standard analysis of KZG-based polynomial protocols and are sketches where marked. A full treatment would instantiate a published simulator for hiding KZG with masked polynomials [Sefranek] and state the knowledge-soundness theorem with its exact extractor.
