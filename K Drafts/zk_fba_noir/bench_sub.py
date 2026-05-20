#!/usr/bin/env python3
"""
ZK-FBA Noir -- sub-circuit benchmark
-----------------------------------------------------------------------------
Times each constraint family in isolation, mirroring the per-layer structure
of the Rust Criterion suite in ../zk_fba.

Three circuits are benchmarked:

  zk_fba_accumulators  C1-C4 only  ask/bid depth recurrences
                                   Rust analogue: Layer 3c Q1..Q4
  zk_fba_mcv           C5 + MCV    min-exclusivity + global maximum
                                   Rust analogue: Layer 3c Q5 + Layer 1
  zk_fba_noir (full)   all five    combined circuit (baseline for comparison)

Each circuit is compiled and keyed once (setup), then the prove + verify
hot loop is repeated N_SAMPLES times.  Mean, std-dev, min, max, and 95 %
confidence interval are reported (same statistics as bench.py / Criterion).

Gate counts (ACIR opcodes, UltraHonk gates, polynomial domain size) are
printed alongside timing so you can reason about why timings differ.

Usage:
    python3 bench_sub.py              # 10 samples per circuit (default)
    python3 bench_sub.py -n 20        # 20 samples
    python3 bench_sub.py -n 5 --quick # skip warm-up, 5 samples

Note: All three circuits hit the same 2^12 (4096) UltraHonk domain because
their gate counts (206, 525, 689 ACIR opcodes) all round up to the same
next power of two.  Prove times will therefore be similar despite the
different constraint counts.
"""

import argparse
import math
import os
import shutil
import statistics
import subprocess
import sys
import time
from dataclasses import dataclass, field
from typing import Optional

HERE = os.path.dirname(os.path.abspath(__file__))

# -- Known gate counts (from `nargo info --workspace` and `bb gates`) ---------
GATE_INFO = {
    "zk_fba_accumulators": {
        "acir_opcodes": 206,
        "brillig_opcodes": 0,
        "circuit_size": 3284,
        "domain_size": 4096,
        "domain_bits": 12,
        "constraints": "C1-C4 (accumulator recurrences)",
        "rust_analogue": "Layer 3c Q1..Q4",
    },
    "zk_fba_mcv": {
        "acir_opcodes": 525,
        "brillig_opcodes": 34,
        "circuit_size": 3621,
        "domain_size": 4096,
        "domain_bits": 12,
        "constraints": "C5 + MCV (min-exclusivity + max)",
        "rust_analogue": "Layer 3c Q5 + Layer 1",
    },
    "zk_fba_noir": {
        "acir_opcodes": 689,
        "brillig_opcodes": 34,
        "circuit_size": 3970,
        "domain_size": 4096,
        "domain_bits": 12,
        "constraints": "C1-C5 + MCV (full circuit)",
        "rust_analogue": "All layers",
    },
}

# -- Circuit descriptors -------------------------------------------------------
@dataclass
class Circuit:
    name:      str           # Nargo package name
    pkg_flag:  str           # --package <name>
    json:      str           # path to compiled ACIR JSON (relative to HERE)
    witness:   str           # path to witness gz (relative to HERE)
    vk_dir:    str           # directory written by bb write_vk
    proof_dir: str           # directory written by bb prove

    @property
    def vk_file(self) -> str:
        return os.path.join(self.vk_dir, "vk")

    @property
    def proof_file(self) -> str:
        return os.path.join(self.proof_dir, "proof")

    @property
    def public_inputs_file(self) -> str:
        return os.path.join(self.proof_dir, "public_inputs")

CIRCUITS = [
    Circuit(
        name      = "zk_fba_accumulators",
        pkg_flag  = "--package zk_fba_accumulators",
        json      = "target/zk_fba_accumulators.json",
        witness   = "target/zk_fba_accumulators.gz",
        vk_dir    = "target/vk_acc",
        proof_dir = "target/proof_acc",
    ),
    Circuit(
        name      = "zk_fba_mcv",
        pkg_flag  = "--package zk_fba_mcv",
        json      = "target/zk_fba_mcv.json",
        witness   = "target/zk_fba_mcv.gz",
        vk_dir    = "target/vk_mcv",
        proof_dir = "target/proof_mcv",
    ),
    Circuit(
        name      = "zk_fba_noir",
        pkg_flag  = "--package zk_fba_noir",
        json      = "target/zk_fba_noir.json",
        witness   = "target/zk_fba_noir.gz",
        vk_dir    = "target/vk",
        proof_dir = "target/proof_out",
    ),
]

# -- Rust Criterion means (ms) -------------------------------------------------
RUST_PROVE_MS  = 10.56    # Layers 1-3f, excl. pairing
RUST_VERIFY_MS = 15.71    # Layer 3f pairing verification
RUST_TOTAL_MS  = 27.52    # full pipeline

# -- helpers -------------------------------------------------------------------
W = 76

def sep(c="-", n=W): print(c * n)

def check_tool(name: str) -> str:
    path = shutil.which(name)
    if path:
        return path
    print(f"\n  '{name}' not found in PATH.\n")
    sys.exit(1)

def tool_version(name: str) -> str:
    r = subprocess.run([name, "--version"], capture_output=True, text=True)
    line = (r.stdout or r.stderr).strip().splitlines()[0] if r.returncode == 0 else "?"
    return line

def run_cmd(cmd: str, label: str, silent: bool = False) -> float:
    """Run a shell command, time it. Returns elapsed ms. Exits on failure."""
    if not silent:
        dots = max(1, 54 - len(label))
        print(f"  {label} {'.' * dots}", end="", flush=True)
    t0 = time.perf_counter()
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True, cwd=HERE)
    ms = (time.perf_counter() - t0) * 1_000
    if result.returncode != 0:
        if not silent:
            print(f" FAILED (exit {result.returncode})")
        if result.stderr.strip():
            lines = result.stderr.strip().splitlines()[-10:]
            print("\n".join("    " + l for l in lines))
        sys.exit(1)
    if not silent:
        print(f" {ms:>8.1f} ms")
    return ms


def ci95(data: list[float]) -> tuple[float, float]:
    """95 % confidence interval using t-distribution (same as Criterion)."""
    n = len(data)
    if n < 2:
        m = data[0] if data else 0.0
        return m, m
    m   = statistics.mean(data)
    s   = statistics.stdev(data)
    # t critical value for 95% CI (two-tailed) -- tabulated for small n
    T95 = {2:12.706,3:4.303,4:3.182,5:2.776,6:2.571,7:2.447,8:2.365,
           9:2.306,10:2.262,11:2.228,12:2.201,13:2.179,14:2.160,
           15:2.145,16:2.131,17:2.120,18:2.110,19:2.101,20:2.093,
           25:2.064,30:2.042,40:2.021,50:2.009,60:2.000,120:1.980}
    keys = sorted(T95)
    t = T95.get(n) or T95[min(keys, key=lambda k: abs(k - n))]
    margin = t * s / math.sqrt(n)
    return m - margin, m + margin


def bench_loop(cmd: str, n_samples: int, n_warmup: int, label: str) -> list[float]:
    """Run cmd n_warmup + n_samples times; return the n_samples timed results."""
    if n_warmup > 0:
        warmup_times = []
        for _ in range(n_warmup):
            warmup_times.append(run_cmd(cmd, "", silent=True))
        avg_warmup = statistics.mean(warmup_times)
        print(f"  Warm-up ({n_warmup} iters): {avg_warmup:.1f} ms / iter")

    samples = []
    bar_w = 28
    for i in range(n_samples):
        ms = run_cmd(cmd, "", silent=True)
        samples.append(ms)
        filled = int(bar_w * (i + 1) / n_samples)
        bar = "#" * filled + "." * (bar_w - filled)
        mean_so_far = statistics.mean(samples)
        print(f"\r  [{bar}] {i+1:>3}/{n_samples}  last={ms:6.1f} ms  mean={mean_so_far:6.1f} ms",
              end="", flush=True)
    print()
    return samples


# -- main ----------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(description="ZK-FBA sub-circuit benchmark")
    parser.add_argument("-n", "--samples",  type=int, default=10,
                        help="Number of timed samples per benchmark (default: 10)")
    parser.add_argument("-w", "--warmup",   type=int, default=2,
                        help="Warm-up iterations before timing (default: 2)")
    parser.add_argument("--quick",          action="store_true",
                        help="Skip warm-up (--warmup 0) and use 5 samples")
    args = parser.parse_args()

    if args.quick:
        args.warmup  = 0
        args.samples = 5

    check_tool("nargo")
    check_tool("bb")
    nargo_ver = tool_version("nargo")
    bb_ver    = tool_version("bb")

    print()
    sep("=")
    print("  ZK-FBA Noir  --  Sub-circuit benchmark")
    print("  Barretenberg UltraHonk  |  BN254")
    sep("=")
    print(f"  nargo   : {nargo_ver}")
    print(f"  bb      : {bb_ver}")
    print(f"  samples : {args.samples}  |  warm-up: {args.warmup}")
    sep()

    # -- Gate count summary ----------------------------------------------------
    print()
    print("  CIRCUIT SIZES")
    sep()
    print(f"  {'Circuit':<24} {'ACIR ops':>9} {'Brillig':>8} {'UH gates':>9} {'Domain':>9}  Constraints")
    sep()
    for c in CIRCUITS:
        gi = GATE_INFO[c.name]
        print(f"  {c.name:<24} {gi['acir_opcodes']:>9} {gi['brillig_opcodes']:>8} "
              f"{gi['circuit_size']:>9} {gi['domain_size']:>9}  {gi['constraints']}")
    sep()
    print()
    print("  All three circuits share the same 2^12 = 4096 UltraHonk domain")
    print("  (gate counts 3284, 3621, 3970 all round up to the same power of 2).")
    print("  Prove times will be similar despite the different constraint counts.")
    print()

    # -- One-time setup --------------------------------------------------------
    sep("-")
    print("  One-time setup (compile + write_vk for each circuit):")
    sep("-")

    for c in CIRCUITS:
        print(f"\n  [{c.name}]")
        run_cmd(f"nargo compile {c.pkg_flag}",
                f"nargo compile  {c.pkg_flag}")
        run_cmd(f"nargo execute {c.pkg_flag}",
                f"nargo execute  {c.pkg_flag}")
        run_cmd(f"bb write_vk -b ./{c.json} -o ./{c.vk_dir}",
                f"bb write_vk    {c.pkg_flag}")

    # -- Benchmarks ------------------------------------------------------------
    results: dict[str, dict[str, list[float]]] = {}

    for c in CIRCUITS:
        cmd_prove = (
            f"bb prove -b ./{c.json} -w ./{c.witness} "
            f"-k ./{c.vk_file} -o ./{c.proof_dir}"
        )
        # bb verify always needs -i, even when public_inputs is 0 bytes
        cmd_verify = (
            f"bb verify -k ./{c.vk_file} "
            f"-p ./{c.proof_file} -i ./{c.public_inputs_file}"
        )

        print()
        sep("-")
        print(f"  Benchmarking: [{c.name}]  prove")
        sep("-")
        prove_samples = bench_loop(cmd_prove, args.samples, args.warmup,
                                   f"{c.name} prove")

        print()
        sep("-")
        print(f"  Benchmarking: [{c.name}]  verify")
        sep("-")
        verify_samples = bench_loop(cmd_verify, args.samples, args.warmup,
                                    f"{c.name} verify")

        results[c.name] = {
            "prove":  prove_samples,
            "verify": verify_samples,
        }

    # -- Results table ---------------------------------------------------------
    print()
    sep("=")
    print("  RESULTS  --  prove and verify (ms)  |  mean [CI lo .. CI hi]")
    sep("=")
    print(f"  {'Circuit':<24}  {'prove mean':>11}  {'95% CI':>20}  {'verify mean':>12}  {'95% CI':>20}")
    sep()

    for c in CIRCUITS:
        r = results[c.name]
        pm  = statistics.mean(r["prove"])
        plo, phi = ci95(r["prove"])
        vm  = statistics.mean(r["verify"])
        vlo, vhi = ci95(r["verify"])
        print(f"  {c.name:<24}  {pm:>10.1f}ms  [{plo:7.1f} .. {phi:7.1f}]ms"
              f"  {vm:>11.1f}ms  [{vlo:7.1f} .. {vhi:7.1f}]ms")

    sep()
    print()

    # -- Prove+Verify totals ---------------------------------------------------
    sep("=")
    print("  PROVE + VERIFY  totals  vs  Rust Criterion means")
    sep("=")
    print(f"  {'Circuit':<24}  {'P+V mean':>10}  {'std-dev':>9}  {'vs Rust full':>13}")
    sep()
    for c in CIRCUITS:
        r  = results[c.name]
        pv = [p + v for p, v in zip(r["prove"], r["verify"])]
        mean_pv = statistics.mean(pv)
        std_pv  = statistics.stdev(pv) if len(pv) > 1 else 0.0
        ratio   = mean_pv / RUST_TOTAL_MS
        print(f"  {c.name:<24}  {mean_pv:>9.1f}ms  {std_pv:>8.2f}ms  {ratio:>12.2f}x")
    sep()

    # -- Detailed stats --------------------------------------------------------
    print()
    sep("=")
    print("  DETAILED STATISTICS")
    sep("=")
    print(f"  {'Benchmark':<40} {'mean':>10} {'std-dev':>10} {'min':>10} {'max':>10}  n")
    sep()
    for c in CIRCUITS:
        for phase in ("prove", "verify"):
            data  = results[c.name][phase]
            m     = statistics.mean(data)
            s     = statistics.stdev(data) if len(data) > 1 else 0.0
            label = f"{c.name} {phase}"
            print(f"  {label:<40} {m:>9.2f}ms {s:>9.2f}ms "
                  f"{min(data):>9.2f}ms {max(data):>9.2f}ms  {len(data)}")
    sep()

    # -- Domain size analysis --------------------------------------------------
    print()
    sep("-")
    print("  WHY SUB-CIRCUITS ARE NOT FASTER THAN THE FULL CIRCUIT")
    sep("-")
    print()
    print("  All three circuits have the same UltraHonk domain: 2^12 = 4096 points.")
    print("  The dominant cost in bb prove is the FFT/IFFT over the domain, which")
    print("  scales as O(n log n).  Since n = 4096 for all three circuits,")
    print("  the polynomial arithmetic cost is identical regardless of gate count.")
    print()
    print("  The gate counts within the same domain are:")
    for c in CIRCUITS:
        gi = GATE_INFO[c.name]
        print(f"    {c.name:<24}  {gi['circuit_size']:>4} gates / 4096 domain"
              f"  = {100*gi['circuit_size']//4096:>2}% utilisation")
    print()
    print("  To see a meaningful timing difference between sub-circuits,")
    print("  the gate count difference would need to push across a power-of-two")
    print("  boundary (e.g. 2^11=2048 vs 2^12=4096).")
    print()
    print("  Rust analogue: the five quotient polynomials each use the same")
    print("  32-point evaluation domain, so there is no per-constraint speedup")
    print("  there either -- the isolation is conceptual, not algorithmic.")
    sep("-")
    print()


if __name__ == "__main__":
    main()
