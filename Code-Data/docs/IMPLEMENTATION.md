# Rust implementation

`zeequent-types` defines the parameters, orders, book and public receipt. `zeequent-kzg` implements commitments, openings, setup import and serialization. `zeequent-core` implements the auction and proof systems. `zeequent-cli` manages batch files and runs the benchmarks.

| File | Content |
|---|---|
| `clearing.rs` | The clearing rule, the witness columns, fills |
| `check.rs` | F1–F33 checked tick by tick over the integers: the executable specification |
| `columns.rs` | Column layout shared by prover and verifier |
| `lookup.rs` | Range checks: limb decomposition and the logarithmic-derivative lookup |
| `public_polys.rs` | Polynomials the verifier computes from the receipt |
| `constraints.rs` | The clearing and no-cross constraint systems |
| `engine.rs` | Two-round prover and verifier for any constraint system; proof serialization |
| `proof.rs` | `prove` / `verify` and `prove_no_cross` / `verify_no_cross`, bound to the book commitments |
| `book.rs` | Committed book, empty-book proof, update receipts, commitment log |
| `updates.rs` | Update-validity proof |
| `batch.rs` | `verify_batch` |

## Setup

The CLI accepts a powers-of-tau file with the G1 powers needed by both domains, the G2 powers for verification (including the book-domain power), and the blinding basis. It derives the Lagrange points by inverse FFT. For the batch workflow the required degree is `2M + 8`, with `M` the update domain. Setup consistency checks do not establish secrecy of the trapdoor. Benchmarks use locally generated test parameters.

**Commitments.** We commit directly to the evaluations in each witness column. In the Lagrange basis, $[p(\tau)]_1=\sum_i p[i]\,[L_i(\tau)]_1$ requires one size-$n$ multi-scalar multiplication, with the column entries as scalars. Our earlier coefficient-based implementation took 3.6 times as long to prove. We commit separately to the mask $\rho(X)Z_H(X)$ and the blinding term $\gamma r(X)$, then add both commitments. For columns sampled within the proof, the mask and blinding degrees equal the number of opening points. The running book commitments instead use degree-three masks fixed at opening.

**Quotient.** For the benchmark domains and column layouts, we evaluate the combined constraint polynomial on a coset of size $4n$ that avoids $H$, divide pointwise by $Z_H$, and interpolate. The large coset is a union of four cosets of $H$, and we process them one at a time with transforms of size $n$. Before each transform we reduce a masked column, whose degree slightly exceeds $n$, modulo $X^n - s$, where $s$ is the constant value of $X^n$ on that coset. On each small coset $Z_H$ is a constant and $\omega x$ is the next point, so a shifted column is a rotation. The prover stops if the interpolated quotient contains a nonzero coefficient above degree $2n+8$. This local check is separate from the verifier's identity test.

**Openings.** To open several polynomials at a point $z$, the prover combines them with powers of a challenge, divides by $X - z$, and commits to the quotient $W$; it sends one combined blinding evaluation $b$. With $C$ and $v$ the combined commitment and value, the verifier checks both opening points with two pairings:

$$e\Big(\sum_{k} r^k \big(C_k - v_k [1]_1 - b_k\, \Gamma + z_k W_k\big),\ [1]_2\Big) = e\Big(\sum_{k} r^k W_k,\ [\tau]_2\Big),$$

where $r$ is a further challenge. We evaluate the left pairing argument with a single multi-scalar multiplication over the proof commitments. This reduced verification time by about a factor of eight relative to combining the commitments pairwise.

**Public polynomials.** We store a public polynomial as its non-zero values on $H$. The verifier evaluates it at $\zeta$ from $L_i(\zeta) = \omega^i(\zeta^n - 1)/(n(\zeta - \omega^i))$ with one batch inversion, at a cost linear in the number of non-zero values: the width of the plateau for $Mask_P$, at most $2^w$ for the range table, where $w=\min(8,\log_2 n)$.

**Transcript.** We maintain a SHA-256 digest. To absorb a message, we hash the current digest with the message label, length, and contents. To derive a challenge, we reduce 64 derived bytes modulo the field size and hash the challenge back into the state. The clearing transcript includes the protocol label, grid dimensions, quantity widths, eight integer receipt fields and ordered commitments. The venue must identify the batch and supply the tick-to-price mapping separately.

**Proof format.** We serialize the column and quotient commitments, followed by two openings. Each opening includes its point, evaluations, combined blinding evaluation, and witness. A compressed curve point occupies 32 bytes. If $c$ columns are opened and $s$ also open at $\omega\zeta$, the proof occupies $32(2c+s+8)+12$ bytes. With $c=88$ and $s=7$ this gives 6,124 bytes. Decompression requires a base-field square root per point and accounts for most single-threaded verification time.

**Determinism.** At the close we derive the prover's randomness from a hash of its private state, which includes the mask and blinding of every book column. Closing the same batch twice produces the same bytes, and the tool refuses orders after a close. The one-transcript condition and its privacy implications are discussed in the security analysis.

## Public verification

`verify_batch` checks the supplied opening, verifies the hash chain, derives the ordered differences and verifies the update proof. It then checks the crossing or no-cross proof against the relevant terminal commitments. It returns the four final heads, the digest and the number of steps. A common digest and agreed venue metadata are supplied outside the library.

The [Rust README](../zeequent-rs/README.md) gives a complete CLI example. The [security analysis](SECURITY_BOOK.md) distinguishes membership of a checked transition from admission of every submitted order.
