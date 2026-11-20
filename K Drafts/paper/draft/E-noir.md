# E The Noir Circuit

*Draft in Markdown. Label: `app:noir`.*

The circuit of Section 5.1 states the clearing constraints in a general-purpose proof system. It serves two purposes: it is the baseline of Section 5.5, and it is a second implementation of the rule, written from the constraints and not from the Rust code.

**Statement.** The private inputs are the book, as four arrays of $N$ 32-bit volumes, and a random salt. The public outputs are a digest of the book and the receipt, nine field elements in 288 bytes. A second circuit takes a book in which no price crosses and outputs only the digest.

**Circuit model.** Three properties of Noir shape the circuit. A value computed from the inputs by ordinary arithmetic is constrained to be that value, so the depth columns need no constraint of their own. Integer types carry range checks, and a subtraction fails the proof if its result would be negative. An *unconstrained* function runs outside the circuit, and its result is a free witness until constraints pin it. The domain, the quotient, the openings, and the lookup argument belong to the backend.

**The minimum, again.** The constraints F9–F13 of Section 3.3 become three steps. An unconstrained function returns the smaller of the two depths. The circuit subtracts it from each depth in a 64-bit unsigned type; each subtraction fails if the hint exceeds that depth, which is F9, F10, F12, and F13. It then asserts that the product of the two differences is zero, which is F11. The hint is now at most both depths and equal to one of them, whatever the function returned.

**Types.** Table 16 shows what the language's types replace. Arrays have exactly $N$ entries, so the circuit has no padding ticks and no padding constraints, and an inequality is a direct comparison, so it has no slack column and no limbs for F14 and F19.

**Table 16.** Types of the circuit and the constraints they replace.

| Quantity | Type | Replaces |
|---|---|---|
| Volume at one tick | 32-bit unsigned | F1, F2, F24, F25 |
| Depths, $Min$, surpluses, $\Delta$ | 64-bit unsigned | F12, F13, and the range of every accumulated value |
| $c$, $d$, $p^*$ | 32-bit unsigned | |
| Side | Boolean | |
| Array of length $N$ | | F3, F4, F26, F27 |

**The receipt.** The circuit takes only the book and must produce the receipt. An unconstrained function runs the clearing rule as ordinary code, and the circuit checks its output with the scalar checks and the constraints, one assertion per constraint (Table 17). By Theorem 1 the constraints accept one receipt per book, so a wrong hint cannot yield a proof for a wrong receipt; it can only fail to yield a proof. The reads at $c$, $d$, $p^*$, and their neighbours use indices that are witness values, which the backend implements as memory lookups.

**Table 17.** Steps of the clearing check in the circuit.

| Step | Assertion | Constraints |
|---|---|---|
| Receipt | $c \le p^* \le d < N$; $V_{max} \ge 1$; $V_{min\Delta} \le P$; $s$, $P$, $J$ zero when balanced | Scalar checks |
| Depth | Running sums from the top tick and from the bottom tick | F5–F8, F28–F31 |
| Each tick | The minimum; $\Delta = SurpA + SurpB$; $Min \le V_{max}$; $\Delta \ge V_{min\Delta}$ on $[c,d]$ | F9–F14, F18, F19 |
| Plateau | $Min[c] = Min[d] = V_{max}$; $Min$ strictly lower at $c - 1$ and $d + 1$ where they exist | F15–F17 |
| Clearing tick | The surpluses at $p^*$ are $V_{min\Delta}$ on the long side and 0 on the other | F20 |
| Tie-break | If $p^* > c$ and supply is long or the batch is balanced, $\Delta[p^* - 1] > V_{min\Delta}$ | F21 |
| Rationing | $P$ is the long side's volume at $p^*$; $J$ is the short side's eligible IO depth at $p^*$ | F32, F33 |

**The digest.** We pack each tick into one field element, $b + 2^{32} a + 2^{64} b^{io} + 2^{96} a^{io}$, and absorb the ticks with a sponge over the Poseidon2 permutation of width 4, with rate 3 and capacity 1. The initial state is $(\text{salt}, 0, 0, N)$, permuted once; the output is the first state element. The salt is needed because a book with little volume could otherwise be guessed and tested against the digest. The choice of hash decides the size of the circuit: a Pedersen hash of the unpacked volumes took 1.78 million gates at $N = 1000$, and the sponge takes about 25,000. The sponge is our own arrangement of the standard permutation and has not been analysed separately.

**Size.** The circuit has 12,757 gates at 100 ticks, 100,357 at 1,000, and 197,666 at 2,000, about 100 per tick. The proof is 14,656 bytes and the verification key 3,680 bytes at every size. The number of ticks is a compile-time constant, so each book size has its own verification key.

**Tests.** The library has 20 tests. Four check the receipt of a hand-computed book: demand long, supply long, balanced, and a single maximizing tick with IO orders. Twelve feed a false receipt and require that the assertion of a named constraint rejects it. The rest cover the no-cross circuit and the dependence of the digest on the book and on the salt. On the 57 benchmark books the circuit and the Rust implementation output the same receipt.

**Toolchain.** We compile with `nargo` 1.0.0 and prove with Barretenberg 6.0.0 in its zero-knowledge UltraHonk mode with a Poseidon2 transcript. The circuit is 208 lines and its tests 161.
