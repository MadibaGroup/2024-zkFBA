# Appendix D. Our Rust implementation

## Reusing the book commitments

Our Rust implementation keeps private polynomial openings alongside the four public column commitments. This lets us use the last commitment of a column in the closing proof without recommitting its contents. The distinction matters even if two commitments encode identical volumes: a fresh blinding would generally produce a different public point. The crossing verifier compares the four designated proof commitments with the four heads obtained from the log. The no-cross verifier performs the comparison for the two ordinary heads. In both branches the update verifier derives its weighted commitment from the logged differences.

The batch verifier checks the empty opening, replays the log and verifies the update and outcome proofs. It returns the final commitments, the digest and the number of steps. A caller must compare that digest with the one published for the batch. The step count can be converted into an order count under the honest four-entry writing procedure. Successful proof verification alone gives no count of independently authenticated traders or submissions.

## Evaluating the constraints

We use the same routine to combine the constraints at a point on both sides of verification. The prover evaluates it across four cosets of the size-$n$ domain to interpolate the quotient. The verifier evaluates it at the transcript challenge $\zeta$ using the opened values. At each point on a coset, the value at the next price row is a rotation on that coset. The verifier obtains the corresponding shifted values at $\omega\zeta$. This arrangement implements the demand and supply recurrences without a separate opening proof for each tick.

The public selectors also have a cost. For example, the plateau selector at $\zeta$ is $\sum_{i=c}^d L_i(\zeta)$. Its evaluation depends on the number of selected ticks even when the proof contains the same number of group elements. At batch verification the multi-scalar multiplication for the update argument additionally depends on the number of entries. These are two reasons why proof size alone cannot predict the verifier's running time.

## Reading the proof-size measurements

On the 1,000-tick, 32-bit crossing configuration we open 88 columns at $\zeta$ and seven of those again at $\omega\zeta$. The 88 columns consist of 13 value columns, 46 limb columns and 29 lookup columns. The quotient has its own commitment. Thus a count of 89 column-and-quotient commitments in the benchmark and a layout of 88 columns in [Section 7](07-implementations-and-evaluation.md) describe the same proof. The serialized proof also carries evaluations, blinding evaluations and opening witnesses. Its total length is 6,124 bytes. The update proof is a separate 1,700-byte artifact in this configuration. Neither number includes the history the verifier must read.

The proving and verification keys used in the timings came from local test parameters. The implementation also supports importing ceremony powers. Consistency checks verify algebraic relations among powers. Security of a powers-of-tau ceremony also depends on honest participation and protection of secret randomness ([24](references.md#ref-24)). For the end-to-end claim both the setup and the shared-commitment assumptions in [Appendix C](C-security-proofs.md) apply.
