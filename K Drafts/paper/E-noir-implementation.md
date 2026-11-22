# Appendix E. Our Noir implementation

## Public statement for a finished book

We use a separate crossing circuit and no-cross circuit. Both take a private salt and four arrays of $N$ unsigned 32-bit quantities. For tick $i$ we encode ordinary bids, ordinary asks, IO bids and IO asks in that order:

$$
 x_i=B_i+2^{32}A_i+2^{64}B_i^o+2^{96}A_i^o .
$$

Each packed value is below $2^{128}$, so these four lanes do not overlap or wrap in the circuit field. The digest initializes a four-element state with the salt and $N$, applies the Poseidon2 permutation ([18](references.md#ref-18)), then absorbs three packed ticks per permutation. The first state element is the public digest. Both branches bind all four arrays through this computation, even though the no-cross predicate uses only ordinary volumes. Treating the digest as a binding and hiding representation requires assumptions about the sponge construction and a suitably random secret salt. We leave the security analysis of this particular salted sponge construction open.

## Which computations constrain the result

For a cross an unconstrained routine searches the book and proposes the receipt. This is witness generation: the proof is accepted only if the constrained checks on that receipt succeed. The circuit accumulates ordinary and IO depths in unsigned 64-bit integers. The minimum at each tick is also proposed as a hint. The two unsigned surplus subtractions and their zero-product check force the proposed minimum to equal the smaller depth. The circuit then checks the volume ceiling, both plateau endpoints, the neighbouring drops, the imbalance floor and the selected tick. It applies the strict predecessor check on precisely the branches in [Section 3](03-protocol.md), and checks $P$ and $J$ against the book at positive imbalance.

The no-cross circuit instead checks the product of ordinary demand and supply at every tick. Its only public return value is the digest. The verifier needs the expected digest, price grid and verification key for the intended branch and size. A valid proof for a digest chosen by the prover at close gives no evidence that a trader's earlier submission was in the book.

## What the independent implementation checks

The two implementations encode the same integer auction rule using different proof representations. Tests in our Noir implementation include balanced, demand-long and supply-long examples and receipts with incorrect endpoints, side, tie selection or IO quantity. These cases exercise branches absent from the zero-IO evaluation corpus. Agreement on output fields across implementations checks the arithmetic at the level of the auction. The proof formats and book commitments differ, and the standalone circuit has no authenticated connection to the update history in our Rust implementation. The security proposition for a trader's recorded contribution consequently applies to the full protocol in [Appendix C](C-security-proofs.md).
