# 4. Linking the clearing price to the recorded book

A price proof on four final columns cannot distinguish two histories with the same totals. That is sufficient for clearing: ten one-unit bids and one ten-unit bid at the same tick contribute the same depth. It is insufficient for a trader who wants to identify which transition recorded their own bid. We keep the price grid for the closing proof and use a separate index, the position in the update log, for this second question.

## Opening the batch

Write $[x]_1$ and $[x]_2$ for the scalar $x$ in the two pairing groups. The reference string contains powers of a secret $\tau$ and a short blinding basis with independent secret $\gamma$. A running commitment to a column $b$ has the form

$$
 C=[b(\tau)+\rho_0(\tau)Z_H(\tau)]_1+[\gamma r(\tau)]_1 .
$$

The term $\rho_0 Z_H$ vanishes on every price row and randomizes evaluations away from $H$. Both $\rho_0$ and the initial blinding polynomial have degree at most three. The exchange samples them when the batch opens, before knowing which orders will arrive. [Appendix H](H-polynomial-checks.md) derives the pairing check for opening this commitment at a challenge point.

An empty opening must prevent the exchange from starting with undisclosed volume. For each column the exchange publishes $C_0$, $C_\rho=[\rho_0(\tau)]_1$ and $C_r=[\gamma r_0(\tau)]_1$. We use a generalized Schnorr proof of representation to prove knowledge of the coefficients of the blinding component ([8](references.md#ref-8), [27](references.md#ref-27)). The verifier checks this proof and

$$
        e(C_0-C_r,[1]_2)=e(C_\rho,[Z_H(\tau)]_2).
$$

The intended value component is thus a multiple of $Z_H$. It evaluates to zero at every price tick. The head of the initial log hashes the opening, so that a later history refers to a particular set of four starting commitments.

## Checking an order

Suppose a trader bids $q$ units at limit tick $\ell$. Adding that bid to its column changes the interpolant by $qL_\ell(X)$. The exchange samples fresh uniform $\rho$ and moves the bid commitment by

$$
                \Phi=q[L_\ell(\tau)]_1+\rho[\gamma]_1 .
$$

The three other columns move by independent blindings of zero. The exchange records the four new commitments in a fixed column order. An entry hashes its position, column and new commitment together with the preceding digest. For each entry the verifier subtracts the previous commitment of *the same column* from the newly published commitment. The preceding entry in the log can belong to a different column.

The exchange returns $(j,\ell,q,\rho,C_{\rm before},C_{\rm after})$ privately, where $j$ identifies the encoded column. The trader compares $(j,\ell,q)$ with the intended order, checks that $C_{\rm after}-C_{\rm before}=\Phi$, and identifies that transition between consecutive states of $j$ in the common history. An exchange could satisfy the receipt equation using two commitments it never published. Locating the transition in the log ties the check to the batch. The receipt has no authenticated submission identifier. Two receipts referring to the same transition establish a single contribution. Disclosing a receipt to another party reveals the associated fields and permits the same check.

## Checking the intervening updates

Even a correctly recorded addition proves little at close if a later log entry could remove it. Requiring an authenticated customer receipt for every entry would also change the disclosure policy. Instead, the exchange proves the following property of each public difference: its value on $H$ is a quantity in $[0,2^{k_{\rm in}})$ at one position, or zero everywhere. When the exchange follows the order-entry procedure, one order gives one quantity step and three zero steps. The verification relation imposes no fixed grouping, positive-step count or trader identity. A nonzero update is checked against $H$, which includes padding. For a crossing outcome the separate final-book constraints force all four padding regions to be empty.

Let $\Phi_0,\ldots,\Phi_{\mu-1}$ be the differences in log order. The prover uses an update domain of size $M\geq\max(n,\mu)$ that contains $H$. It first commits to the quantity and tick-label columns and a freshly masked empty commitment $C_{\rm mask}$ with its proof. Once the differences and these commitments are fixed, a challenge $\eta$ determines a weighted public commitment

$$
           C_W=C_{\rm mask}+\sum_{j<\mu}\eta^j\Phi_j .
$$

The verifier computes this combination itself. On $H$, the honest value $W_i$ is the sum of $\eta^jq_j$ over the steps labeled $\omega^i$. The prover range-checks the quantities and uses a second challenge $\beta$ to link the step rows to the price rows:

<a id="eq-hist"></a>

$$
\sum_{j<\mu}\frac{\eta^jq_j}{\beta-t_j}
       =\sum_{i<n}\frac{W_i}{\beta-\omega^i}. \tag{2}
$$

For a fixed $\eta$ and except at exceptional challenges, agreement as rational functions equates the total weight at every label. A label outside the domain has zero weight on the right. The commitments to steps, quantities and labels were fixed before $\eta$, so comparing the coefficients in $\eta$ separates individual contributions, including several updates with the same label. [Appendix B](B-constraint-relations.md) gives the running-sum constraints. The cryptographic conclusion requires consistent extraction of the polynomials behind the commitments.

Each accepted order contributes four log entries under the prescribed writing procedure. An update domain of $M$ rows therefore accommodates at most $\lfloor M/4\rfloor$ such orders. The update relation bounds the number of logged steps; it does not count authenticated submissions. The trader needs one commitment equation and a check that the transition appears between consecutive states of the same column in the log. The public verifier at close performs a multi-scalar multiplication over the entire log. The update proof is succinct for a fixed layout, while reading the log and computing $C_W$ cost work proportional to the number of entries.

## Verifying the batch at close

A verifier needs the opening, the ordered log, the update proof and the announced outcome with its proof. Starting at the four opening commitments, the verifier walks the entries in order. For an entry in column $j$, it subtracts the most recent commitment in $j$ from the new one and replaces that column's head. This pass produces the sequence of differences for [Eq. (2)](04-committed-book.md#eq-hist) and the final commitments for the outcome proof. Both checks must use the same opening and ordered entries. Two separately valid proofs about different commitments would tell a trader nothing about the price of the book containing their order.

The verifier checks the empty opening, replays the digest chain, verifies the update proof on the derived differences and checks the proof for the announced branch. It then compares the resulting digest with the one associated with the batch on the common publication channel. The batch verifier in our Rust implementation returns the digest, the number of steps and the final commitments after its cryptographic checks. Agreement on which digest was published for the instrument and cutoff is a separate responsibility of the venue. A hash chain lets a verifier detect a change relative to that digest. By itself, it gives two traders who have received different histories no way to decide which one closed.

Consider the ten-unit bid at tick 5 in [Table 1](02-auction-rule.md#table-1), submitted as one order. Its trader can identify a ten-unit transition in the ordinary bid column. After close the trader can verify the public batch and read $v=2$, $P=10$ and $J=1$ from the accepted receipt. One unit of sell IO reduces the excess demand from two to one. The trader can therefore calculate a nine-unit entitlement without learning the limits behind the two better-priced units or the identities of the sellers. If those ten units instead came from several orders, the same public receipt would give each a fill rate of $9/10$. A proof of the closing receipt would still leave the assignment of individual whole units to the venue.
