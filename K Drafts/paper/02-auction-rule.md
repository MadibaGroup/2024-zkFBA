# 2. Auction and cryptographic background

## The batch and its order book

A frequent batch auction collects orders over a short interval and, when interest crosses, matches them at a common price ([6](references.md#ref-6), [7](references.md#ref-7)). In our protocol one batch covers one asset on $N$ increasing price ticks. Each batch starts empty, and each order accepted before the close adds volume at one tick. Amendments, cancellations and carry-over are outside this batch model. Those choices matter for verification: a subtraction that would be legitimate in a venue accepting cancellations is invalid in the relation we prove. The FBA literature also considers orders that remain outstanding across batches ([6](references.md#ref-6)).

Let $B_i,A_i$ be ordinary bid and ask quantities with limit exactly at tick $i$, and let $B_i^{o},A_i^{o}$ denote imbalance-only (IO) interest. We encode a market buy at the top tick and a market sell at the bottom tick. They are eligible everywhere on the grid. At the corresponding endpoint the encoding gives a market order the same treatment as an ordinary limit order. Ordinary demand, supply, executable volume and signed imbalance are

<a id="eq-depth"></a>

$$
\begin{aligned}
D_i&=\sum_{j\geq i}B_j, & S_i&=\sum_{j\leq i}A_j,\\
 V_i&=\min(D_i,S_i), & g_i&=S_i-D_i.
\end{aligned}\tag{1}
$$

Write $\delta_i=|g_i|$. A negative $g_i$ means demand is long. A positive $g_i$ means supply is long. These quantities exclude IO interest.

## Selecting a tick

Set $V_{\max}=\max_i V_i$. If $V_{\max}=0$, the outcome is *no cross*, and there is no clearing price. Otherwise the maximizing ticks form an interval $[c,d]$. Indeed, for $i<j<k$, depth monotonicity gives $D_j\geq V_k$ and $S_j\geq V_i$, so $V_j\geq\min(V_i,V_k)$. A tick between two maxima must also maximize volume.

Let $v=\min_{c\leq i\leq d}\delta_i$. A minimizer qualifies if the book is balanced there, or if its ordinary long-side volume at that exact tick covers $v$. We select the lowest qualifying tick, $p^\star$. The at-price condition protects better-priced interest. We require the long side to be able to absorb the full imbalance before we know how much IO interest will be admitted. This makes the price independent of IO. If two prices can both protect better-priced orders, we give priority to the lower qualifying price. The two-tick example in [Appendix A](A-price-and-allocation.md) shows where that convention decides the outcome. [Appendix A](A-price-and-allocation.md) proves that a qualifying tick exists.

## Admitting IO interest

At a positive imbalance let $s=0$ for long demand and $s=1$ for long supply. Let $P$ be the long side's ordinary volume at $p^\star$, and $J$ the short side's eligible IO volume there. Set $s=P=J=0$ at balance. The public clearing receipt is

$$
 R=(V_{\max},c,d,p^\star,v,s,P,J).
$$

The admitted IO quantity is $I=\min(J,v)$ and the residual imbalance is $v-I$. Each side trades $V_{\max}+I$ units before rounding. Ordinary interest at a better eligible tick and eligible ordinary interest on the short side fill in full. At positive imbalance the at-price ordinary long side fills at rate $(P-v+I)/P$, and short-side eligible IO fills at $I/J$ when $J>0$. At balance all eligible ordinary interest fills and IO receives zero. These are rational entitlements. Truncating each order's entitlement independently, as the fill helper in our Rust implementation does, may leave whole units unassigned.

## An eight-tick book

<a id="table-1"></a>

**Table 1.** A crossing book with three minimum-imbalance ticks. The sell IO at tick 4 is eligible at the selected price.

| Tick | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Ordinary bids | 0 | 0 | 3 | 0 | 0 | 10 | 2 | 0 |
| Ordinary asks | 0 | 4 | 6 | 0 | 0 | 0 | 5 | 3 |
| Sell IO | 0 | 0 | 0 | 0 | 1 | 0 | 0 | 0 |
| Demand $D$ | 15 | 15 | 15 | 12 | 12 | 12 | 2 | 0 |
| Supply $S$ | 0 | 4 | 10 | 10 | 10 | 10 | 15 | 18 |
| Ordinary volume $V$ | 0 | 4 | 10 | 10 | 10 | 10 | 2 | 0 |
| Absolute imbalance $\delta$ | 15 | 11 | 5 | 2 | 2 | 2 | 13 | 18 |

In [Table 1](02-auction-rule.md#table-1), ticks 2 through 5 all trade ten ordinary units. Tick 2 leaves five units of buy interest unmatched, while ticks 3, 4 and 5 leave two. Choosing tick 3 would put all twelve eligible bids strictly above the clearing price, although ordinary sellers offer only ten units. Tick 5 has ten bids at the tick and can absorb the imbalance. Its receipt is $(10,2,5,5,2,0,10,1)$. The one-unit sell IO raises executable volume to eleven. The two units bid above tick 5 fill, and nine of the ten units bid at tick 5 trade.

Nasdaq’s Closing Cross also ranks candidate prices by executable volume. Rule 4754(b)(2)(A)–(D) first maximizes executable eligible shares, then minimizes unmatched market-on-close and limit-on-close interest with eligible offsetting interest, including imbalance-only orders. An entered-price test and then proximity to the system bid-ask midpoint resolve remaining ties. Subparagraphs (E) and (F) specify when Nasdaq adjusts the resulting price, and Rule 4754(b)(3) sets execution priorities ([29](references.md#ref-29)). In Zeequent, ordinary interest alone determines the price. Imbalance-only interest enters afterward, when we compute entitlements at the selected tick.

<a id="crypto-background"></a>

## Verifying a clearing price on a private book

In Table 1 a reader can check the price by inspecting all eight ticks. A trader in a live batch sees their own order and the announced result. Even if the exchange discloses the depths at tick 5, those two numbers cannot establish that no other tick would trade more units. We therefore prove a statement about the *whole* committed book. On a crossing batch the public inputs identify the final commitments and the receipt. The exchange uses as its private *witness* the volumes behind the commitments, the randomness used to open them and the intermediate columns needed to check the rule.

A zero-knowledge succinct noninteractive argument of knowledge (zk-SNARK) lets a prover publish a short argument that it knows a witness satisfying a specified relation ([16](references.md#ref-16)). Completeness means an honest prover with a valid witness can convince the verifier. Knowledge soundness requires that, under the stated assumptions, a valid witness can be extracted from any efficient prover that convinces the verifier with non-negligible probability. The extractor is used in the security definition. To verify a clearing proof, an observer uses the public inputs and the verification key for that auction configuration. Verifying that the inputs came from the recorded batch also requires the log. Under the proof system’s setup model, zero knowledge requires a simulator that uses the public statement to generate a view indistinguishable from the verifier’s real view, without the private witness. In our auction the public receipt already reveals, among other things, the maximum volume and the clearing tick. Deductions from those values are part of the disclosure. For the combined order log, trader receipts and two proofs, a joint simulation remains open ([Section 5](05-security.md)).

We use a PLONK-style polynomial argument to express the clearing rule ([16](references.md#ref-16)). Each price tick occupies one row. For example, ordinary bids give one column and the demand computed from them gives another. We interpolate each column over a finite field so that evaluating its polynomial at a designated point recovers the entry in that row. A correct demand column satisfies $D_i-D_{i+1}=B_i$ before the last row, with a separate boundary condition at the last row. Similar identities enforce executable volume and price priority. In a polynomial interactive oracle proof, the verifier can query previously fixed, degree-bounded polynomials at points of its choice ([12](references.md#ref-12)). Here the prover commits to the polynomials and proves the evaluations used by the verifier.

We use the hiding form of the Kate–Zaverucha–Goldberg (KZG) polynomial commitment for this purpose ([20](references.md#ref-20)). Public setup parameters encode powers of a secret field element in a group. With those parameters the exchange can commit to a polynomial in one group element and later open it at a specified point. A pairing is a map on two groups that turns scalar multiplication in either input into exponentiation in the target group. It lets the verifier check a polynomial factorization at the secret setup point without knowing that point. This check authenticates the claimed evaluation. The verifier can thus use the last published book commitments directly in the clearing check. The hiding commitment conceals the column before opening. Since an opening discloses an evaluation, we also randomize each book polynomial away from the price rows while preserving every quantity on the grid.

The exchange combines the row identities and commits to a quotient that exists when their combination vanishes across the grid. The verifier tests the resulting identity at a challenge point outside the grid. Once the quotient is fixed, the two sides of a false identity of bounded degree can agree at only a limited number of points. Fiat–Shamir supplies each challenge by hashing the public transcript accumulated up to that round, so anyone can repeat verification after the auction closes ([15](references.md#ref-15)). The closing proof saves the verifier from reconstructing the private depths. Succinctness applies to the proof of that calculation. [Section 3](03-protocol.md) specifies the auction constraints and challenge order. [Appendix H](H-polynomial-checks.md) works through one depth constraint and the opening that authenticates a polynomial evaluation.
