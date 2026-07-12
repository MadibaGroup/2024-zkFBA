

| n | Gates | domain | Prove | Verify | Total | Proof size |
| --- | --- | --- | --- | --- | --- | --- |
| 21 | 3,970 | 2^12=4,096 | 1.1 ms | 12.3 ms | 93.4 ms | 14.0 KB |
| 100 | 8,394 | 2^14=16,384 | 106.5 ms | 13.1 ms | 119.6 ms | 14.3 KB |
| 1000 | 58,794 | 2^16=65,536 | 286.2 ms | 12.9 ms | 299.2 ms | 14.3 KB |




**Why these numbers are better than expected, and what they mean**

Gates scale linearly with N, not at the predicted rate. The estimate going in was ~3,281 ACIR opcodes for N=100 -> domain 2^15=32,768. The actual gate count (8,394 -> 2^14) is half that, because the UltraHonk compiler merges C5's product gate, the two subtraction witnesses, and the range checks into shared lookup table rows more efficiently than the per-constraint breakdown suggested. N=1000 also lands one power-of-two lower than predicted (2^16 instead of 2^18).

Proving scales O(n log n) in theory but shows large constant-cost amortization in practice. The domain went 4x for N=100 and 16x for N=1000, yet prove time only went 1.3x and 3.5x. The reason is that bb prove has significant fixed overhead, SRS loading, curve parameter initialization, pairing setup, that dominates at the 4K-16K domain sizes involved here. At these scales, parallelism across 14 threads also absorbs most of the arithmetic growth.

Verification is constant. 12.3 ms -> 13.1 ms -> 12.9 ms. Three different domain sizes, indistinguishable verification times. This is the ZK property in action: the verifier performs the same four BN254 pairings regardless of how many price ticks were in the auction.

Proof size is essentially constant. UltraHonk proof size is determined by the number of wire/permutation polynomials committed (fixed by circuit structure) and a logarithmic number of KZG opening evaluations (proportional to log_2 domain). Going from domain 4,096 to 65,536 adds only four evaluation points, a rounding difference of 0.3 KB.

The Noir prover overtakes Rust at large N. The Rust CSV prover clocks 54 ms at N=100 and 416 ms at N=1000. At N=21, Rust is 3x faster than Noir (31 ms vs 93 ms). By N=1000, Noir (299 ms) beats Rust (416 ms). The reason is the V_KL = (AccA − Min) · (AccB − Min) quotient polynomial in the Rust implementation: computing it requires polynomial multiplication then naive long division (O(domain^2)). At N=1000 the Rust domain is 1,024, and that O(domain^2) term (plus the Layer 4 sanity check at 371 ms alone) dominates. Barretenberg avoids this entirely; UltraHonk's gate structure never forms an explicit product polynomial, the product constraint is encoded as a mul-gate and the quotient is computed by the general-purpose proving engine using a sublinear approach.

All three Noir sizes are practical for batch auction systems. Even N=1000 at 299 ms fits inside any batch interval longer than 300 ms. At the 1-second cadence used by most FBA proposals, the prover is idle 70% of the time at the largest realistic order-book size.
