# ZK-FBA — Zero-Knowledge Proof of Frequent Batch Auction Arithmetic

A research implementation of a PLONK-style polynomial proof system for
**Frequent Batch Auctions (FBAs)** over the BN254 elliptic curve, built in
Rust using the `arkworks` cryptographic library suite.


---

## Overview

A Frequent Batch Auction clears a double-sided market at a single uniform
price, the **Market Clearing Volume (MCV)**, computed from cumulative bid
and ask depth curves. This project proves that computation was done correctly
using zero-knowledge polynomial commitments, without revealing the individual
order data. (Notes: as the number of trades goes up, it is easier to "circuit" stuff in Noir, as you have to do it by hand in Rust. There is a limit for the sub circuits that come across as the powers of 2 are reached.)

The implementation is structured as a sequential pipeline of layers, each
building on the last:

```
Layer 1   Array computation         bid depth, ask depth, min (MCV) arrays
Layer 2   Polynomial interpolation  5 witness polynomials over Fr / H
Layer 3a  KZG trusted setup         simulated Powers-of-Tau SRS (G1 + G2)
Layer 3b  KZG witness commits       commit to 5 witness polynomials
Layer 3c  Quotient polynomials      V(X)/D(X) = Q(X) with zero remainder
Layer 3d  KZG quotient commits      commit to 5 quotient polynomials
Layer 3e  Fiat-Shamir transform     replace verifier challenges with SHA-256
Layer 3f  KZG opening proofs        π per polynomial + pairing verification
Layer 4   Constraint verification   algebraic check on all 5 constraints
```

---

## Mathematical Background

### Auction Data

The market has 21 price ticks (0, 10, 20, …, 110). At each price:

- `Bid[i]` — quantity offered to buy at that price (raw order flow)
- `Ask[i]` — quantity offered to sell at that price
- `Acc_B[i]` — bid depth: backward cumulative sum of Bid (total demand at price i or higher)
- `Acc_A[i]` — ask depth: forward cumulative sum of Ask (total supply at price i or lower)
- `Min[i]` — min(Acc_B[i], Acc_A[i]) — MCV candidate; the global maximum is the clearing volume

### Polynomial Encoding

The 21-element arrays are padded to 32 values (zeros) and interpolated via
inverse NTT over a multiplicative subgroup H of order 32 in the BN254 scalar
field Fr. The generator ω satisfies ω³² = 1.

### Vanishing Constraints

Five polynomial identities must hold over H to certify a correct auction:

| Label | Constraint |
|---|---|
| V_AccA_init | Acc_A(ω⁰) = Ask(ω⁰) |
| V_AccA_rec  | Acc_A(ω·X) − Acc_A(X) − Ask(ω·X) = 0  for all X ≠ ω²⁰ |
| V_AccB_init | Acc_B(ω²⁰) = Bid(ω²⁰) |
| V_AccB_rec  | Acc_B(X) − Acc_B(ω·X) − Bid(X) = 0   for all X ≠ ω³¹ |
| V_KL        | (Acc_A(X) − Min(X)) · (Acc_B(X) − Min(X)) = 0  for all X ∈ H |

The skip points are **asymmetric** by design:
- V_AccA_rec skips ω²⁰ (ask accumulates forward; the data→padding boundary fails there)
- V_AccB_rec skips ω³¹ (bid accumulates backward; the cyclic wrap-around at ω³¹ fails there)

### KZG Polynomial Commitments

Each witness polynomial p(X) is committed as a single G1 curve point:

```
C = [p(τ)]_G1 = Σᵢ pᵢ · [τⁱ · G1]
```

where τ is a secret field element (toxic waste) used only during the trusted
setup. In production, τ is destroyed after a multi-party ceremony.

### Quotient Polynomials

For each constraint V(X) = 0 on H, the prover computes:

```
Q(X) = V(X) / D(X)    (zero remainder proves the constraint holds)
```

Two kinds of divisor D(X) are used:
- `(X − ωʲ)` — for point constraints (initialisation)
- `Z_H(X) = X³² − 1` — for full-domain constraints (recurrences, min exclusivity)

### Fiat-Shamir Heuristic

In the interactive protocol the verifier sends a random evaluation point ζ.
The Fiat-Shamir transform replaces that with a hash of the proof transcript,
making the proof non-interactive:

```
ζ = SHA-256( C_Bid ‖ C_Ask ‖ C_AccB ‖ C_AccA ‖ C_Min
            ‖ C_Q_AccA_init ‖ … ‖ C_Q_KL )   mod |Fr|

α = SHA-256( ζ ‖ Bid(ζ) ‖ Ask(ζ) ‖ … ‖ Q_KL(ζ) )   mod |Fr|
```

The verifier recomputes ζ and α from the published commitments and evaluations,
then confirms five residues all vanish and their α-weighted batch sum is zero:

```
rᵢ = Vᵢ(ζ) − Qᵢ(ζ) · Dᵢ(ζ)     (must equal 0 for each i)

Σᵢ αⁱ⁻¹ · rᵢ = 0                 (batched check)
```

### KZG Opening Proofs

Fiat-Shamir produces claimed evaluations, but does not prove they are
consistent with the commitments. KZG opening proofs close this gap. For each
committed polynomial p(X) with commitment C and claimed value y = p(ζ), the
prover computes a single G1 point:

```
π = [(p(X) − y) / (X − ζ)]_G1
```

The polynomial (p(X) − y) has p(ζ) − y = 0 as a root, so (X − ζ) divides it
exactly. The verifier checks with a pairing equation — requiring the G2 point
[τ]G2 from the SRS, but no knowledge of τ itself:

```
e( C − y·G1,  G2 )  ==  e( π,  [τ]G2 − ζ·G2 )
```

Both sides equal e(G1, G2)^{p(τ)−y} when the proof is honest. A cheating
prover cannot produce a valid π for a false y without solving the discrete
logarithm problem on BN254.

**Batched verification** reduces n individual pairing checks (2n pairings) to
2 pairings for any n, using a random scalar r:

```
e( Σᵢ rⁱ·(Cᵢ − yᵢ·G1),  G2 )  ==  e( Σᵢ rⁱ·πᵢ,  [τ]G2 − ζ·G2 )
```

This implementation opens all 10 committed polynomials at ζ and 3 polynomials
at ω·ζ (the shifted evaluations required by the recurrence constraints), for
13 opening proofs total. Verification costs 4 pairings in the batched mode.

---

## Project Structure

```
zk_fba/
├── Cargo.toml              package manifest and dependency versions
├── src/
│   ├── lib.rs              library: all pipeline layers (~1134 lines)
│   └── main.rs             binary: formatted output + per-layer timing (~269 lines)
└── benches/
    └── kzg_bench.rs        criterion benchmark harness (~215 lines)
```

---

## Dependencies

All dependencies are pinned in `Cargo.lock` via `cargo build`.

| Crate | Version | Role |
|---|---|---|
| `ark-ff` | 0.4.2 | BN254 scalar field Fr: arithmetic, inversion, exponentiation |
| `ark-poly` | 0.4.2 | DensePolynomial, GeneralEvaluationDomain, IFFT (NTT) |
| `ark-ec` | 0.4.2 | G1/G2 group operations, Pippenger MSM, bilinear pairing |
| `ark-bn254` | 0.4.0 | BN254 curve parameters (Fr, G1Affine, G1Projective, G2Affine, G2Projective, Bn254 pairing) |
| `ark-std` | 0.4.0 | RNG abstraction, `test_rng()` |
| `ark-serialize` | 0.4.2 | Compressed G1 point serialisation for hashing and display |
| `sha2` | 0.10.9 | SHA-256 implementation (RustCrypto) for Fiat-Shamir transcript |
| `criterion` | 0.5.1 | Statistical benchmark harness (dev-dependency only) |

---

## How to Run

### Prerequisites

- **Rust** — install via [rustup.rs](https://rustup.rs). This project requires
  Rust ≥ 1.65 (the 2021 edition). The version used during development is shown
  in the execution environment section below.
- No other system dependencies are needed; all cryptographic primitives are
  pure Rust.

### Commands

```bash
# Clone / navigate to the project
cd zk_fba

# Run the full pipeline (all layers, formatted output with per-layer timing)
cargo run --release

# Run all statistical benchmarks (criterion, 100 samples each, 3 s warm-up)
cargo bench

# Run a specific benchmark by substring match
cargo bench -- layer3e
cargo bench -- layer3f
cargo bench -- full_pipeline
cargo bench -- kzg_commit_single

# Run the test suite (if tests are added)
cargo test
```

The `--release` flag is required for meaningful timings; debug builds are
10–50× slower due to no optimisation and extra bounds checks.

---

## Execution Environment

The numbers below are from a single cold `cargo run --release` run followed by
a full `cargo bench` session. All work is single-threaded.

| Item | Value |
|---|---|
| **Machine** | Apple Mac Studio (2024) |
| **CPU** | Apple M4 Max (14-core, ARM64, 4 nm) |
| **RAM** | 36 GB unified memory |
| **OS** | macOS Sequoia 15.3 (Darwin 24.3.0, kernel `xnu-11215.81.4`) |
| **Architecture** | `arm64` (AArch64) |
| **Rust toolchain** | `rustc 1.95.0 (59807616e 2026-04-14)` / `cargo 1.95.0` |
| **Optimisation** | `opt-level = 3` (release + bench profiles) |
| **Threads** | 1 (no `rayon` or async; all layers run sequentially) |
| **Binary size** | 839 KB (stripped release Mach-O) |

### How the code was compiled and run

1. **Compilation** — `cargo run --release` first invokes `cargo build --release`,
   which calls `rustc` with `-C opt-level=3` (cargo release defaults). The
   entire dependency graph — including the arkworks crates — is compiled from
   source on first build; subsequent runs use the incremental cache in
   `target/release/deps/`. The release build takes approximately 40–90 seconds
   on first compile.

2. **Linking** — The compiler links all crates into a single self-contained
   native binary at `target/release/zk_fba`. No dynamic libraries are needed
   at runtime; all cryptographic code is statically inlined.

3. **Execution** — The OS loads the binary directly; Rust has no VM or JIT.
   The `main()` function runs all pipeline layers in sequence on a single
   thread. All timing is measured with `std::time::Instant` (nanosecond
   resolution on Apple Silicon) and printed after each phase.

4. **Benchmarks** — `cargo bench` compiles a separate benchmark binary
   (`target/release/deps/kzg_bench-*`) that links Criterion. Criterion runs
   each benchmark function in a loop: 3 seconds of warm-up (cache and branch
   predictor stabilisation), then 100 timed samples. It reports the
   statistical mean and 95% confidence interval, discards outliers, and writes
   HTML reports to `target/criterion/`.

### Why `cargo run --release` timings differ from `cargo bench`

The `cargo run` timings are **wall-clock times for a single iteration**,
measured after the OS has loaded the binary. They capture real end-to-end
latency but have noise from cache cold-starts and OS scheduling. The criterion
benchmark timings are **statistical means over 100 iterations** with warm CPU
caches and branch predictors; they are the more reliable numbers for reporting.

---

## Benchmark Results

Measured on the Apple M4 Max described above. All values are criterion
statistical means.

### Per-layer benchmarks

| Benchmark | Mean | Notes |
|---|---|---|
| `layer1_compute_all_arrays` | **681 ns** | 21 additions per accumulator; pure integer work |
| `layer2_interpolate_5_polys` | **7.60 µs** | 5 × IFFT over domain of size 32 |
| `layer3a_kzg_trusted_setup` | **1.699 ms** | 32 G1 scalar multiplications + 1 G2 scalar multiplication |
| `layer3b_kzg_commit_witnesses_5` | **2.222 ms** | 5 Pippenger MSMs, degree-31 polynomials |
| `layer3c_compute_quotient_polys_5` | **32.13 µs** | 2 synthetic divisions + 3 polynomial long divisions |
| `layer3d_kzg_commit_quotients_5` | **1.405 ms** | 5 Pippenger MSMs, degrees 0–30 |
| `layer3e_fiat_shamir_prove` | **8.61 µs** | 2 × SHA-256 + 13 Horner polynomial evaluations |
| `layer3f_opening_proofs_13_msms` | **5.185 ms** | 13 synthetic divisions + 13 Pippenger MSMs |
| `layer3f_pairing_verify_all` | **15.71 ms** | 26 individual pairings + 4 batched pairings |
| `layer4_verify_constraints` | **66.6 µs** | 32 polynomial evaluations × 5 constraints |
| `full_pipeline_end_to_end` | **27.52 ms** | All layers, fresh τ each iteration |

### Cost breakdown

| Phase | Time | % of total |
|---|---|---|
| Proof generation (Layers 1–3f opening proofs) | ~11.5 ms | ~42% |
| Pairing verification (Layer 3f verify) | ~15.7 ms | ~57% |
| Algebraic check (Layer 4) | ~0.07 ms | < 1% |

Pairing verification dominates because BN254 Miller loop + final exponentiation
is roughly 10× more expensive per call than a G1 MSM of comparable size.
The batched 2-pairing mode (4 pairings total instead of 26) is the efficient
path; individual checks are run here for diagnostic completeness.

### Single-polynomial commit breakdown

| Polynomial | Degree | Commit time |
|---|---|---|
| `w_bid` | 31 | 402 µs |
| `w_ask` | 31 | 403 µs |
| `w_acc_b` | 31 | 400 µs |
| `w_acc_a` | 31 | 401 µs |
| `w_min` | 31 | 401 µs |
| `q_acc_a_init` | 30 | 395 µs |
| `q_acc_a_rec` | 0 (constant) | **60 µs** |
| `q_acc_b_init` | 30 | 393 µs |
| `q_acc_b_rec` | 0 (constant) | **60 µs** |
| `q_kl` | 30 | 398 µs |

The two recurrence quotient polynomials (`q_acc_a_rec`, `q_acc_b_rec`) are
degree-0 constants — a single field element — so their MSM reduces to one
scalar multiplication: ~60 µs instead of ~400 µs. Their opening proofs are
the G1 identity point (point at infinity), which the pairing check handles
correctly.

---

## Output Description

Running `cargo run --release` prints a structured report. Below is a summary
of each section:

**Layer 1** — Tabulates all 21 price ticks with their raw bid/ask volumes,
cumulative depth curves, and min values. Prints the MCV (= 5000 units at price
103 in the sample data).

**Layer 2** — Reports the domain size (32), polynomial degree bound (31), and
number of polynomials (5). Timing covers 5 IFFT calls.

**Layer 3a/b** — Prints the SRS size (32 G1 points + 1 G2 point) and the
compressed hex prefix of each of the 5 witness KZG commitments (32 bytes each
on BN254).

**Layer 3c** — Table of all 5 quotient polynomials: divisor used, degree of
Q, remainder (must be zero), and pass/fail. All must pass.

**Layer 3d** — Hex prefixes of the 5 quotient commitments. Reports the
maximum quotient degree vs. the SRS degree bound.

**Layer 3e** — Fiat-Shamir challenges ζ and α in hex; evaluations of all 8
polynomial values used by the verifier (5 at ζ, 3 at ω·ζ); Z_H(ζ); the 5
individual residues rᵢ; and the batched check result.

**Layer 3f** — For each of the 10 committed polynomials: the compressed hex
prefix of the opening proof π and the individual pairing check result. Then
the 3 shifted opening proofs at ω·ζ. Then the batched 2-pairing results for
both evaluation points. All 13 proofs must pass.

**Layer 4** — Direct algebraic verification of all 5 constraints by evaluating
polynomials at every domain point. This is a redundant check independent of
the KZG/FS machinery, confirming the witness polynomials are correct.

**Timing summary** — Wall-clock time for each layer in the single run, plus
total.

---

## Proof Completeness

The implementation is a complete, externally-verifiable non-interactive proof:

| Component | Status |
|---|---|
| Witness polynomial commitments (×5) | Complete |
| Quotient polynomial commitments (×5) | Complete |
| Fiat-Shamir challenges ζ and α | Complete |
| Polynomial evaluations at ζ (×10) and ω·ζ (×3) | Complete |
| Batched consistency check (Fiat-Shamir residues) | Complete |
| KZG opening proofs at ζ (×10) | Complete |
| KZG opening proofs at ω·ζ (×3) | Complete |
| Pairing verification — individual (×13) | Complete |
| Pairing verification — batched (×4 pairings) | Complete |

A third party who receives only the 10 commitments, the 13 evaluation claims,
the 13 opening proof points, and the public domain parameters can verify the
entire proof without access to the polynomial coefficients or the toxic waste τ.

---

## References

- Kate, Zaverucha, Goldberg — *Constant-Size Commitments to Polynomials and
  Their Applications*, ASIACRYPT 2010. (KZG commitments)
- Gabizon, Williamson, Ciobotaru — *PLONK: Permutations over Lagrange-Bases
  for Oecumenical Noninteractive Arguments of Knowledge*, 2019.
- Fiat, Shamir — *How to Prove Yourself: Practical Solutions to Identification
  and Signature Problems*, CRYPTO 1986.
- arkworks contributors — <https://github.com/arkworks-rs>
