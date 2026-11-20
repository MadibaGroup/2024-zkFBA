# 6 Security

*Draft in Markdown. Label for cross-references: `sec:security`. Target 1.25 pages. Proofs are in Appendix C.*

We compare the zk-FBA with an auction run by a trusted exchange, and show that it is as correct without the trust and reveals no more. The exchange is the adversary for integrity: it sees every order and may deviate arbitrarily. Secrecy is against observers and traders who may collude; the exchange sees the book in any auction.

**Assumptions.** We work in the algebraic group model with the $q$-discrete-logarithm assumption over BN254, and model SHA-256 as a random oracle for the Fiat–Shamir transform and the hash chain of the board. The reference string comes from a ceremony with at least one honest participant. The exchange publishes the head digest of the board where all parties read the same value, and it publishes one clearing proof per opened book. We assume, and do not reprove, the properties of the proof system: knowledge soundness and zero knowledge of PLONK-style arguments over KZG [PLONK; Sefranek; Lipmaa], binding of the commitment [KZG], and soundness of the lookup [Haböck].

## 6.1 Properties

In the *reference auction*, a trusted exchange receives orders, acknowledges each to its trader, and publishes at the close the outcome $\mathsf{Clear}(\text{book})$ of Definition 2 (Appendix A). The public sees the number of orders, their arrival times, and the outcome. A batch auction has the verification problem of an election, with many private inputs, one public result, and an operator who computes it, and the properties we require mirror those of end-to-end verifiable voting (Table 7).

**Definition 1 (verifiable private batch clearing).** Let $\text{Book}^*$ be the sum of what the entries of the board commit to. A protocol between an exchange, traders, and a public verifier is a *verifiable private batch clearing* if an honest exchange is always accepted and, whenever the verifier accepts, the following hold except with negligible probability.

1. **Recorded as submitted.** Every order for which a trader holds a receipt that passes the trader's check is in $\text{Book}^*$, at its tick and with its quantity.
2. **No removal.** Every entry of the board adds a non-negative quantity at a single tick.
3. **Cleared as recorded.** The published outcome is $\mathsf{Clear}(\text{Book}^*)$.
4. **No discretion.** The board determines the outcome.
5. **Order secrecy.** The view of the observers and of any set of colluding traders can be simulated from their view of the reference auction: the number and arrival times of the orders, the outcome, and the colluding traders' own orders.

**Table 7.** The properties of Definition 1.

| Property | Election counterpart | Checked by | Rests on |
|---|---|---|---|
| Recorded as submitted | Recorded as cast | Each trader | The receipt equation; binding |
| No removal | No ballot removed | Anyone | The update-validity proof |
| Cleared as recorded | Counted as recorded | Anyone | The clearing proof; Theorem 1 |
| No discretion | A unique tally | Anyone | $\mathsf{Clear}$ is a function defined on every book |
| Order secrecy | Ballot secrecy | | Uniform board entries; zero knowledge; masked book columns |

## 6.2 Results

**Theorem 3 (integrity).** Under the assumptions above, if the verifier accepts a batch, properties 1 to 4 hold except with probability at most

$$\varepsilon \le \varepsilon_{\mathrm{KZG}} + \varepsilon_{\mathrm{clear}} + \varepsilon_{\mathrm{upd}} + \varepsilon_{\mathrm{hash}},$$

where $\varepsilon_{\mathrm{KZG}}$ bounds a break of the commitment, $\varepsilon_{\mathrm{hash}}$ a collision in the chain, and $\varepsilon_{\mathrm{clear}} \le (2E + 7)\,n/|\mathbb{F}|$ and $\varepsilon_{\mathrm{upd}} \le (2E_Q + 12)\,M/|\mathbb{F}|$ are the errors of the two polynomial protocols per random-oracle query. For our parameters the last two are below $2^{-230}$ (Appendix C.7).

**Theorem 4 (order secrecy).** Under the assumptions above, the zk-FBA satisfies property 5: what it publishes beyond the reference auction, the opening, the board, the receipts of colluding traders, and the proofs, can be simulated from the view of the reference auction.

Each proof has two layers. The cryptographic layer says that an accepted proof implies that the constraint polynomials vanish, and that the transcript reveals nothing else; we take it from the literature. The semantic layer says what the vanishing means for the auction, and it is ours. For integrity, it is Theorem 1: the scalar checks and the constraints F1–F33 hold if and only if the receipt is $\mathsf{Clear}$ of the book in the committed columns, an argument over the integers. For secrecy, it is a count: the mask of a book column is fixed at the opening, before any proof, and we show that it still leaves the opened evaluations uniform.

## 6.3 Scope

**The exchange's own orders.** A board entry without a receipt holder is an order of the exchange. $\text{Book}^*$ is the traders' orders plus a non-negative book that the exchange chose. This is true of any venue; what the proof removes is the ability to drop or alter an order after acknowledging it.

**One proof per book.** After one clearing proof, the mask of a book column has one spare coefficient. A second, different proof for the same book would reveal more evaluations than the mask covers. Our implementation derives the prover's randomness from its private state, so closing a batch twice reproduces the same proof, but the rule remains a requirement on the exchange.

**The proof system.** The cited soundness and zero-knowledge results are for PLONK. Our argument uses the same technique with a second prover round and with four columns committed before the proof starts. We have not proved the two properties for this exact variant, and we do not claim simulation extractability.
