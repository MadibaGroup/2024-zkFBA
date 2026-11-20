# 6 Security

We give conditional integrity and secrecy results relative to a trusted reference auction with the same clearing rule and public receipt. For integrity we allow the exchange to deviate. For secrecy we consider observers and colluding traders who see an honest exchange's transcript.

**Assumptions.** We work in the algebraic group model with the $q$-discrete-logarithm assumption over BN254, and model SHA-256 as a random oracle for the Fiat–Shamir transform and the hash chain of the board. The reference string comes from a ceremony with at least one honest participant. The exchange publishes the head digest of the board where all parties read the same value, and it publishes one clearing proof per opened book. We assume, and do not reprove, the properties of the proof system: knowledge soundness and zero knowledge of PLONK-style arguments over KZG [PLONK; Sefranek; Lipmaa], binding of the commitment [KZG], and soundness of the lookup [Haböck].

## 6.1 Properties

In the *reference auction*, a trusted exchange receives orders, acknowledges each to its trader, and publishes at the close the outcome $\mathsf{Clear}(\text{book})$ of Definition 2 (Appendix A). The public sees the number of orders, their arrival times, and the outcome. A batch auction has the verification problem of an election, with many private inputs, one public result, and an operator who computes it, and the properties we require mirror those of end-to-end verifiable voting (Table 7).

**Definition 1 (verifiable private batch clearing).** Let $\text{Book}^*$ be the sum of what the entries of the board commit to. A protocol between an exchange, traders, and a public verifier is a *verifiable private batch clearing* if an honest exchange is always accepted and, whenever the verifier accepts, the following hold except with negligible probability.

1. **Recorded as submitted.** Every order for which a trader holds a receipt that passes the trader's check is in $\text{Book}^*$, at its tick and with its quantity.
2. **No removal.** Every logged step adds a non-negative quantity at a single tick in its column, or changes only the blinding.
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

Appendix C proves that the scalar checks and clearing constraints characterize the reference rule. The secrecy argument counts the evaluations exposed by one clearing proof against the randomness sampled when the batch opens. Both results use the proof-system assumptions stated above.

## 6.3 Scope

**The exchange's own orders.** The public log does not establish who submitted an order. The exchange can add steps for which no trader holds a receipt. $\text{Book}^*$ is the traders' orders plus a non-negative book that the exchange chose. The integrity claim protects orders with receipts verified against the accepted board.

**One proof per book.** The opening and one clearing proof can expose up to three evaluations of a four-coefficient mask. A second proof at fresh points can exceed that budget. Our implementation derives the prover's randomness from its private state, so closing a batch twice reproduces the same proof, but the rule remains a requirement on the exchange.

**The proof system.** The cited soundness and zero-knowledge results are for PLONK. Our argument uses the same technique with a second prover round and with four columns committed before the proof starts. We have not proved the two properties for this exact variant, and we do not claim simulation extractability.
