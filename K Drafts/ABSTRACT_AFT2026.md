# AFT 2026 -- Submission Abstract

**Title:** Zeequent: Verifiable Frequent Batch Auctions via Zero-Knowledge Polynomial Commitments

---

## Abstract

Frequent batch auctions (FBAs) have emerged as a promising market design
for reducing latency-based advantages in equity trading, yet their adoption
is hindered by a fundamental trust problem: participants must accept the
exchange's clearing computation on faith, with no means of verifying that
the published price and volume were honestly derived from the submitted
orders. We present **Zeequent**, a zero-knowledge protocol that resolves
this tension by enabling an exchange to prove, for every auction round, that
the market clearing volume and uniform clearing price were computed correctly
from the committed order book, without revealing any individual order.

Zeequent is constructed as a specialized Polynomial Interactive Oracle Proof
(PIOP) for the FBA market-clearing procedure. We design a custom
arithmetization around five vanishing constraints that jointly certify
forward ask-depth accumulation, backward bid-depth accumulation, and
min-exclusivity of the clearing volume candidates. These constraints admit
a compact 32-point evaluation domain, with
polynomial commitments produced via a batched Kate-Zaverucha-Goldberg (KZG)
quotient construction and a non-interactive Fiat-Shamir transform. The
verifier accepts the proof and confirms correctness using four BN254 pairings.

To evaluate the cost of general-purpose zk tooling relative to
protocol-specific arithmetization, we implement Zeequent in two forms: a
hand-specialized prover in Rust using the arkworks cryptographic library,
and a compiled circuit in Noir targeting the Barretenberg UltraHonk backend.
The specialized Rust prover achieves an end-to-end prove-plus-verify latency
of 27.5 ms on commodity hardware (Apple M4 Max), while the Noir circuit
requires 93.4 ms, a 3.4x overhead attributable to UltraHonk's general
constraint system operating over a 128x larger polynomial domain (4,096 vs
32 evaluation points). Proof generation alone is 7.7x slower in Noir (81.1
ms vs 10.6 ms), whereas pairing verification is 1.3x faster (12.3 ms vs
15.7 ms) due to Barretenberg's optimized BN254 implementation. These results
show that, at the sub-100 ms latency regime required for practical batch
auction systems, custom arithmetization retains a decisive performance
advantage over compiled ZK circuits, while the compiled approach provides
superior security defaults and deployment ergonomics. We conclude with a
discussion of the trade-offs between proof size, proving latency, trusted
setup assumptions, and auditor accessibility in the context of regulated
financial market infrastructure.

---

*Keywords:* zero-knowledge proofs, frequent batch auctions, KZG commitments,
PIOP, market microstructure, verifiable computation, Noir, Barretenberg,
UltraHonk, custom arithmetization
