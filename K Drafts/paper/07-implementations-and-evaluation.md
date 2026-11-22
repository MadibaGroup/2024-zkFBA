# 7. Implementations and measurements

We built both implementations for this research. [Appendix D](D-rust-implementation.md) details commitment reuse in our Rust implementation, and [Appendix E](E-noir-implementation.md) specifies the digest and witness checks in our Noir implementation. Our Rust implementation uses arkworks for the committed-book protocol ([2](references.md#ref-2)). It imports a ceremony reference string or generates local parameters for testing, as in our benchmarks. Pairing checks can test consistency of the supplied powers. Destruction of the setup secret is a separate assumption ([24](references.md#ref-24)). The prover commits to columns in the Lagrange basis. The prover and verifier use the same constraint routine, evaluated on cosets to construct the quotient and at the challenge point to verify it.

Our Noir implementation independently encodes the clearing rule and uses Barretenberg’s UltraHonk backend ([3](references.md#ref-3), [25](references.md#ref-25)). Our benchmark records identify the toolchain as Nargo 1.0.0 and bb 6.0.0. Its witness contains four private arrays of unsigned 32-bit quantities and a secret salt. We pack each tick's four entries into one field element and hash the sequence with a Poseidon2 sponge ([18](references.md#ref-18)). Both outcome branches publish a digest of the four arrays. For a cross, an unconstrained function proposes the eight receipt fields and the circuit checks each one. For a no-cross, the circuit checks that the ordinary depths never overlap. The resulting proof concerns a finished book. Recording orders and validating transitions are functions of our Rust implementation.

We compared outputs on 25 calibrated books, 32 replayed books and 362 books constructed from Cboe summaries. For each book, all six receipt fields exported by the benchmark in our Rust implementation agreed with their counterparts in our Noir implementation. Independent recomputation from the input volumes reproduced all eight fields in the latter output, including $P$ and $J$. All comparison books have zero IO entries. The CSV benchmark in our Rust implementation imports ordinary volume only; its order-entry workflow accepts all four columns. The worked example in [Section 2](02-auction-rule.md) exercises nonzero IO.

<a id="table-3"></a>

**Table 3.** Clearing-proof measurements on calibrated 1,000-tick books, 32-bit entries, one laptop thread. Verification in our Rust implementation includes decoding. Times for our Noir implementation cover whole commands.

| Measure | Our Rust implementation (arkworks) | Our Noir implementation (UltraHonk) |
| --- | --- | --- |
| Layout | 88 columns, 1,024 rows | 100,357 gates |
| Time to produce proof | 1.34 s | 7.43 s |
| Proof size | 6,124 B | 14,656 B |
| Verification | 29 ms | 27 ms |

Measurements were made on mains power on an AMD Ryzen 3 7320U laptop with four physical cores and eight hardware threads. We ran our Rust implementation natively on Windows 11 and our Noir implementation under WSL2. We used 20 and 10 repetitions per book, respectively, for the calibrated and replayed comparisons. [Table 3](07-implementations-and-evaluation.md#table-3) compares clearing proofs only. The 7.43 s for our Noir implementation is the sum of the witness-generation and proof-command medians. The median of the paired run totals is 7.44 s. [Appendix G](G-benchmarks.md) gives the aggregation conventions and timing distributions. Verification-key generation is excluded. On eight threads our Rust implementation’s median clearing time was 0.37 s for the 1,000-tick calibrated books. Process startup, transcript hashes and the execution environments differ, so the timing ratio cannot be attributed to arithmetization alone.

<a id="table-4"></a>

**Table 4.** After-close work in our Rust implementation on uniform books, 1,000 ticks and eight threads. The fourth column sums the two prover medians. The last column times verification on decoded artifacts.

| Orders | Update rows $M$ | Update proof | Both proofs | Batch check |
| --- | --- | --- | --- | --- |
| 250 | 1,024 | 122 ms | 0.50 s | 52 ms |
| 1,000 | 4,096 | 347 ms | 0.73 s | 118 ms |
| 2,200 | 16,384 | 1,083 ms | 1.46 s | 175 ms |

The cost of proving the closing price follows the padded price grid. Update validity follows the length of the log. [Table 4](07-implementations-and-evaluation.md#table-4) varies the latter at fixed price resolution. In all three configurations the two proofs together occupy 7,824 bytes. At 1,000 orders, update proving takes nearly as long as clearing proving. At 2,200 orders the update domain grows to 16,384 rows and update proving dominates. Announcing a provisional price at close and proving while collecting the next batch is possible only if proving resources keep pace with arrivals. These per-component measurements are insufficient to establish sustained throughput.

For the same 1,000-tick configuration, applying an order to four commitments in memory takes 0.98 ms on one thread. Checking one receipt equation takes 0.24 ms. Those timers exclude writing log entries, locating the receipt in a common history, disk access and network distribution. Similarly, the batch-check column excludes decoding and the external publication of a common digest. For the 101-tick Cboe-derived books, the median of the five-run proving medians for our Rust implementation was 384 ms on one thread and 112 ms on eight. The different number of repetitions is specific to that experiment.
