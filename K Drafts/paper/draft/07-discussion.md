# 7 Discussion

*Draft in Markdown. Label: `sec:discussion`. Target 0.75 page.*

**Proof latency.** The clearing rule runs in 0.07 ms, so the exchange can publish the receipt as soon as the batch closes and the proofs within the next interval. Trading need not wait for a proof. A receipt whose proof does not follow is evidence against the exchange, since the receipt and the board are both its own publications. Under this schedule the proofs of one batch are generated during the next, and the prover must keep pace on average, not finish within the interval.

**Throughput.** Each asset has its own book and its own proofs, so a venue proves its assets in parallel, one core or one machine each. Within one asset the order count is the limit, through the update-validity proof (Table 4). The design with one row per order (Section 4.3) would raise that limit fourfold.

**What the receipt discloses.** A closing auction publishes the price, the volume, and the imbalance with its side. Our receipt adds the plateau $[c,d]$, the at-price volume $P$, and the eligible IO volume $J$. $P$ and $J$ determine the fill ratios, which rationed traders observe in their own fills. The plateau is not needed by any trader; it is public because the verifier builds $Mask_P$ and $Mask_C$ from it. Hiding it would require committed selector columns and constraints on their shape.

**Trust that remains.** The exchange sees every order: we make its output verifiable and do not hide the book from it, unlike the multiparty protocols of Section 2.3. The exchange can refuse an order, which the trader notices since no receipt arrives, and it can add orders of its own (Section 6.3). Fills are integers, and we do not prove how the pro-rata ratios are rounded; the venue must state a rule. The reference string needs one honest ceremony participant.

**Not built.** The verifier is a program. An on-chain verifier in the manner of Zeeperio [Zeeperio] would check the same pairings, but it would also recompute $W$ from the logged steps, a cost linear in the orders that we have not measured as gas. We have not optimized the prover beyond the choices of Section 5.1, and we measured on one laptop.

# 8 Conclusion

A batch auction asks traders to accept a price computed from orders they cannot see. We replace that trust with a proof. We write the clearing rule of a deployed closing cross as 33 polynomial constraints that admit one receipt per book, commit to the book with a homomorphic commitment so that each order is acknowledged and none can be removed, and prove both at the close in 7.8 KB. On a laptop, the proofs for a one-second batch of 1,000 ticks and 1,000 orders take 0.73 s to generate and 0.12 s to check on eight threads. Replayed Nasdaq order flow shows that the rule's tie-breaks are reached in 3.6% of one-second batches, so they cannot be left out of the statement. Two questions remain open: hiding the book from the exchange itself, and proving the rounding of individual fills.
