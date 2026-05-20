# Noir vs Rust for FBA Zero-Knowledge Proofs
## Pros, Cons, and Recommendation

**Author:** Kimia Esmaili, Concordia University

This document compares the two ZK proof implementations of the Frequent Batch
Auction (FBA) protocol in this repository:

- `../zk_fba` -- hand-rolled PLONK in Rust using the arkworks cryptographic
  library suite
- `../zk_fba_noir` -- Noir arithmetic circuit targeting the Barretenberg
  UltraHonk proof system

Both implementations prove the same five auction constraints over the same
BN254 elliptic curve, using the same 21-element auction dataset.  They differ
in everything else: language, proof system architecture, domain size,
development model, and production readiness.

All benchmark figures are from the Apple M4 Max (14-core, 36 GB, macOS
Sequoia 15.3) described in each project's README.

---

## 1. What Each Implementation Is

### Rust / arkworks (`zk_fba`)

A fully hand-written polynomial proof system.  The developer manually
implements every layer:

```
Layer 1   Array computation       bid depth, ask depth, min (MCV)
Layer 2   Polynomial interpolation  5 witness polynomials over BN254 Fr
Layer 3a  KZG trusted setup         simulated Powers-of-Tau SRS
Layer 3b  KZG witness commits       5 Pippenger MSMs
Layer 3c  Quotient polynomials      V(X)/D(X) = Q(X), zero remainder
Layer 3d  KZG quotient commits      5 more MSMs
Layer 3e  Fiat-Shamir transform     SHA-256 transcript hashing
Layer 3f  KZG opening proofs        13 MSMs + pairing verification
Layer 4   Constraint verification   direct algebraic check
```

The entire cryptographic pipeline is visible, inspectable, and tuned for
exactly this circuit (5 polynomials, 32-point domain, 21 data values).

### Noir / Barretenberg (`zk_fba_noir`)

A declarative circuit expressed as arithmetic assertions.  The developer
writes only the constraints:

```noir
assert(acc_a[0] == ask[0]);                          // C1
assert(acc_a[i] == acc_a[i-1] + ask[i]);             // C2, i=1..20
assert(acc_b[20] == bid[20]);                         // C3
assert(acc_b[i] == acc_b[i+1] + bid[i]);             // C4, i=0..19
assert((acc_a[i]-min[i] as Field) * (acc_b[i]-min[i] as Field) == 0);  // C5
```

Barretenberg compiles these to UltraHonk, then handles all polynomial
arithmetic, KZG commitments, Fiat-Shamir, opening proofs, and pairing
verification automatically.

---

## 2. Benchmark Numbers at a Glance

| Metric | Rust | Noir |
|---|---|---|
| Polynomial domain size | 32 points (2^5) | 4,096 points (2^12) |
| Number of polynomials | 10 (5 witness + 5 quotient) | ~30+ (all UltraHonk wire/permutation columns) |
| Proving threads | 1 | 14 |
| Witness gen | 0.067 ms | 82.7 ms (mean, 20 samples) |
| Proof generation | 10.56 ms | 81.1 ms (mean, 20 samples) |
| Pairing verification | 15.71 ms | 12.3 ms (mean, 20 samples) |
| **Prove + verify (total)** | **27.52 ms** | **93.4 ms** |
| Proof size | ~1,200 bytes | 14,656 bytes |
| Lines of cryptographic code | ~1,134 | ~0 (Barretenberg handles it) |
| Lines of constraint code | ~1,134 (mixed) | ~135 (pure constraints) |
| ACIR opcodes | N/A | 689 |
| UltraHonk gates | N/A | 3,970 (domain: 4,096) |

---

## 3. Noir Advantages

### 3.1 Dramatically Less Cryptographic Code

The Rust implementation is ~1,134 lines of dense cryptographic code across
`src/lib.rs` and `src/main.rs`.  The Noir circuit is ~135 lines of
constraint assertions across three readable source files.

If you need to add a new FBA constraint -- say, a price impact limit or a
collateral check -- you write one loop in Noir.  In Rust you need a new
polynomial, a new quotient computation, a new commitment, a new Fiat-Shamir
term, a new opening proof, and a new pairing check.

### 3.2 Production Security by Default

Three security gaps that are acceptable in a research prototype are closed
automatically in Noir:

| Gap | Rust prototype | Noir / Barretenberg |
|---|---|---|
| Trusted setup | Simulated random tau (toxic waste known to the prover) | Ignition multi-party ceremony SRS; tau destroyed as long as one of hundreds of participants was honest |
| Proof blinding | Absent -- polynomial coefficients could be recoverable from evaluations under a strong adversary model | Barretenberg adds blinding factors automatically |
| Prover/verifier separation | Combined: prover and verifier run in the same process with shared memory | `bb prove` and `bb verify` are separate binaries; the verifier needs only proof + VK + public inputs, nothing else |

A production deployment using the Rust proof would need to:
1. Run its own KZG trusted setup ceremony
2. Implement proof blinding
3. Expose a clean verifier API that accepts only a serialised proof

Barretenberg provides all three out of the box.

### 3.3 Human-Readable Constraint Specification

The five FBA constraints in `fba_accumulators.nr` and `fba_mcv.nr` are
English-like assertions that a compliance officer, regulator, or external
auditor can read and cross-reference against the exchange rulebook:

```
acc_a[0] == ask[0]                     -- accumulator seeded at tick 0
acc_a[i] == acc_a[i-1] + ask[i]        -- forward recurrence
acc_b[20] == bid[20]                   -- accumulator seeded at tick 20
acc_b[i] == acc_b[i+1] + bid[i]        -- backward recurrence
(da as Field) * (db as Field) == 0     -- min-exclusivity product gate
min_arr[i] <= mcv for all i            -- MCV upper bound
exists j: min_arr[j] == mcv            -- MCV achieved
```

The Rust cryptographic layers are not auditable in this sense -- the
mathematical correctness of the quotient polynomial computation or the Fiat-
Shamir transcript encoding requires deep cryptographic expertise to verify.

### 3.4 Automatic Proof System Upgrade Path

The Noir source code is decoupled from the proof system.  If a vulnerability
is found in UltraHonk, Aztec can patch Barretenberg and the existing circuit
recompiles unchanged.  If a vulnerability were found in the Rust PLONK
variant, every layer of the cryptographic code would need to be rewritten.

### 3.5 Standard Proof Format and Ecosystem

The Barretenberg proof format is shared across the Aztec ecosystem.
Third-party audit tooling, smart contract verifier contracts (Solidity
verifiers for on-chain verification of Barretenberg proofs), and formal
verification tools can all consume it without modification.  The Rust proof
is a custom binary format with no external tooling.

### 3.6 Verification Is Faster

Despite the larger domain, the Noir verifier (12.3 ms) is 1.3x faster than
the Rust verifier (15.71 ms).  Barretenberg's BN254 pairing implementation
is more optimised than the arkworks one.  The pairing check is the operation
that would eventually be run on-chain or by a third-party clearinghouse, so
this is the number that matters most for deployed systems.

---

## 4. Noir Disadvantages

### 4.1 Slower Proof Generation

`bb prove` takes 81 ms vs 10.6 ms for the Rust prover -- a 7.7x slowdown.

The entire gap is explained by domain size.  The Rust circuit uses exactly
32 evaluation points (the minimum viable domain for 21 data values).
UltraHonk uses 4,096 points (the next power of two above the 3,970-gate
circuit).  That is 128x more polynomial evaluation points.

For polynomial operations scaling as O(n log n):
```
Rust:  32 x 5  =   160 operations (per polynomial)
Noir:  4096 x 12 = 49,152 operations (per polynomial)
Ratio: 307x before parallelism
```

Barretenberg parallelises across 14 threads, reducing the effective ratio
to 307 / 14 = ~22x for the most parallelisable operations, and
128 / 14 = ~9x for the linear ones.  The measured 7.7x is consistent with
this range.

There is no way to reduce the Noir proving time to match Rust for this
specific circuit without switching to a different proof system that allows
smaller domains.  All three sub-circuits (accumulator-only, MCV-only, full)
hit the same 2^12 domain because their gate counts (3,284 / 3,621 / 3,970)
all round up to 4,096.

### 4.2 Larger Proof

14,656 bytes vs ~1,200 bytes -- a 12x increase in proof size.

UltraHonk must commit to all wire columns (arithmetic wires, range-check
wires, permutation wires, lookup wires) and include their opening
evaluations.  The Rust circuit commits to exactly the minimum: 5 witness
polynomials and 5 quotient polynomials (plus 3 shifted evaluations for the
recurrence constraints).

For a system where proofs are transmitted over a network or stored on-chain,
14 KB vs 1.2 KB is a meaningful difference at scale.  For a single-proof
batch auction system it is negligible.

### 4.3 Loss of Algorithmic Transparency

In the Rust implementation every intermediate value is inspectable.  You can
print the polynomial coefficients, verify quotient remainders symbolically,
trace how a specific constraint error propagates, and confirm the math by
hand at every layer.

In Noir, the constraint-to-proof mapping passes through several compiler
layers -- Noir source -> SSA IR -> ACIR -> UltraHonk gate assignment ->
proving key -- that are not designed to be human-readable.  You can see the
ACIR opcode count (689) and the gate count (3,970), but not the internal
structure of the UltraHonk polynomials.

### 4.4 Toolchain Instability

The Noir circuit depends on `nargo` and `bb`, two rapidly evolving pre-1.0
tools.  During this implementation, multiple API-level breaking changes were
encountered between Barretenberg v4 and v5:

- `bb prove` changed its output from a single file to a directory
- `bb verify` gained a mandatory `-i` flag for public inputs
- VK paths changed from `--output-dir/vk` to `--output-dir/vk/vk`
- The `pub` keyword position in Noir function signatures changed

The Rust implementation has no external toolchain dependency beyond the
Rust compiler itself.  All cryptographic code is statically linked and
versioned in `Cargo.lock`.

### 4.5 Less Control Over Performance

Because domain size is fixed to the next power of two above the gate count,
and because all three sub-circuits happen to fall in the same 2^12 bucket,
there is no way to get a speedup from isolating individual constraint
families.  The only available performance lever is reducing the total gate
count below 2,048 -- which is not achievable with the current set of
constraints.

In the Rust implementation, each layer is individually optimisable.  The two
constant-coefficient quotient polynomials (`q_acc_a_rec`, `q_acc_b_rec`) are
already exploited: their MSM reduces from ~400 us to ~60 us because a
degree-0 polynomial has only one coefficient.

---

## 5. Head-to-Head Comparison

| Criterion | Rust | Noir | Winner |
|---|---|---|---|
| Lines of cryptographic code | ~1,134 | ~0 | Noir |
| Lines of constraint code | ~1,134 (mixed) | ~135 | Noir |
| Proof generation time | 10.56 ms (1 thread) | 81.1 ms (14 threads) | Rust |
| Pairing verification time | 15.71 ms | 12.3 ms | Noir |
| Prove + verify (total) | 27.52 ms | 93.4 ms | Rust |
| Proof size | ~1,200 bytes | 14,656 bytes | Rust |
| Trusted setup | Simulated (insecure) | Ignition ceremony (production) | Noir |
| Proof blinding | Absent | Automatic | Noir |
| Prover/verifier separation | Combined process | Separate binaries | Noir |
| Auditability by non-cryptographers | Low | High | Noir |
| Regulatory / compliance readiness | Low | High | Noir |
| Toolchain stability | High (Cargo.lock) | Low (pre-1.0 tools) | Rust |
| Extensibility (new constraints) | Low (new pipeline layer) | High (one new loop) | Noir |
| Standard proof format | No | Yes | Noir |
| On-chain verification support | No | Yes (Solidity verifier) | Noir |
| Domain size | 32 points (optimal) | 4,096 points (general) | Rust |
| Protocol design / debugging | Excellent (all values visible) | Limited (opaque pipeline) | Rust |

---

## 6. Which Is the Better Choice for FBA Proof Systems?

### For production deployment: Noir

The FBA protocol generates one proof per auction round.  Real batch auction
systems run on cadences of seconds to minutes between rounds.  At 93 ms for
prove + verify, Noir is fast enough for any batch interval longer than one
second.  The 7.7x proving slowdown relative to Rust is irrelevant when the
proof is generated in the background while the next auction collects orders.

The security improvements are not optional in a deployed system:

- A real exchange cannot use a simulated trusted setup.  It must either run
  its own multi-party ceremony or use an existing one.  Barretenberg uses
  the Ignition ceremony at no additional cost.
- Proof blinding is required to prevent statistical attacks on polynomial
  evaluations in high-frequency proof environments.  Barretenberg adds it
  automatically.
- The exchange's clearing counterparty -- a bank, central counterparty, or
  regulator -- must be able to verify proofs independently using only a
  published verification key, the proof, and the public MCV value.  The Rust
  implementation is a combined prover/verifier that cannot satisfy this
  requirement without additional engineering.

The auditability advantage is significant in financial infrastructure.  The
five FBA constraints in the Noir source are readable assertions that a
compliance team can verify against the exchange rulebook line by line.
The Rust cryptographic layers require deep expertise to audit.

The proof size difference (14 KB vs 1.2 KB) is negligible for a system
generating one proof per auction round.  Even at one round per second with
12-month retention, the total proof storage is ~440 GB -- dominated by the
auction data itself, not the proofs.

### For protocol research and design: Rust

The Rust implementation is the right tool for the design phase:
understanding which constraints are needed, verifying that the polynomial
identities are correct, confirming that the skip points are placed correctly,
and measuring the theoretical minimum proving cost.

The 1,134-line hand-written pipeline is a complete formal specification of
the proof system.  It makes the mathematical structure explicit in a way that
a compiled Noir circuit cannot.  The gap between the Rust and Noir proving
times (10.6 ms vs 81 ms) is only interpretable because the Rust
implementation exists to serve as the theoretical baseline.

The two implementations are complementary.  The Rust prototype establishes
correctness and measures the theoretical floor.  The Noir circuit brings
that correctness into a production-grade proof system at the cost of a 3.4x
end-to-end slowdown -- a cost that is entirely acceptable for the FBA use
case.

### Recommendation

```
Use Noir for production FBA proof systems.
Use Rust for protocol design, benchmarking, and mathematical verification.
```

The two are not alternatives -- they are different phases of the same
workflow.  Write the Rust prototype to establish and verify the constraint
system.  Deploy the Noir circuit in production.

---

## 7. Key Numbers for Reference

Measured on Apple M4 Max, macOS Sequoia 15.3.
Noir: nargo 1.0.0-beta.21, Barretenberg 5.0.0-nightly.20260505.
Rust: rustc 1.95.0, arkworks 0.4.x, single-threaded.

### Criterion-style benchmark means (bench.py, 20 samples)

| Phase | Noir mean | Rust mean | Ratio |
|---|---|---|---|
| Witness generation | 82.7 ms | 0.067 ms | 1,242x slower |
| Proof generation | 81.1 ms | 10.56 ms | 7.7x slower |
| Pairing verification | 12.3 ms | 15.71 ms | 1.3x **faster** |
| Prove + verify | 93.4 ms | 27.52 ms | 3.4x slower |

### Sub-circuit benchmark means (bench_sub.py, 10 samples)

| Circuit | Prove | Verify | Prove+Verify |
|---|---|---|---|
| Accumulator only (C1-C4) | 76.5 ms | 12.4 ms | 88.9 ms |
| MCV only (C5 + MCV) | 78.5 ms | 12.3 ms | 90.8 ms |
| Full circuit (all 5 + MCV) | 80.6 ms | 12.3 ms | 92.8 ms |

All three sub-circuits share the same 2^12 = 4,096 domain, so proving
times are nearly equal despite different constraint counts.

### Circuit analysis

| Circuit | ACIR opcodes | Brillig opcodes | UH gates | Domain |
|---|---|---|---|---|
| Accumulator (C1-C4) | 206 | 0 | 3,284 | 4,096 |
| MCV (C5 + MCV) | 525 | 34 | 3,621 | 4,096 |
| Full circuit | 689 | 34 | 3,970 | 4,096 |

The zero Brillig opcodes in the accumulator circuit reflect that C1-C4
contain only equality and addition assertions on u64 -- no range comparisons,
so no unconstrained hint functions are needed.  The 34 Brillig opcodes in the
MCV circuit come from the <= comparisons, which require
`directive_integer_quotient` and `directive_invert` hints for range checking.

---

*For the full mathematical derivation of how each polynomial constraint maps
to a Noir circuit assertion, see the explanation document in this repository.*
