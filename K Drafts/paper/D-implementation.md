# D Implementation Details

**Workspace.** The code is a Cargo workspace on arkworks 0.4, with `sha2` and, optionally, `rayon` for multi-threading. Table 13 lists its crates.

**Table 13.** Crates of the Rust implementation. Lines exclude blanks and comments.

| Crate | Content | Lines |
|---|---|---|
| `zeequent-types` | Parameters, orders, the plaintext book, the receipt | 237 |
| `zeequent-kzg` | Commitments, openings, transcript, setup import, file codec | 754 |
| `zeequent-core` | Clearing rule, constraints, prover, verifier, committed book, update validity | 2,596 |
| `zeequent-cli` | The command-line tool | 1,090 |
| Tests | 44 tests | 1,459 |

**Setup.** We read the reference string from a powers-of-tau ceremony file: $[\tau^i]_1$ for $i \le 2M + 8$, $[1]_2$, $[\tau]_2$, and the file's $[\alpha\tau^i]_1$ section as the blinding basis, so $\gamma$ is the ceremony's $\alpha$. From the powers we derive what depends on the domain: the Lagrange-basis points $[L_i(\tau)]_1$, by an inverse FFT over group elements, and $[Z_H(\tau)]_2$. A verifier that receives a setup from another party checks with pairings that its points are powers of one $\tau$ and that the Lagrange points match them. Secrecy of $\tau$ rests on the ceremony.

**Commitments.** We commit directly to the witness evaluations in the Lagrange basis, $[p(\tau)]_1 = \sum_i p[i]\,[L_i(\tau)]_1$: one multi-scalar multiplication of size $n$ per column, with scalars as small as the column's values. Committing from coefficients made the prover 3.6 times slower in our first version. The mask $\rho(X) Z_H(X)$ and the blinding $\gamma\, r(X)$ are committed separately and added, and $\deg \rho = \deg r$ equals the number of points at which the column is opened.

**Quotient.** The combined constraint polynomial has degree up to $3n + 3$, so we evaluate it on a coset of size $4n$ that avoids $H$, divide pointwise by $Z_H$, and interpolate. The large coset is a union of four cosets of $H$, and we process them one at a time with transforms of size $n$. Before each transform we reduce a masked column, whose degree slightly exceeds $n$, modulo $X^n - s$, where $s$ is the constant value of $X^n$ on that coset. On each small coset $Z_H$ is a constant and $\omega x$ is the next point, so a shifted column is a rotation. If the constraints do not hold, the quotient has non-zero coefficients above degree $2n + 8$ and the prover stops.

**Openings.** To open several polynomials at a point $z$, the prover combines them with powers of a challenge, divides by $X - z$, and commits to the quotient $W$; it sends one combined blinding evaluation $b$. With $C$ and $v$ the combined commitment and value, the verifier checks both opening points with two pairings:

$$e\Big(\sum_{k} r^k \big(C_k - v_k [1]_1 - b_k\, \Gamma + z_k W_k\big),\ [1]_2\Big) = e\Big(\sum_{k} r^k W_k,\ [\tau]_2\Big),$$

for a further challenge $r$. The left argument is one multi-scalar multiplication over all commitments of the proof. Using one multi-scalar multiplication sped up verification by about eightfold in our implementation.

**Public polynomials.** We store a public polynomial as its non-zero values on $H$. The verifier evaluates it at $\zeta$ from $L_i(\zeta) = \omega^i(\zeta^n - 1)/(n(\zeta - \omega^i))$ with one batch inversion, at a cost linear in the number of non-zero values: the width of the plateau for $Mask_P$, 256 for the range table.

**Transcript.** The transcript state is a SHA-256 digest. Absorbing hashes the state with a label, a length, and the data; a challenge is 64 derived bytes reduced modulo the field size and hashed back into the state. The transcript absorbs a protocol label, the parameters, the receipt, and every commitment in order, so each challenge depends on the whole statement and on everything sent before it.

**Proof format.** A proof is the column commitments, the quotient commitment, and two openings, each with its point, evaluations, combined blinding evaluation, and witness. Points are compressed to 32 bytes. With $c$ columns of which $s$ are also opened at $\omega\zeta$, the size is $32\,(2c + s + 8) + 12$ bytes: 6,124 for $c = 88$ and $s = 7$. Decompressing a point costs a square root in the base field, which is why decoding dominates verification on one thread.

**Determinism.** At the close we derive the prover's randomness from a hash of its private state, which includes the mask and blinding of every book column. Closing the same batch twice produces the same bytes, and the tool refuses orders after a close. This implements the one-proof rule of Section 6.3 for an honest exchange.

**Batch files.** The command-line tool keeps a batch in a directory (Table 14) and has one command per protocol step: `setup`, `open`, `order`, `check-receipt`, `close`, `verify`, and `fill`.

**Table 14.** Files of one batch, for 32-bit volumes.

| File | Content | Public | Size |
|---|---|---|---|
| Opening | Four commitments with their emptiness proofs | Yes | 1,576 B |
| Board | The log | Yes | 416 B per order |
| Update receipt | Column, tick, quantity, blinding, commitment before and after | To its trader | 192 B |
| Outcome | The clearing receipt, or no-cross | Yes | About 100 B |
| Clearing proof | | Yes | 6,124 B |
| Update-validity proof | | Yes | 1,700 B |
| State | Plaintext book, masks, blindings, update history | No | Grows with the orders |

**Tests.** Table 15 lists the tests that support the claims of Appendix C. We model a cheating prover by switching off the prover's own checks, so that it outputs a proof for a false statement, which the verifier must reject.

**Table 15.** Claims and the tests that support them.

| Claim | Test |
|---|---|
| The code's clearing rule is Definition 2 | 2,000 random books; the receipt is recomputed from the definition by direct search |
| The constraints admit one receipt (Theorem 1) | 300 random books of 6 ticks; every receipt with values up to the total volume is tried, and exactly one is accepted |
| The polynomial system equals the integer constraints | Receipt values and witness cells are changed one at a time and proved with the prover's checks off; the verifier rejects each |
| Every tie-break case proves and verifies | One book per case of Appendix A.2, and a padded domain |
| A crossing book cannot be declared no-cross | Direct test |
| A logged step cannot cancel an order (no removal) | The attack of Section 4.3 is run; no update-validity proof verifies, whatever quantity and tick the exchange claims |
| A repeated close reveals nothing new | The two proofs are compared byte for byte |
| Range checks are exact | An 8-bit quantity in 3-bit limbs, a width that is not a multiple of the limb: values in range pass and values out of range fail |
