# 4 The Committed Book and Trader Checks

We link the clearing proof to the orders traders submitted by committing to an empty book and logging each change to its commitments.

## 4.1 Commitment and Opening

The reference string holds $[\tau^i]_1$ for a secret $\tau$, and a short blinding basis $[\gamma\tau^i]_1$, $i \le 3$, for a second secret $\gamma$. We write $[x]_1$ for $x$ times the generator of the first pairing group and $\Gamma = [\gamma]_1$. Each of the four book columns has a running commitment

$$C = [\mathrm{col}(\tau)]_1 + [\rho_0(\tau)\,Z_H(\tau)]_1 + [\gamma\, r(\tau)]_1,$$

where $\mathrm{col}(X) = \sum_i \mathrm{col}[i]\, L_i(X)$ holds the column's values, $\rho_0$ is a random mask of degree 3, and $r$ is a blinding polynomial of degree 3. This is a hiding KZG10 commitment [KZG] to the masked column of Section 3.7. The mask is fixed when the batch opens, because the clearing proof later opens this polynomial at $\zeta$.

To open a batch, the exchange publishes for each column the commitment $C_0$ to the zero column, the elements $C_\rho = [\rho_0(\tau)]_1$ and $C_r = [\gamma r_0(\tau)]_1$, and a Schnorr proof [Schnorr] that it knows $r_0$ with $C_r$ in the span of the blinding basis. The verifier checks the Schnorr proof and

$$e(C_0 - C_r,\ [1]_2) = e(C_\rho,\ [Z_H(\tau)]_2).$$

The Schnorr proof binds $C_r$ to the blinding basis. The pairing equation then binds the value polynomial in $C_0$ to a multiple of $Z_H$, which vanishes at every tick.

## 4.2 Recording an Order

The commitment is additively homomorphic, so an order changes it by a commitment to the order alone.

> **Protocol 1 (order update).** The trader sends an order of quantity $q$ at tick $\ell$ for one column (a market buy uses $\ell = N-1$, a market sell $\ell = 0$).
> 1. The exchange draws $\rho, \rho_1', \rho_2', \rho_3'$ at random. It moves the order's column by $\Phi = q\,[L_\ell(\tau)]_1 + \rho\,\Gamma$ and each other column by $\rho_t'\,\Gamma$.
> 2. The exchange appends the four new commitments to the board, in a fixed column order.
> 3. The exchange returns the receipt $(\text{column}, \ell, q, \rho, C_{before}, C_{after})$ to the trader.
> 4. The trader checks $C_{after} = C_{before} + q\,[L_\ell(\tau)]_1 + \rho\,\Gamma$ and that $C_{before}$ and $C_{after}$ are consecutive commitments of that column on the board.

$\Phi$ is a Pedersen commitment [Pedersen] to $q$ at position $\ell$. The check in step 4 is an equation between public group elements, so it fixes what the step added: $q$ at tick $\ell$ and nothing at any other tick. The receipt is transferable, and a trader can show a third party what they submitted.

**What the board reveals.** With $\rho$ uniform, $\Phi$ is a uniform group element whatever $q$ and $\ell$ are. All four columns move at every order, three of them by blinding alone, so an observer cannot tell which column an order touched. The four steps expose the order's arrival time. Their joint distribution is independent of its side, type, price, and quantity.

**The log.** Each board entry carries a digest that hashes the previous digest, the entry's index, its column, and its commitment, and the chain starts from a hash of the opening. One digest then fixes the whole sequence of commitments. The exchange must publish the latest digest where all parties see the same value; the chain does not stop an exchange from showing different logs to different parties.

## 4.3 Update Validity

The clearing proof is about the sum of the logged steps. It does not show what each step did. An exchange could record a trader's order, return a valid receipt, and later log a step that subtracts the same quantity at the same tick: the book that clears no longer contains the order, and every check so far passes. We use a second proof to check each logged step.

**Statement.** Let $\Phi_0, \ldots, \Phi_{\mu-1}$ be the $\mu$ logged steps, four per order. For every $j$ there are a quantity $q_j \in [0, 2^{k_{in}})$ and a tick $\ell_j$ such that $\Phi_j$ commits to $q_j L_{\ell_j}$, up to a multiple of $Z_H$.

**Construction.** We use a second domain $K \supseteq H$ of size $M$, a power of two fixed at setup, with one row per step.

1. The exchange commits to a column $Qty$ with $Qty[j] = q_j$ and a column $Tick$ with $Tick[j] = \omega^{\ell_j}$, and to a masked empty commitment $\Phi_{mask}$ with the opening proof of Section 4.1.
2. A challenge $\eta$ is derived from every $\Phi_j$, $\Phi_{mask}$, and the commitments to $Qty$ and $Tick$.
3. Both sides compute $W = \Phi_{mask} + \sum_j \eta^j \Phi_j$ from the board. The exchange knows the column behind it: $w[i] = \sum_{j : \ell_j = i} \eta^j q_j$, a histogram of the steps weighted by powers of $\eta$.
4. The exchange proves, with the proof system of Section 3.7 run on $K$, that $Qty$ is in range (limbs and the lookup of Section 3.4) and that
   $$\sum_{j<\mu} \frac{\eta^j\, q_j}{\beta - Tick[j]} = \sum_{i<n} \frac{w[i]}{\beta - \omega^i}$$
   for a challenge $\beta$ drawn after the commitments. This is the lookup argument again, with two helper columns and a running sum, and the quantities in place of unit numerators.

The rational identity holds only if, at every tick $i$, the weighted quantities of the steps that name tick $i$ sum to $w[i]$, and a step that names no tick contributes nothing. Since $W$ is computed by the verifier from the log, $w$ is also the weighted sum of what the steps actually commit to. The two agree for a random $\eta$ only if they agree step by step. Every logged step therefore adds $q_j \ge 0$ at one tick and nothing elsewhere, except with probability at most $(2E_Q + 12)\,M/|\mathbb{F}|$, where $E_Q$ is the number of looked-up values of $Qty$ (Appendix C.7). A step that adds zero is valid, which is what the three blinding-only steps of each order are.

**Cost.** The 32-bit configuration reported in Section 5 uses 17 commitments and 1,700 bytes. The prover works on a domain of size $M$, and a batch holds at most $M/4$ orders. The verifier computes $W$ with one multi-scalar multiplication over the logged steps, so its work grows with the number of orders. This row layout checks each step separately. It allows more than one nonzero step in a group of four.

## 4.4 Verifying a Batch

A verifier accepts a batch if four checks pass: each opening commitment is empty; the log chains from the opening to the published digest; the update-validity proof verifies against the logged steps; and the clearing proof, or the no-cross proof, verifies against the last commitment of each column. A trader additionally runs step 4 of Protocol 1 for each of their orders and computes their fill from the receipt (Section 3.5).

