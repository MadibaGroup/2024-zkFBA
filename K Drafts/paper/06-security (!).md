# 6 Security

We compare the protocol with a trusted exchange running the same clearing rule. For integrity, we allow the exchange to deviate arbitrarily. For secrecy, we consider what observers and colluding traders learn from an honest exchange’s publications and their own receipts. The exchange receives the orders in plaintext.

**Assumptions.** We work in the algebraic group model under the $q$-discrete-logarithm assumption over BN254. We model SHA-256 as a random oracle for Fiat–Shamir and the board’s hash chain. The setup ceremony has at least one honest participant, all parties see the same published head digest, and each opened book yields at most one distinct clearing proof. We assume knowledge soundness and zero knowledge for the PLONK-style argument used here [22, 30, 44], binding for KZG [29], and soundness for the lookup [26]. Section 6.3 explains where our protocol differs from the cited constructions.

## 6.1 Properties

In the *reference auction*, a trusted exchange acknowledges each order to its trader and publishes $\mathsf{Clear}(\text{book})$ at the close (Definition 2, Appendix A). Observers see the number and timing of arrivals and the outcome. As in end-to-end verifiable voting, the party who submits a private input checks its inclusion, while anyone can check the computation on the recorded inputs (Table 7).

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

**Theorem 4 (order secrecy).** Under the stated assumptions, the reference view suffices to simulate the additional information available to observers and colluding traders, including the opening, log, colluding traders’ receipts, and proof transcripts.

For integrity, Theorems 1 and 2 relate the polynomial constraints to the integer clearing rule. For secrecy, the book commitments require particular care because the exchange fixes their masks at the opening, before it knows the proof challenges. Appendix C counts the evaluations exposed by a single close and states the proof-system assumptions on which the simulation relies.

## 6.3 Scope

**Exchange orders.** An accepted board can contain contributions for which no trader holds a receipt. Thus $\text{Book}^*$ can include additional non-negative volume chosen by the exchange. The inclusion guarantee applies to orders whose receipts traders have checked against the board.

**Repeated closes.** A second proof at fresh challenge points could expose more evaluations than the fixed book mask covers. The implementation derives proving randomness from the private batch state so that closing the same batch twice reproduces the same proof.

**Scope of the proof-system assumptions.** Our variant adds a lookup round and carries four existing book commitments into the proof. Knowledge soundness and zero knowledge for this exact variant are assumptions in our analysis. Simulation extractability is outside its scope.
