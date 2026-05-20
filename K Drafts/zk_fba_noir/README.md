# ZK-FBA Noir — Zero-Knowledge Proof of Frequent Batch Auction Arithmetic

A Noir circuit implementation of the same **Frequent Batch Auction (FBA)**
proof system, targeting the Barretenberg UltraHonk proof
system over the BN254 elliptic curve.

Where the Rust implementation builds every cryptographic layer by hand
(polynomial interpolation, KZG commitments, Fiat-Shamir, opening proofs,
pairing verification), this project expresses the **same five auction
constraints** as a Noir arithmetic circuit.  Barretenberg handles the entire
cryptographic stack automatically.

**Author:** Kimia Esmaili, Concordia University

---

## Overview

A Frequent Batch Auction clears a double-sided market at a single uniform
price — the **Market Clearing Volume (MCV)** — derived from cumulative bid and
ask depth curves.  This project proves that the clearing computation was
performed correctly without revealing the individual order book.

The Rust implementation encodes the proof as explicit polynomial identities
over a 32-point evaluation domain and constructs the KZG proof layer by layer.
The Noir implementation encodes the same constraints as **arithmetic circuit
assertions** — equality checks and range bounds — and delegates the full
cryptographic pipeline to the Barretenberg proving backend:

```
Noir source (constraints)
        │
        ▼
nargo compile  →  ACIR bytecode (abstract circuit IR)
        │
        ▼
nargo execute  →  witness (satisfying assignment for all gates)
        │
        ▼
bb write_vk    →  verification key  (circuit-specific, one-off)
        │
        ▼
bb prove       →  UltraHonk proof  (KZG commits + quotients + Fiat-Shamir + openings)
        │
        ▼
bb verify      →  pairing check  (pass / fail)
```

The five FBA vanishing constraints map directly to circuit assertions with no
polynomial arithmetic required in source:

| Rust layer | Noir assertion |
|---|---|
| V_AccA_init: Acc_A(ω⁰) = Ask(ω⁰) | `assert(acc_a[0] == ask[0])` |
| V_AccA_rec: Acc_A(ωX) − Acc_A(X) − Ask(ωX) = 0 | `assert(acc_a[i] == acc_a[i-1] + ask[i])` for i = 1..20 |
| V_AccB_init: Acc_B(ω²⁰) = Bid(ω²⁰) | `assert(acc_b[20] == bid[20])` |
| V_AccB_rec: Acc_B(X) − Acc_B(ωX) − Bid(X) = 0 | `assert(acc_b[i] == acc_b[i+1] + bid[i])` for i = 0..19 |
| V_KL: (Acc_A − Min)(Acc_B − Min) = 0 | `assert((acc_a[i]-min[i] as Field) * (acc_b[i]-min[i] as Field) == 0)` |

---

## Mathematical Background

### Auction Data

The market has 21 price ticks (0, 10, 20, …, 110).  At each tick index i:

- `bid[i]` — quantity offered to buy at that price (raw order flow)
- `ask[i]` — quantity offered to sell at that price
- `acc_b[i]` — bid depth: backward cumulative sum of `bid` (total demand at
  price i or higher)
- `acc_a[i]` — ask depth: forward cumulative sum of `ask` (total supply at
  price i or lower)
- `min_arr[i]` — `min(acc_b[i], acc_a[i])`: the MCV candidate at tick i
- `mcv` — `max(min_arr)`: the Market Clearing Volume (public output)

Sample auction data used in this implementation:

```
Price:    0    10    20    30    40    50    60    70    80    90   100   101   102   103   104   105   106   107   108   109   110
bid:    100   100   100   200   200   500  1000  1500  2000   700   100   100   100     0     0     0   500   500  1000  1000  2000
ask:      0    20    30    50   100   200   400   700  1000  1500  1000     0     0     0     0     0   100   100   200   200   200

acc_a:    0    20    50   100   200   400   800  1500  2500  4000  5000  5000  5000  5000  5000  5000  5100  5200  5400  5600  5800
acc_b: 11700 11600 11500 11400 11200 11000 10500  9500  8000  6000  5300  5200  5100  5000  5000  5000  5000  4500  4000  3000  2000

min:      0    20    50   100   200   400   800  1500  2500  4000  5000  5000  5000  5000  5000  5000  5000  4500  4000  3000  2000
MCV = 5000   (achieved at price ticks 100–106, indices 10–16)
```

### The Five Vanishing Constraints

In the Rust implementation these are polynomial identities over H (32-point
domain).  In Noir they become per-element array assertions.  The mapping is
exact — the skip points that appear in the polynomial form vanish naturally:

**C1 — V_AccA_init**
The forward accumulator is seeded at the lowest price tick.

```
Polynomial:  Acc_A(ω⁰) = Ask(ω⁰)
Array:       acc_a[0] == ask[0]
```

**C2 — V_AccA_rec**
Each ask-depth element equals the previous plus the current ask volume.
The polynomial form skips ω²⁰ (the boundary between data and padding);
the array form simply stops at i = 20.

```
Polynomial:  Acc_A(ωX) − Acc_A(X) − Ask(ωX) = 0  for all X ≠ ω²⁰
Array:       acc_a[i] == acc_a[i-1] + ask[i]       for i = 1..20
```

**C3 — V_AccB_init**
The backward accumulator is seeded at the highest price tick.

```
Polynomial:  Acc_B(ω²⁰) = Bid(ω²⁰)
Array:       acc_b[20] == bid[20]
```

**C4 — V_AccB_rec**
Each bid-depth element equals the next plus the current bid volume.
The polynomial form skips ω³¹ (cyclic wrap-around);
the array form simply stops at i = 19.

```
Polynomial:  Acc_B(X) − Acc_B(ωX) − Bid(X) = 0  for all X ≠ ω³¹
Array:       acc_b[i] == acc_b[i+1] + bid[i]      for i = 0..19
```

**C5 — V_KL (min-exclusivity)**
At every price tick, `min_arr[i]` equals whichever depth curve is smaller,
so at least one factor is zero.  Range bounds prevent the prover from
supplying the maximum instead of the minimum.

```
Polynomial:  (Acc_A(X) − Min(X)) · (Acc_B(X) − Min(X)) = 0  ∀ X ∈ H
Array:       assert(min_arr[i] <= acc_a[i])                    for i = 0..20
             assert(min_arr[i] <= acc_b[i])                    for i = 0..20
             assert((acc_a[i]-min_arr[i] as Field)
                  * (acc_b[i]-min_arr[i] as Field) == 0)       for i = 0..20
```

The cast to `Field` is needed so the multiplication uses a single mul-gate
rather than u64 wide-product logic.

**MCV correctness (not a polynomial constraint)**
The public output `mcv` is proven to be the exact global maximum of `min_arr`:

```
(a) min_arr[i] <= mcv          for all i = 0..20   (upper bound)
(b) exists j : min_arr[j] == mcv                   (bound is achieved)
```

### How Barretenberg Handles the Cryptography

Barretenberg compiles the ACIR constraint system to **UltraHonk** — a
variant of the PLONK argument with custom gate types and lookup tables.
The steps it performs automatically, replacing the manual Rust layers, are:

| Rust layer | Barretenberg equivalent |
|---|---|
| Layer 2: IFFT interpolation | Internal witness-column FFT over the circuit domain |
| Layer 3a: KZG trusted setup | Pre-generated Ignition ceremony SRS (no per-run τ) |
| Layer 3b: witness commits | KZG commits to all UltraHonk wire polynomials |
| Layer 3c: quotient polys | UltraHonk grand-product and quotient computation |
| Layer 3d: quotient commits | KZG commits to all quotient polynomials |
| Layer 3e: Fiat-Shamir | Poseidon-based transcript hashing (not SHA-256) |
| Layer 3f: opening proofs | One KZG opening proof per committed polynomial |
| Layer 3f: pairing verify | BN254 Miller loop + final exponentiation |

The only Rust layer with no Barretenberg counterpart is **Layer 1** (array
computation) — that arithmetic happens as part of the Noir witness, not the
circuit itself.

---

## Architecture

### Constraint Modules

The circuit is split into two modules that can be compiled and benchmarked
independently:

**`fba_accumulators.nr`** — Constraints C1–C4 (accumulator recurrences)

```noir
pub fn check_accumulators(
    bid: [u64; 21], ask: [u64; 21],
    acc_b: [u64; 21], acc_a: [u64; 21],
) { ... }
```

Contains 206 ACIR opcodes and zero Brillig (unconstrained) opcodes — all
assertions are simple equality and addition checks on `u64` values, which
require no hint functions.

**`fba_mcv.nr`** — Constraint C5 + MCV check (min-exclusivity and global max)

```noir
pub fn check_mcv(
    acc_a: [u64; 21], acc_b: [u64; 21],
    min_arr: [u64; 21], mcv: u64,
) { ... }
```

Contains 525 ACIR opcodes and 34 Brillig opcodes.  The Brillig opcodes arise
from the `<=` comparisons on `u64`, which Noir compiles to range-check
constraints requiring unconstrained integer-quotient and invert hints
(`directive_integer_quotient`, `directive_invert`).

**`main.nr`** — Thin orchestration shell

```noir
mod fba_accumulators;
mod fba_mcv;

fn main(bid, ask, acc_b, acc_a, min_arr, mcv: pub u64) {
    fba_accumulators::check_accumulators(bid, ask, acc_b, acc_a);
    fba_mcv::check_mcv(acc_a, acc_b, min_arr, mcv);
}
```

`mcv` is the only public input — the verifier learns only the Market
Clearing Volume, nothing about the individual order arrays.

### Standalone Sub-circuit Packages

Two additional packages provide isolated compilation and proving targets,
mirroring the per-layer benchmark structure of the Rust Criterion suite:

| Package | Constraints | ACIR opcodes | Rust analogue |
|---|---|---|---|
| `acc_circuit/` (`zk_fba_accumulators`) | C1–C4 only | 206 | Layer 3c Q1..Q4 |
| `mcv_circuit/` (`zk_fba_mcv`) | C5 + MCV | 525 | Layer 3c Q5 + Layer 1 |
| `zk_fba_full/` (`zk_fba_noir`) | All five + MCV | 689 | Full pipeline |

---

## Project Structure

```
zk_fba_noir/
├── Nargo.toml                   Noir workspace manifest (3 members)
│
├── zk_fba_full/                 Main package — full FBA circuit
│   ├── Nargo.toml               package name: zk_fba_noir
│   ├── Prover.toml              private witness inputs + public mcv
│   └── src/
│       ├── main.nr              orchestration shell (~45 lines)
│       ├── fba_accumulators.nr  C1-C4: accumulator module (~40 lines)
│       └── fba_mcv.nr           C5+MCV: min-exclusivity module (~50 lines)
│
├── acc_circuit/                 Standalone accumulator sub-circuit
│   ├── Nargo.toml               package name: zk_fba_accumulators
│   ├── Prover.toml              only bid/ask/acc_b/acc_a inputs
│   └── src/main.nr              C1-C4 only (~40 lines)
│
├── mcv_circuit/                 Standalone MCV sub-circuit
│   ├── Nargo.toml               package name: zk_fba_mcv
│   ├── Prover.toml              only acc_a/acc_b/min_arr/mcv inputs
│   └── src/main.nr              C5 + MCV only (~45 lines)
│
├── time_proof.py                Single-run wall-clock timer (all 5 phases)
├── bench.py                     Criterion-style benchmark (full circuit, N samples)
└── bench_sub.py                 Sub-circuit benchmark (all 3 packages, gate analysis)
```

Compiled artifacts land in the workspace-root `target/` directory:

```
target/
├── zk_fba_noir.json             ACIR bytecode (full circuit)
├── zk_fba_accumulators.json     ACIR bytecode (accumulator sub-circuit)
├── zk_fba_mcv.json              ACIR bytecode (MCV sub-circuit)
├── zk_fba_noir.gz               witness (full)
├── zk_fba_accumulators.gz       witness (accumulator)
├── zk_fba_mcv.gz                witness (MCV)
├── vk/                          verification key (full)
├── vk_acc/                      verification key (accumulator)
├── vk_mcv/                      verification key (MCV)
├── proof_out/                   proof + public_inputs (full)
├── proof_acc/                   proof + public_inputs (accumulator)
└── proof_mcv/                   proof + public_inputs (MCV)
```

---

## Dependencies

| Tool | Version | Role |
|---|---|---|
| `nargo` | 1.0.0-beta.21 | Noir compiler + package manager + witness generator |
| `bb` (Barretenberg) | 5.0.0-nightly.20260505 | UltraHonk prover and verifier |

### Install

```bash
# 1. nargo (Noir compiler)
curl -L https://raw.githubusercontent.com/noir-lang/noirup/main/install | bash
source ~/.zshrc
noirup                        # installs latest stable nargo

# 2. bb (Barretenberg proving backend)
# Via bbup (if the script is available):
curl -L https://raw.githubusercontent.com/AztecProtocol/aztec-packages/refs/heads/master/barretenberg/bbup/bbup \
     -o /tmp/bbup && chmod +x /tmp/bbup && /tmp/bbup
source ~/.zshrc

# Or manually from a GitHub release (arm64-darwin example):
VERSION="5.0.0-nightly.20260505"
curl -L "https://github.com/AztecProtocol/barretenberg/releases/download/v${VERSION}/barretenberg-arm64-darwin.tar.gz" \
     -o /tmp/bb.tar.gz
mkdir -p ~/.bb && tar -xzf /tmp/bb.tar.gz -C ~/.bb
export PATH="$HOME/.bb:$HOME/.nargo/bin:$PATH"   # add to ~/.zshrc for persistence
```

Verify both tools are on PATH before running:

```bash
nargo --version   # nargo version = 1.0.0-beta.21
bb --version      # 5.0.0-nightly.20260505
```

---

## How to Run

All commands are run from the workspace root (`zk_fba_noir/`).  Prepend
`PATH="$HOME/.bb:$HOME/.nargo/bin:$PATH"` if the tools are not already on
your shell PATH.

### Full pipeline — single run with timing

```bash
python3 time_proof.py
```

Runs all five phases for the full circuit, prints wall-clock time for each,
and shows a side-by-side comparison with the Rust Criterion means.

### Criterion-style benchmark — full circuit

```bash
python3 bench.py                  # 20 samples, 3 warm-up (default)
python3 bench.py -n 50            # 50 samples
python3 bench.py -n 100 -w 5      # 100 samples, 5 warm-up
python3 bench.py --no-warmup -n 5 # quick smoke test
```

Benchmarks `nargo execute`, `bb prove`, `bb verify`, and `prove+verify`
with the same statistical methodology as Criterion: mean, std-dev, min, max,
and 95% confidence interval (t-distribution), plus a Tukey IQR outlier
report.  Setup phases (`nargo compile`, `bb write_vk`) run once before the
hot loop and are excluded, matching the Rust benchmark structure.

### Sub-circuit benchmark — gate count and per-family timing

```bash
python3 bench_sub.py              # 10 samples per circuit (default)
python3 bench_sub.py -n 20        # 20 samples per circuit
python3 bench_sub.py --quick      # 5 samples, no warm-up
```

Benchmarks all three packages (`zk_fba_accumulators`, `zk_fba_mcv`,
`zk_fba_noir`) in sequence.  Prints the ACIR opcode count, Brillig opcode
count, UltraHonk gate count, and polynomial domain size for each circuit,
then timing results for prove and verify per package.

### Individual nargo / bb commands

```bash
# Compile a specific package
nargo compile --package zk_fba_noir
nargo compile --package zk_fba_accumulators
nargo compile --package zk_fba_mcv

# Generate witness (reads Prover.toml from the package directory)
nargo execute --package zk_fba_noir

# Circuit info (ACIR + Brillig opcode counts, all packages)
nargo info --workspace

# UltraHonk gate count
bb gates -b ./target/zk_fba_noir.json
bb gates -b ./target/zk_fba_accumulators.json
bb gates -b ./target/zk_fba_mcv.json

# Write verification key
bb write_vk -b ./target/zk_fba_noir.json -o ./target/vk

# Prove (requires VK; outputs proof/ directory)
bb prove -b ./target/zk_fba_noir.json \
         -w ./target/zk_fba_noir.gz \
         -k ./target/vk/vk \
         -o ./target/proof_out

# Verify
bb verify -k ./target/vk/vk \
          -p ./target/proof_out/proof \
          -i ./target/proof_out/public_inputs
```

---

## Execution Environment

| Item | Value |
|---|---|
| **Machine** | Apple Mac Studio (2024) |
| **CPU** | Apple M4 Max (14-core, ARM64, 4 nm) |
| **RAM** | 36 GB unified memory |
| **OS** | macOS Sequoia 15.3 (Darwin 24.3.0) |
| **Architecture** | `arm64` (AArch64) |
| **nargo** | 1.0.0-beta.21 |
| **Barretenberg (bb)** | 5.0.0-nightly.20260505 |
| **Threads (bb)** | 14 (all available performance cores; bb auto-detects) |
| **Proof system** | UltraHonk over BN254 |

### How the code is compiled and executed

1. **`nargo compile`** — Translates `src/main.nr` through the Noir SSA IR to
   ACIR (Abstract Circuit IR), a backend-agnostic bytecode format.  Outputs
   `target/zk_fba_noir.json` (~16 KB).  All Noir type checking, range
   constraint insertion, and Brillig unconstrained function extraction happen
   here.

2. **`nargo execute`** — Runs the Noir virtual machine over the inputs in
   `Prover.toml` to produce a satisfying witness: a concrete assignment to
   every gate in the circuit.  Outputs `target/zk_fba_noir.gz` (~1.9 KB
   compressed).  This is analogous to Rust's Layer 4 algebraic check —
   both confirm the witness satisfies every constraint before any
   cryptographic work begins.

3. **`bb write_vk`** — Derives the verification key from the ACIR artifact.
   This is a one-time per-circuit precomputation: the VK encodes the
   circuit's structure and the Ignition ceremony SRS binding.  Output:
   `target/vk/vk` (~3.6 KB) and `target/vk/vk_hash`.  Analogous to Rust's
   Layer 3a KZG trusted setup, except Barretenberg uses a fixed multi-party
   ceremony SRS rather than a per-run random τ.

4. **`bb prove`** — Barretenberg executes the full UltraHonk prover:
   - FFT/IFFT over the circuit domain (n = 4,096 points, 14 threads)
   - KZG commitment to all wire and permutation polynomials
   - Grand-product argument and quotient polynomial computation
   - Poseidon-based Fiat-Shamir transcript (replaces SHA-256 from Rust)
   - KZG opening proofs for all committed polynomials
   Output: `target/proof_out/proof` (14,656 bytes) and
   `target/proof_out/public_inputs` (32 bytes for `mcv`).

5. **`bb verify`** — Loads the proof, VK, and public inputs; runs the
   UltraHonk verifier (BN254 Miller loops + final exponentiation).  Prints
   `Proof verified successfully`.

### Why timings differ between `time_proof.py` and `bench.py`

`time_proof.py` reports **single-run wall-clock times** measured from Python
(`time.perf_counter`).  These include process startup overhead for each
`nargo` / `bb` invocation and are sensitive to OS scheduling and cold caches.

`bench.py` reports **statistical means over 20 iterations** after a warm-up
phase.  The warm-up stabilises branch predictors and brings the SRS data into
CPU cache; subsequent iterations measure steady-state cryptographic cost.
The bench.py numbers are the reliable figures for comparison with Rust
Criterion means.

---

## Circuit Analysis

### Gate Count Summary

| Circuit | ACIR opcodes | Brillig opcodes | UltraHonk gates | Domain | Utilisation |
|---|---|---|---|---|---|
| `zk_fba_accumulators` (C1–C4) | 206 | 0 | 3,284 | 4,096 (2¹²) | 80% |
| `zk_fba_mcv` (C5 + MCV) | 525 | 34 | 3,621 | 4,096 (2¹²) | 88% |
| `zk_fba_noir` (full) | **689** | **34** | **3,970** | **4,096 (2¹²)** | **97%** |

**ACIR opcodes** are the backend-agnostic constraint count produced by the
Noir compiler.  **UltraHonk gates** are the physical gate count after
Barretenberg lowers ACIR to its custom gate types (arithmetic gates, range
gates via 4-bit lookup tables, and permutation gates).  The ratio of
~5.76 gates per ACIR opcode arises primarily from `u64` range checks: each
`<=` comparison decomposes into a 64-bit range check, which UltraHonk handles
with 16 lookup table rows (64 bits / 4 bits per row).

**Brillig opcodes** are unconstrained hint functions executed outside the ZK
constraint system.  They appear only in circuits that use `<=` on `u64`:
`directive_integer_quotient` (8 ops, computes a / b as a hint for range
checks) and `directive_invert` (9 ops, computes 1/x for the bool found-flag
logic).  The accumulator circuit has no comparisons and therefore zero
Brillig opcodes.

The **polynomial domain** is the next power of two above the gate count.
All three circuits fall in the same bucket (2¹² = 4,096), so their FFT sizes
are identical.

### Why sub-circuits are not faster than the full circuit

The dominant cost in `bb prove` is the polynomial FFT over the domain, which
scales as O(n log n).  Since all three circuits share the same domain
(n = 4,096), their polynomial-arithmetic costs are equal.  A speedup would
only appear if the sub-circuit gate count crossed a power-of-two boundary
(e.g., below 2,048 gates would use the 2¹¹ domain and be roughly 2× faster).
The sub-circuits are isolated for **conceptual clarity** — to show which
constraint family owns which gate count and Brillig opcodes — not for
algorithmic speedup, just as the Rust per-layer benchmarks isolate cost per
layer without reducing the underlying domain size.

### Domain size: Noir vs Rust

| | Rust (hand-rolled) | Noir / UltraHonk |
|---|---|---|
| Domain size | 32 points (2⁵) | 4,096 points (2¹²) |
| Domain bits | 5 | 12 |
| Domain ratio | 1× | **128×** |

The Rust circuit was hand-crafted to use exactly 5 polynomials over the
minimum viable domain (32 points for 21 data values).  UltraHonk is a
general-purpose system: it allocates separate wire columns for each gate
type (arithmetic, range, lookup, permutation) and pads to the next power of
two, producing a 128× larger domain.

For polynomial commitment costs that scale O(n): 128× larger domain,
parallelised across 14 threads, yields an expected slowdown of
128 / 14 ≈ 9×.  The measured proving slowdown is **7.7×** — consistent
with the prediction.

---

## Benchmark Results

Measured on the Apple M4 Max described above.

### Per-phase single-run timing (`time_proof.py`)

| Phase | Time | Rust analogue |
|---|---|---|
| `nargo compile` (ACIR bytecode) | 88.8 ms | — |
| `nargo execute` (witness gen) | 89.2 ms | Layer 4 algebraic check (~66 µs) |
| `bb write_vk` (verification key) | 27.1 ms | Layer 3a KZG trusted setup (1.699 ms) |
| `bb prove` (commit + quotient + FS + openings) | 85.1 ms | Layers 1–3f prove (~10.56 ms) |
| `bb verify` (pairing verification) | 16.3 ms | Layer 3f pairing verify (15.71 ms) |
| **prove + verify (runtime total)** | **179.6 ms** | **27.37 ms** |
| Total all five phases | 306.4 ms | — |

### Criterion-style benchmark (`bench.py`, 20 samples, 3 warm-up rounds)

| Benchmark | Mean | Std-dev | 95% CI | Rust mean | Ratio |
|---|---|---|---|---|---|
| `nargo execute` (witness gen) | **82.746 ms** | 837 µs | [82.4 – 83.1 ms] | 0.067 ms | 1,242× slower |
| `bb prove` (commit+quotient+FS+open) | **81.096 ms** | 1.195 ms | [80.5 – 81.7 ms] | 10.56 ms | 7.7× slower |
| `bb verify` (pairing verification) | **12.335 ms** | 217 µs | [12.2 – 12.4 ms] | 15.71 ms | **1.3× faster** |
| **prove + verify (end-to-end)** | **93.431 ms** | 1.122 ms | [92.9 – 94.0 ms] | 27.52 ms | 3.4× slower |

Outliers: 0/20 for execute and verify; 2/20 high-mild for prove and
prove+verify.

### Sub-circuit benchmark (`bench_sub.py`, 10 samples, 2 warm-up rounds)

| Circuit | Prove mean | Prove CI | Verify mean | Prove+Verify |
|---|---|---|---|---|
| `zk_fba_accumulators` (C1–C4) | 76.46 ms ± 0.37 ms | [76.2 – 76.7 ms] | 12.40 ms ± 0.38 ms | 88.9 ms |
| `zk_fba_mcv` (C5 + MCV) | 78.47 ms ± 0.62 ms | [78.0 – 78.9 ms] | 12.28 ms ± 0.23 ms | 90.8 ms |
| `zk_fba_noir` (full) | 80.59 ms ± 0.86 ms | [80.0 – 81.2 ms] | 12.26 ms ± 0.26 ms | 92.8 ms |

Verify time (12.3 ms) is **identical across all three circuits** — the
pairing check depends only on VK structure, not gate count.  The 4.1 ms
difference in prove time between accumulator (76.5 ms) and full circuit
(80.6 ms) reflects the marginal cost of 686 extra gates (from 3,284 to
3,970) within the same 4,096-point domain.

### Cost breakdown

| Phase | Time | % of prove+verify |
|---|---|---|
| `bb prove` (polynomial work) | 81.1 ms | 87% |
| `bb verify` (pairing check) | 12.3 ms | 13% |

Contrast with Rust: pairing verification dominates at 57% of total.
Barretenberg's pairing implementation is more efficient than the arkworks
one, reducing verify from 15.7 ms to 12.3 ms (a 1.3× improvement), while
the larger domain size pushes proving from 10.6 ms to 81.1 ms.

### Proof size

| Implementation | Proof size | Structure |
|---|---|---|
| Noir / UltraHonk | **14,656 bytes** | All UltraHonk wire + permutation + quotient commitments + evaluations + opening proofs |
| Rust / bespoke PLONK | ~1,200 bytes | 10 KZG commitments + 13 evaluations + 13 opening proofs (4 × 32 B + 13 × 32 B + 13 × 32 B) |

UltraHonk's proof is 12× larger because it commits to many more wire
polynomials (separate columns for arithmetic, range, and permutation gates),
whereas the Rust circuit uses exactly the minimum set of 5 witness and 5
quotient polynomials needed for the FBA constraints.

---

## Proof Completeness

The Noir / Barretenberg proof is complete and externally verifiable.  A third
party who receives only `target/proof_out/proof`, `target/vk/vk`, and
`target/proof_out/public_inputs` (the `mcv` value) can verify the entire
proof without access to any witness data, circuit source, or toxic waste.

| Component | Status |
|---|---|
| All five FBA vanishing constraints encoded | Complete |
| MCV public output bound and existence checks | Complete |
| KZG commits to all wire polynomials | Complete (handled by Barretenberg) |
| Quotient polynomial computation and commits | Complete (handled by Barretenberg) |
| Poseidon-based Fiat-Shamir transcript | Complete (handled by Barretenberg) |
| KZG opening proofs (one per committed polynomial) | Complete (handled by Barretenberg) |
| Pairing verification | Complete (handled by Barretenberg) |
| Ceremony SRS (no toxic waste) | Complete (Ignition ceremony, not simulated τ) |

Compared with the Rust implementation, Noir closes three gaps that exist in
the research prototype:

| Gap in Rust prototype | Status in Noir |
|---|---|
| Simulated τ (random secret, not a ceremony) | Fixed: Barretenberg uses the Ignition multi-party ceremony SRS |
| No proof blinding (polynomial coefficients could leak from evaluations) | Fixed: Barretenberg adds blinding factors automatically |
| Combined prover/verifier in a single process | Fixed: `bb prove` and `bb verify` are separate binaries; verifier needs only proof + VK + public inputs |

---

## Comparison with the Rust Implementation

| Aspect | Rust (`zk_fba`) | Noir (`zk_fba_noir`) |
|---|---|---|
| **Language** | Rust + arkworks | Noir + Barretenberg |
| **Proof system** | Bespoke PLONK (5 polys, 32-point domain) | UltraHonk (multi-column, 4096-point domain) |
| **Curve** | BN254 | BN254 |
| **Fiat-Shamir hash** | SHA-256 | Poseidon |
| **Trusted setup** | Simulated random τ per run | Ignition multi-party ceremony SRS |
| **Polynomial domain** | 32 points (2⁵) | 4,096 points (2¹²) |
| **Proof generation** | 10.56 ms (1 thread) | 81.1 ms (14 threads) |
| **Verification** | 15.71 ms | 12.3 ms |
| **Prove + verify** | 27.52 ms | 93.4 ms |
| **Proof size** | ~1,200 bytes | 14,656 bytes |
| **Lines of cryptographic code** | ~1,134 (all layers manual) | ~135 (constraints only; crypto is zero) |
| **Threads** | 1 | 14 |

The Rust implementation is faster because it was hand-optimised for exactly
this circuit: 5 polynomials, 32 evaluation points, 21 data values.
UltraHonk is designed to compile arbitrary circuits correctly and securely,
not to minimise cost for a specific known structure.  The trade-off is
approximately 3.4× slower end-to-end runtime in exchange for a proof system
that is production-grade, ceremony-backed, and requires zero cryptographic
code from the application developer.

---

## References

- Gabizon, Williamson, Ciobotaru — *PLONK: Permutations over Lagrange-Bases
  for Oecumenical Noninteractive Arguments of Knowledge*, 2019.
  (The PLONK argument that UltraHonk extends.)
- Kate, Zaverucha, Goldberg — *Constant-Size Commitments to Polynomials and
  Their Applications*, ASIACRYPT 2010.  (KZG polynomial commitments.)
- Fiat, Shamir — *How to Prove Yourself: Practical Solutions to
  Identification and Signature Problems*, CRYPTO 1986.
  (Fiat-Shamir heuristic for non-interactive proofs.)
- Aztec Protocol — Barretenberg: <https://github.com/AztecProtocol/barretenberg>
- Noir Language Reference: <https://noir-lang.org>
- Rust reference implementation: `../zk_fba/README.md`
