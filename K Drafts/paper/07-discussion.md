# 7 Discussion

**Proof latency.** On our laptop the clearing rule takes 0.07 ms for 1,000 ticks. The exchange can publish the outcome at the close and produce the proofs during the next interval. A verifier accepts the outcome after checking both proofs. For this schedule to keep up, the exchange must provision enough proving capacity for its incoming batches.

**Throughput.** The exchange can prove different assets independently. Within an asset, the update-validity prover uses four rows per order and the verifier reads the ordered log. At 1,000 ticks and 2,200 orders, the two proofs take 1.46 s on eight threads.

**Disclosure.** In addition to price, volume, and imbalance, the receipt publishes the plateau $[c,d]$, the at-price long volume $P$, and eligible IO volume $J$. Traders use $P$ and $J$ to compute fill ratios. The verifier uses the plateau to construct public selectors. Hiding the plateau would require proving the shape of committed selectors.

**Exchange discretion.** The exchange sees the full order flow and can refuse to acknowledge an order or add orders of its own. The proofs bind the outcome to the accepted log. The fill formulas specify pro rata ratios before integer rounding. A venue must specify how it assigns any remaining units.

**Verification.** We implement a standalone verifier. On-chain verification would also have to account for computing $W$ from the log, whose cost grows with the number of steps. We have no gas measurements.

# 8 Conclusion

We adapt the Nasdaq Closing Cross to a frequent batch auction and prove its clearing rule over a committed order book. Traders verify inclusion against the update log. A public verifier checks that the logged steps are non-negative additions and that the final book yields the published receipt. At 1,000 ticks and 1,000 orders, both proofs take 0.73 s to generate on eight laptop threads. The protocol leaves order submission and integer fill rounding to the venue.
