# Independent Noir implementation

**Statement.** The witness consists of four arrays of $N$ unsigned 32-bit volumes and a salt. The crossing circuit publishes nine field elements (288 bytes) for the book digest and eight receipt values. The no-cross circuit publishes only the digest. It hashes all four arrays and checks the absence of an ordinary cross. The Rust no-cross proof uses only the two ordinary final commitments.

**Circuit model.** Noir constrains the result of ordinary arithmetic, including the sums used to compute depth. Its unsigned integer types enforce bounds, so a subtraction that would underflow cannot satisfy the circuit. An unconstrained function can supply a witness value, but the circuit must still check that value. The backend implements the polynomial domain, quotient, openings, and lookups.

**Executable volume.** An unconstrained function proposes the minimum of the two depths. The circuit subtracts this value from each depth in unsigned 64-bit arithmetic, enforcing the differences and their non-negativity (F9, F10, F12, F13). It then checks that their product is zero (F11). Together the checks force the proposed value to equal the smaller depth.

**Types.** The first table maps Noir types to the constraints they enforce. Arrays have exactly $N$ entries. Native comparisons implement the inequalities in F14 and F19 without an explicit slack column or limb witnesses at the circuit-language level.

Types of the circuit and the constraints they replace.

| Quantity | Type | Replaces |
|---|---|---|
| Volume at one tick | 32-bit unsigned | F1, F2, F24, F25 |
| Depths, $Min$, surpluses, $\Delta$ | 64-bit unsigned | F12, F13, and the range of every accumulated value |
| $c$, $d$, $p^*$ | 32-bit unsigned | |
| Side | Boolean | |
| Array of length $N$ | | F3, F4, F26, F27 |

**Receipt.** An unconstrained function computes a candidate receipt from the book. The circuit then applies the scalar and per-tick checks in the table below. By the [clearing-rule argument](CORRECTNESS_PROOF.md), every accepted candidate equals the rule’s receipt. The backend handles reads at $c$, $d$, $p^*$, and their neighbours as memory lookups because those positions depend on the witness.

Steps of the clearing check in the circuit.

| Step | Assertion | Constraints |
|---|---|---|
| Receipt | $c \le p^* \le d < N$; $V_{max} \ge 1$; $V_{min\Delta} \le P$; $s$, $P$, $J$ zero when balanced | Scalar checks |
| Depth | Running sums from the top tick and from the bottom tick | F5–F8, F28–F31 |
| Each tick | The minimum; $\Delta = SurpA + SurpB$; $Min \le V_{max}$; $\Delta \ge V_{min\Delta}$ on $[c,d]$ | F9–F14, F18, F19 |
| Plateau | $Min[c] = Min[d] = V_{max}$; $Min$ strictly lower at $c - 1$ and $d + 1$ where they exist | F15–F17 |
| Clearing tick | The surpluses at $p^*$ are $V_{min\Delta}$ on the long side and 0 on the other | F20 |
| Tie-break | If $p^* > c$ and supply is long or the batch is balanced, $\Delta[p^* - 1] > V_{min\Delta}$ | F21 |
| Rationing | For positive imbalance, $P$ is the long side's volume at $p^*$; $J$ is the short side's eligible IO depth at $p^*$ | F32, F33 |

**Digest.** We pack the four volumes at each tick into $b+2^{32}a+2^{64}b^{io}+2^{96}a^{io}$. A width-four Poseidon2 sponge with rate three and capacity one absorbs the resulting field elements. We initialize the state to $(\text{salt},0,0,N)$, permute once before absorption, and take the first state element as the output after absorption. A secret, high-entropy salt is intended to prevent an observer from testing a guessed low-volume book against the digest without also guessing the salt. At 1,000 ticks a Pedersen hash of the unpacked volumes required 1.78 million gates. Our sponge uses about 25,000. This arrangement of the standard permutation has not received a separate security analysis.

**Size.** The circuit has 12,757 gates at 100 ticks, 100,357 at 1,000, and 197,666 at 2,000, about 100 per tick. The proof is 14,656 bytes and the verification key 3,680 bytes at every size. The number of ticks is a compile-time constant, so each book size has its own verification key.

The [Noir README](../zeequent-noir-standalone/README.md) describes the entry points, CSV conversion and proof scripts. The Rust `export-noir` command writes a legacy format.
