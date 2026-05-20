#!/usr/bin/env python3
"""
ZK-FBA Noir -- timing harness
-----------------------------------------------------------------------------
Runs the full Noir proof pipeline:

  1. nargo compile   -> compile circuit to ACIR bytecode
  2. nargo execute   -> evaluate circuit + generate witness
  3. bb write_vk     -> build the verification key from the circuit
  4. bb prove        -> generate the UltraHonk proof
  5. bb verify       -> verify the proof

Prints per-phase wall-clock times and a side-by-side comparison with the
Rust/arkworks criterion benchmark means from ../zk_fba/README.md.

Usage:
    cd zk_fba_noir
    python3 time_proof.py

Requirements:
    nargo  >= 0.38.0  (install via noirup -- see INSTALL below)
    bb     >= 0.82.0  (install via bbup  -- see INSTALL below)

---- INSTALL ----------------------------------------------------------------
# 1. nargo (Noir compiler + package manager)
curl -L https://raw.githubusercontent.com/noir-lang/noirup/main/install | bash
source ~/.zshrc        # or ~/.bash_profile / restart terminal
noirup                 # installs latest stable nargo

# 2. bb (Barretenberg proving backend)
curl -L https://raw.githubusercontent.com/AztecProtocol/aztec-packages/refs/heads/master/barretenberg/bbup/bbup -o /tmp/bbup
chmod +x /tmp/bbup && /tmp/bbup
source ~/.zshrc        # adds bb to PATH
-----------------------------------------------------------------------------
"""

import subprocess
import time
import sys
import os
import shutil

HERE = os.path.dirname(os.path.abspath(__file__))

# -- Rust criterion means (ms) from ../zk_fba/README.md ----------------------
# All values are statistical means over 100 iterations, Apple M4 Max.
RUST = {
    "Layer 1  array computation":          0.000681,
    "Layer 2  polynomial interpolation":   0.00760,
    "Layer 3a KZG trusted setup":          1.699,
    "Layer 3b KZG witness commits (x5)":   2.222,
    "Layer 3c quotient polynomials (x5)":  0.03213,
    "Layer 3d KZG quotient commits (x5)":  1.405,
    "Layer 3e Fiat-Shamir prove":          0.00861,
    "Layer 3f opening proofs (13 MSMs)":   5.185,
    "Layer 3f pairing verification":       15.71,
    "Layer 4  algebraic check":            0.0666,
}
RUST_TOTAL = 27.37

RUST_PROVE_MS = sum(RUST[k] for k in [
    "Layer 1  array computation",
    "Layer 2  polynomial interpolation",
    "Layer 3a KZG trusted setup",
    "Layer 3b KZG witness commits (x5)",
    "Layer 3c quotient polynomials (x5)",
    "Layer 3d KZG quotient commits (x5)",
    "Layer 3e Fiat-Shamir prove",
    "Layer 3f opening proofs (13 MSMs)",
])

# -- helpers ------------------------------------------------------------------

W = 72

def sep(c="-", n=W): print(c * n)

def check_tool(name: str) -> str:
    path = shutil.which(name)
    if path:
        return path
    print(f"\n  x  '{name}' not found in PATH.\n")
    if name == "nargo":
        print("  Install nargo:")
        print("    curl -L https://raw.githubusercontent.com/noir-lang/noirup/main/install | bash")
        print("    noirup")
    elif name == "bb":
        print("  Install bb (Barretenberg):")
        print("    curl -L https://raw.githubusercontent.com/AztecProtocol/aztec-packages/refs/heads/master/barretenberg/bbup/bbup -o /tmp/bbup")
        print("    chmod +x /tmp/bbup && /tmp/bbup")
    print()
    sys.exit(1)


def run(cmd: str, label: str) -> float:
    """Run a shell command, time it, print result. Returns elapsed ms."""
    dots = max(1, 52 - len(label))
    print(f"  {label} {'.' * dots}", end="", flush=True)
    t0 = time.perf_counter()
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True, cwd=HERE)
    ms = (time.perf_counter() - t0) * 1_000
    if result.returncode != 0:
        print(f" FAILED (exit {result.returncode})\n")
        if result.stdout.strip():
            print("  stdout:\n" + "\n".join("    " + l for l in result.stdout.strip().splitlines()[-20:]))
        if result.stderr.strip():
            print("  stderr:\n" + "\n".join("    " + l for l in result.stderr.strip().splitlines()[-20:]))
        sys.exit(1)
    print(f" {ms:>8.1f} ms")
    return ms


def tool_version(name: str) -> str:
    r = subprocess.run([name, "--version"], capture_output=True, text=True)
    line = (r.stdout or r.stderr).strip().splitlines()[0] if r.returncode == 0 else "?"
    return line

# -- main ---------------------------------------------------------------------

def main():
    os.chdir(HERE)

    nargo_path = check_tool("nargo")
    bb_path    = check_tool("bb")

    nargo_ver = tool_version("nargo")
    bb_ver    = tool_version("bb")

    print()
    sep("=")
    print("  ZK-FBA Noir  |  Barretenberg UltraHonk  |  BN254")
    sep("=")
    print(f"  nargo : {nargo_ver}")
    print(f"  bb    : {bb_ver}")
    print(f"  cwd   : {HERE}")
    sep()
    print()

    t: dict[str, float] = {}

    # -- Step 1: Compile ------------------------------------------------------
    # Translates src/main.nr -> ACIR bytecode + solving info
    # Rust analogue: none (the circuit *is* the constraint encoding)
    t["compile"] = run(
        "nargo compile --package zk_fba_noir",
        "nargo compile  (ACIR bytecode)"
    )

    # -- Step 2: Execute / witness generation ---------------------------------
    # Runs the circuit with Prover.toml inputs, produces target/witness.gz.
    # Rust analogue: Layer 4 algebraic check (~66 us) -- both confirm the
    # witness satisfies every constraint before touching any cryptography.
    t["execute"] = run(
        "nargo execute --package zk_fba_noir",
        "nargo execute  (witness gen)"
    )

    # -- Step 3: Write verification key --------------------------------------
    # One-time precomputation from the circuit artifact.
    # Rust analogue: none (KZG SRS is pre-generated in production;
    # Rust simulates it per-run with random tau -> Layer 3a, 1.7 ms).
    t["write_vk"] = run(
        "bb write_vk -b ./target/zk_fba_noir.json -o ./target/vk",
        "bb write_vk    (verification key)"
    )

    # -- Step 4: Prove --------------------------------------------------------
    # Barretenberg builds the UltraHonk proof:
    #   - polynomial commitment (KZG) to all witness columns
    #   - quotient polynomial computation
    #   - Fiat-Shamir challenges (Poseidon-based inside bb)
    #   - KZG opening proofs (one per committed polynomial)
    # Rust analogue: Layers 1-3f prove -> ~11.5 ms total (excl. pairing verify)
    t["prove"] = run(
        "bb prove -b ./target/zk_fba_noir.json -w ./target/zk_fba_noir.gz -k ./target/vk/vk -o ./target/proof_out",
        "bb prove        (KZG + FS + openings)"
    )

    # -- Step 5: Verify -------------------------------------------------------
    # Checks the UltraHonk proof using only: proof, vk, public inputs.
    # Rust analogue: Layer 3f pairing verification -> 15.71 ms
    t["verify"] = run(
        "bb verify -k ./target/vk/vk -p ./target/proof_out/proof -i ./target/proof_out/public_inputs",
        "bb verify       (pairing check)"
    )

    # -- Proof size -----------------------------------------------------------
    proof_path = os.path.join(HERE, "target", "proof_out", "proof")
    proof_bytes = os.path.getsize(proof_path) if os.path.exists(proof_path) else 0

    # -- Summary table --------------------------------------------------------
    print()
    sep("=")
    print("  RESULTS -- Wall-clock times (single run, not criterion means)")
    sep("=")
    print(f"  {'Phase':<44}  {'Noir (ms)':>10}  {'Rust (ms)':>10}")
    sep()

    rows = [
        ("nargo compile  (ACIR bytecode)",
         "compile",
         "--",
         None),

        ("nargo execute  (witness generation)",
         "execute",
         "Layer 4  algebraic check",
         RUST["Layer 4  algebraic check"]),

        ("bb write_vk    (verification key)",
         "write_vk",
         "Layer 3a KZG trusted setup",
         RUST["Layer 3a KZG trusted setup"]),

        ("bb prove       (commit+quotient+FS+open)",
         "prove",
         "Layers 1-3f prove  (excl. pairing)",
         RUST_PROVE_MS),

        ("bb verify      (pairing verification)",
         "verify",
         "Layer 3f pairing verification",
         RUST["Layer 3f pairing verification"]),
    ]

    for label, key, rust_label, rust_val in rows:
        noir_ms = t[key]
        if rust_val is None:
            rust_str = "--"
        else:
            rust_str = f"{rust_val:.3f}"
        print(f"  {label:<44}  {noir_ms:>10.1f}  {rust_str:>10}")

    sep()
    noir_runtime = t["execute"] + t["prove"] + t["verify"]
    rust_runtime = RUST_TOTAL
    print(f"  {'prove + verify  (runtime, excl. setup)':<44}  {noir_runtime:>10.1f}  {rust_runtime:>10.2f}")
    total_all = sum(t.values())
    print(f"  {'total  (all 5 phases)':<44}  {total_all:>10.1f}  {'--':>10}")
    sep()
    print()
    print(f"  Proof size : {proof_bytes:,} bytes")
    print(f"  Rust proof : ~1,200 bytes  (10 comms + 13 evals + 13 openings)")
    print()
    sep("-")
    print("  Correspondence between Noir phases and Rust layers")
    sep("-")
    print("  nargo execute  ~  Layer 4 algebraic check")
    print("                    Both confirm the witness satisfies every")
    print("                    constraint before any cryptographic work.")
    print()
    print("  bb write_vk    ~  Layer 3a KZG trusted setup")
    print("                    Rust derives a fresh SRS per run (~1.7 ms).")
    print("                    Barretenberg uses a pre-generated ceremony SRS;")
    print("                    write_vk distils the circuit-specific part.")
    print()
    print("  bb prove       ~  Layers 1-3f (prove side)")
    print(f"                    Rust total: {RUST_PROVE_MS:.2f} ms")
    print("                    Includes: arrays, IFFT, commits, quotients,")
    print("                    Fiat-Shamir, and 13 KZG opening proofs.")
    print()
    print("  bb verify      ~  Layer 3f pairing verification")
    print(f"                    Rust: {RUST['Layer 3f pairing verification']:.2f} ms")
    print("                    Both perform the final pairing check(s).")
    sep("-")
    print()


if __name__ == "__main__":
    main()
