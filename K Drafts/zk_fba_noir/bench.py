#!/usr/bin/env python3
"""
ZK-FBA Noir -- Criterion-style benchmark
========================================
Mirrors the methodology of `cargo bench` (Criterion) used in ../zk_fba:
  * Warm-up phase  : N_WARMUP iterations to stabilise CPU caches / branch
                     predictors (Criterion warms up for 3 s; we warm up for
                     N_WARMUP full prove+verify cycles, default 3).
  * Measurement    : N_SAMPLES timed iterations per benchmark.
  * Statistics     : mean, std-dev, min, max, 95 % confidence interval
                     (t-distribution, same as Criterion's bootstrap interval).
  * Outlier report : Tukey IQR fence method -- the same algorithm Criterion uses
                     to flag high/low mild and severe outliers.

Benchmarks run (in order):
  nargo_execute      witness generation      ~ Rust layer4_verify_constraints
  bb_prove           proof generation        ~ Rust layer3f_opening_proofs_13_msms
                                               + all earlier layers
  bb_verify          pairing verification    ~ Rust layer3f_pairing_verify_all
  prove_plus_verify  end-to-end proof work   ~ Rust full_pipeline_end_to_end

Setup phases (compile, write_vk) are run once before the loop and are NOT
included in the benchmark timings, mirroring Criterion which does not
benchmark one-time initialisation inside the hot loop.

Usage
-----
    cd zk_fba_noir
    python3 bench.py                  # 20 samples, 3 warm-up (default)
    python3 bench.py -n 50            # 50 samples
    python3 bench.py -n 100 -w 5      # 100 samples, 5 warm-up
    python3 bench.py --no-warmup -n 5 # quick smoke test, no warm-up

Install (if nargo / bb are missing)
------------------------------------
  # nargo
  curl -L https://raw.githubusercontent.com/noir-lang/noirup/main/install | bash
  source ~/.zshrc && noirup

  # bb (Barretenberg)
  curl -L https://raw.githubusercontent.com/AztecProtocol/aztec-packages/refs/heads/master/barretenberg/bbup/bbup -o /tmp/bbup
  chmod +x /tmp/bbup && /tmp/bbup && source ~/.zshrc
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

# -- Project root --------------------------------------------------------------
HERE = os.path.dirname(os.path.abspath(__file__))

# -- Commands ------------------------------------------------------------------
CMD_COMPILE = "nargo compile --package zk_fba_noir"
CMD_EXECUTE = "nargo execute --package zk_fba_noir"
CMD_VK      = "bb write_vk -b ./target/zk_fba_noir.json -o ./target/vk"
CMD_PROVE   = "bb prove -b ./target/zk_fba_noir.json -w ./target/zk_fba_noir.gz -k ./target/vk/vk -o ./target/proof_out"
CMD_VERIFY  = "bb verify -k ./target/vk/vk -p ./target/proof_out/proof -i ./target/proof_out/public_inputs"

# -- Rust Criterion means (ms) from ../zk_fba/README.md ------------------------
# All values: statistical means over 100 samples, warm cache, Apple M4 Max.
RUST_MS: dict[str, float] = {
    "layer1_compute_all_arrays":         0.000681,
    "layer2_interpolate_5_polys":        0.00760,
    "layer3a_kzg_trusted_setup":         1.699,
    "layer3b_kzg_commit_witnesses_5":    2.222,
    "layer3c_compute_quotient_polys_5":  0.03213,
    "layer3d_kzg_commit_quotients_5":    1.405,
    "layer3e_fiat_shamir_prove":         0.00861,
    "layer3f_opening_proofs_13_msms":    5.185,
    "layer3f_pairing_verify_all":        15.71,
    "layer4_verify_constraints":         0.0666,
    "full_pipeline_end_to_end":          27.52,
}

# Rust prove-side subtotal (all layers except pairing verify)
_RUST_PROVE_LAYERS = [
    "layer1_compute_all_arrays",
    "layer2_interpolate_5_polys",
    "layer3a_kzg_trusted_setup",
    "layer3b_kzg_commit_witnesses_5",
    "layer3c_compute_quotient_polys_5",
    "layer3d_kzg_commit_quotients_5",
    "layer3e_fiat_shamir_prove",
    "layer3f_opening_proofs_13_msms",
]
RUST_PROVE_MS = sum(RUST_MS[k] for k in _RUST_PROVE_LAYERS)

# -- Formatting helpers ---------------------------------------------------------

def fmt(ms: float) -> str:
    """Human-readable duration from milliseconds."""
    if ms >= 60_000:
        return f"{ms / 60_000:.2f} min"
    if ms >= 1_000:
        return f"{ms / 1_000:.3f} s  "
    if ms >= 1:
        return f"{ms:.3f} ms"
    if ms >= 0.001:
        return f"{ms * 1_000:.2f} us"
    return f"{ms * 1_000_000:.0f} ns"

def sep(c: str = "-", n: int = 76) -> None:
    print(c * n)

# -- Statistics -----------------------------------------------------------------

# t-distribution critical values for two-tailed 95 % CI (df = n-1).
# Matches Criterion's bootstrap interval width for small n.
_T95 = {
     1: 12.706,  2: 4.303,  3: 3.182,  4: 2.776,  5: 2.571,
     6:  2.447,  7: 2.365,  8: 2.306,  9: 2.262, 10: 2.228,
    11:  2.201, 12: 2.179, 13: 2.160, 14: 2.145, 15: 2.131,
    16:  2.120, 17: 2.110, 18: 2.101, 19: 2.093, 20: 2.086,
    25:  2.060, 30: 2.042, 40: 2.021, 50: 2.009, 60: 2.000,
    80:  1.990, 99: 1.984,
}

def t95(n: int) -> float:
    df = n - 1
    if df in _T95:
        return _T95[df]
    # linear interpolation for df between table entries
    keys = sorted(_T95.keys())
    for i in range(len(keys) - 1):
        if keys[i] <= df <= keys[i + 1]:
            lo, hi = keys[i], keys[i + 1]
            frac = (df - lo) / (hi - lo)
            return _T95[lo] + frac * (_T95[hi] - _T95[lo])
    return 1.960  # large-sample fallback


@dataclass
class Stats:
    samples: list[float]
    mean:    float = 0.0
    sd:      float = 0.0
    lo_ci:   float = 0.0  # 95 % CI lower bound
    hi_ci:   float = 0.0  # 95 % CI upper bound
    minimum: float = 0.0
    maximum: float = 0.0
    # Outlier counts (Tukey IQR fences, same as Criterion)
    low_severe:  int = 0
    low_mild:    int = 0
    high_mild:   int = 0
    high_severe: int = 0

    def __post_init__(self) -> None:
        s = self.samples
        n = len(s)
        self.mean    = statistics.mean(s)
        self.sd      = statistics.stdev(s) if n > 1 else 0.0
        self.minimum = min(s)
        self.maximum = max(s)
        if n > 1:
            se = self.sd / math.sqrt(n)
            half = t95(n) * se
            self.lo_ci = self.mean - half
            self.hi_ci = self.mean + half
        else:
            self.lo_ci = self.hi_ci = self.mean
        # Tukey outlier detection
        q1 = statistics.quantiles(s, n=4)[0]
        q3 = statistics.quantiles(s, n=4)[2]
        iqr = q3 - q1
        for v in s:
            if   v < q1 - 3.0 * iqr: self.low_severe  += 1
            elif v < q1 - 1.5 * iqr: self.low_mild    += 1
            elif v > q3 + 3.0 * iqr: self.high_severe += 1
            elif v > q3 + 1.5 * iqr: self.high_mild   += 1

    @property
    def n_outliers(self) -> int:
        return self.low_severe + self.low_mild + self.high_mild + self.high_severe


# -- Shell execution ------------------------------------------------------------

def check_tool(name: str) -> None:
    if not shutil.which(name):
        print(f"\n  x  '{name}' not found in PATH.\n")
        if name == "nargo":
            print("  Install nargo:")
            print("    curl -L https://raw.githubusercontent.com/noir-lang/noirup/main/install | bash")
            print("    source ~/.zshrc && noirup\n")
        elif name == "bb":
            print("  Install bb (Barretenberg):")
            print("    curl -L https://raw.githubusercontent.com/AztecProtocol/aztec-packages/"
                  "refs/heads/master/barretenberg/bbup/bbup -o /tmp/bbup")
            print("    chmod +x /tmp/bbup && /tmp/bbup && source ~/.zshrc\n")
        sys.exit(1)


def run(cmd: str, label: str = "", silent: bool = False) -> float:
    """Run *cmd*, return elapsed wall-clock milliseconds. Exit on failure."""
    if label and not silent:
        print(f"  {label}", end="", flush=True)
    t0 = time.perf_counter()
    r  = subprocess.run(cmd, shell=True, capture_output=True, text=True, cwd=HERE)
    ms = (time.perf_counter() - t0) * 1_000
    if r.returncode != 0:
        suffix = f" <- FAILED (exit {r.returncode})\n"
        print(suffix if label else "")
        tail = lambda s: "\n".join(("    " + l) for l in s.strip().splitlines()[-15:])
        if r.stdout.strip(): print(f"  stdout:\n{tail(r.stdout)}")
        if r.stderr.strip(): print(f"  stderr:\n{tail(r.stderr)}")
        sys.exit(1)
    if label and not silent:
        print(f"  {fmt(ms)}", flush=True)
    return ms


def tool_version(name: str) -> str:
    r = subprocess.run([name, "--version"], capture_output=True, text=True)
    lines = (r.stdout or r.stderr).strip().splitlines()
    return lines[0] if lines else "?"


# -- Benchmark loop -------------------------------------------------------------

@dataclass
class BenchResult:
    label:       str
    rust_key:    Optional[str]
    rust_ms:     Optional[float]
    stats:       Stats
    warmup_n:    int
    warmup_ms:   list[float] = field(default_factory=list)


def run_bench(
    label:    str,
    cmd:      str,
    n_warmup: int,
    n_samples: int,
    rust_key: Optional[str] = None,
) -> BenchResult:
    """
    Run *cmd* n_warmup + n_samples times.
    Display a live progress bar identical to Criterion's collector display.
    Return a BenchResult with full statistics.
    """
    rust_ms = RUST_MS.get(rust_key) if rust_key else None

    BAR = 30
    print(f"\n  Benchmarking: {label}")

    # -- Warm-up --------------------------------------------------------------
    warmup_times: list[float] = []
    if n_warmup > 0:
        print(f"  Warming up ({n_warmup} iteration{'s' if n_warmup > 1 else ''}) ...", end="", flush=True)
        for _ in range(n_warmup):
            warmup_times.append(run(cmd, silent=True))
            print(".", end="", flush=True)
        wu_mean = statistics.mean(warmup_times)
        print(f"  {fmt(wu_mean)} / iter")

    # -- Measurement ----------------------------------------------------------
    est_total_s = (statistics.mean(warmup_times) if warmup_times else 1_000) * n_samples / 1_000
    print(f"  Collecting {n_samples} sample{'s' if n_samples > 1 else ''}"
          f"  (est. {est_total_s:.0f} s total)")

    samples: list[float] = []
    for i in range(1, n_samples + 1):
        ms = run(cmd, silent=True)
        samples.append(ms)
        filled = round(BAR * i / n_samples)
        bar    = "#" * filled + "." * (BAR - filled)
        cur_mean = statistics.mean(samples)
        print(f"\r  [{bar}] {i:>{len(str(n_samples))}}/{n_samples}"
              f"  last={fmt(ms):>10}  mean={fmt(cur_mean):>10}", end="", flush=True)
    print()  # newline after progress bar

    return BenchResult(
        label     = label,
        rust_key  = rust_key,
        rust_ms   = rust_ms,
        stats     = Stats(samples),
        warmup_n  = n_warmup,
        warmup_ms = warmup_times,
    )


# -- Report printing ------------------------------------------------------------

def print_criterion_block(res: BenchResult) -> None:
    """
    Print one benchmark result in Criterion's standard format:

      bench_name       time:  [lo_CI   mean   hi_CI]
                       change: (no change / N% faster / N% slower vs Rust)
      Found X outliers among N measurements (Y.YY%)
        X (Y.YY%) high severe
    """
    s = res.stats
    n = len(s.samples)

    # Header line  -  name + [lo  mean  hi]
    ci_str = f"[{fmt(s.lo_ci):>10}  {fmt(s.mean):>10}  {fmt(s.hi_ci):>10}]"
    name   = res.label[:38]
    print(f"  {name:<40} time:  {ci_str}")

    # Change vs Rust
    if res.rust_ms:
        ratio = s.mean / res.rust_ms
        if ratio > 1.05:
            change = f"+{(ratio - 1)*100:.1f}%  ({ratio:.1f}x slower than Rust)"
        elif ratio < 0.95:
            change = f"{(ratio - 1)*100:.1f}%  ({1/ratio:.1f}x faster than Rust)"
        else:
            change = "no change  (within 5% of Rust)"
        print(f"  {'':40} change: {change}")

    # Outliers
    if s.n_outliers > 0:
        pct = s.n_outliers / n * 100
        print(f"  Found {s.n_outliers} outlier{'s' if s.n_outliers > 1 else ''}"
              f" among {n} measurements ({pct:.2f}%)")
        for count, label in [
            (s.low_severe,  "low  severe"),
            (s.low_mild,    "low  mild  "),
            (s.high_mild,   "high mild  "),
            (s.high_severe, "high severe"),
        ]:
            if count:
                print(f"    {count} ({count/n*100:.2f}%)  {label}")
    else:
        print(f"  Found 0 outliers among {n} measurements (0.00%)")


def print_summary_table(results: list[BenchResult], setup_ms: dict[str, float]) -> None:
    """
    Print the side-by-side comparison table analogous to the README table in
    ../zk_fba (Criterion means vs Noir wall-clock means).
    """
    sep("=")
    print("  SUMMARY  --  Noir bench means vs Rust Criterion means  (Apple M4 Max)")
    sep("=")
    print(f"  {'Benchmark':<42}  {'Noir mean':>12}  {'Rust mean':>12}  {'ratio':>8}")
    sep()

    for r in results:
        m = r.stats.mean
        rust_str  = f"{r.rust_ms:.3f} ms" if r.rust_ms else "--"
        ratio_str = f"{m / r.rust_ms:.1f}x" if r.rust_ms else "--"
        print(f"  {r.label:<42}  {fmt(m):>12}  {rust_str:>12}  {ratio_str:>8}")

    sep()

    # Setup phases (one-time, not benchmarked in the loop)
    print()
    print(f"  One-time setup phases  (not in benchmark loop):")
    sep("-")
    rust_setup_map = {
        "nargo compile":    None,
        "bb write_vk":      "layer3a_kzg_trusted_setup",
    }
    for phase, ms in setup_ms.items():
        rk  = rust_setup_map.get(phase)
        r   = RUST_MS.get(rk, 0.0) if rk else None
        rs  = f"{r:.3f} ms" if r else "--"
        rat = f"{ms / r:.1f}x" if r else "--"
        print(f"  {phase:<42}  {fmt(ms):>12}  {rs:>12}  {rat:>8}")

    sep("-")
    print()
    print("  Notes:")
    print(f"  . Rust figures: Criterion means, 100 samples, warm cache")
    print(f"  . Noir figures: single-run wall-clock, warm cache (after warm-up)")
    print(f"  . Rust `full_pipeline` includes a fresh tau per iteration (~1.7 ms KZG setup).")
    print(f"    Barretenberg uses a fixed ceremony SRS -- no per-proof setup cost.")
    print(f"  . Proof systems differ: Rust = bespoke PLONK (degree-31, 32-point domain);")
    print(f"    Noir = UltraHonk (general circuit, larger polynomial domain).")
    sep()


def print_detailed_stats(results: list[BenchResult]) -> None:
    sep("-")
    print("  DETAILED STATISTICS")
    sep("-")
    hdr = f"  {'Benchmark':<30}  {'mean':>10}  {'std-dev':>10}  {'min':>10}  {'max':>10}  {'n':>4}"
    print(hdr)
    sep("-")
    for r in results:
        s = r.stats
        print(f"  {r.label:<30}  {fmt(s.mean):>10}  {fmt(s.sd):>10}"
              f"  {fmt(s.minimum):>10}  {fmt(s.maximum):>10}  {len(s.samples):>4}")
    sep("-")


# -- Main ----------------------------------------------------------------------

def main() -> None:
    parser = argparse.ArgumentParser(
        description="ZK-FBA Noir -- Criterion-style benchmark",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__.split("Usage")[1].split("Install")[0].strip() if "Usage" in __doc__ else "",
    )
    parser.add_argument("-n", "--samples",   type=int, default=20,
                        help="timed samples per benchmark (default: 20)")
    parser.add_argument("-w", "--warmup",    type=int, default=3,
                        help="warm-up iterations (default: 3)")
    parser.add_argument("--no-warmup",       action="store_true",
                        help="skip warm-up phase")
    parser.add_argument("--no-execute",      action="store_true",
                        help="skip nargo_execute benchmark (saves time)")
    args = parser.parse_args()

    n_warmup  = 0 if args.no_warmup else args.warmup
    n_samples = args.samples
    os.chdir(HERE)

    # Tool presence check
    check_tool("nargo")
    check_tool("bb")

    nargo_ver = tool_version("nargo")
    bb_ver    = tool_version("bb")

    # Rough time estimate: each bb prove ~ 3 s (conservatively)
    est_prove_s = 3.0
    est_total_s = (n_warmup + n_samples) * (est_prove_s + 0.4) * (3 if not args.no_execute else 2)

    print()
    sep("=")
    print("  ZK-FBA Noir  --  Criterion-style benchmark")
    print("  Barretenberg UltraHonk  |  BN254")
    sep("=")
    print(f"  nargo    : {nargo_ver}")
    print(f"  bb       : {bb_ver}")
    print(f"  samples  : {n_samples}  |  warm-up: {n_warmup}  |  est. runtime: ~{est_total_s:.0f} s")
    sep()
    print()

    # -- One-time setup -------------------------------------------------------
    print("  One-time setup (not benchmarked):")
    t_compile = run(CMD_COMPILE, "  nargo compile  ...")
    t_vk      = run(CMD_VK,      "  bb write_vk    ...")
    # Execute once to produce witness.gz for bb prove to consume
    run(CMD_EXECUTE, "  nargo execute  (initial witness) ...")
    setup_ms  = {"nargo compile": t_compile, "bb write_vk": t_vk}
    print()

    results: list[BenchResult] = []

    # -- Benchmark 1 : nargo execute ------------------------------------------
    if not args.no_execute:
        results.append(run_bench(
            label    = "nargo_execute (witness gen)",
            cmd      = CMD_EXECUTE,
            n_warmup = n_warmup,
            n_samples = n_samples,
            rust_key = "layer4_verify_constraints",
        ))

    # -- Benchmark 2 : bb prove -----------------------------------------------
    results.append(run_bench(
        label    = "bb_prove (commit+quotient+FS+open)",
        cmd      = CMD_PROVE,
        n_warmup = n_warmup,
        n_samples = n_samples,
        rust_key  = None,   # no single Rust layer maps 1:1; use table note
    ))

    # -- Benchmark 3 : bb verify ----------------------------------------------
    results.append(run_bench(
        label    = "bb_verify (pairing verification)",
        cmd      = CMD_VERIFY,
        n_warmup = 1,        # proof file stable after bb_prove; minimal warm-up
        n_samples = n_samples,
        rust_key  = "layer3f_pairing_verify_all",
    ))

    # -- Derived : prove + verify ----------------------------------------------
    prove_s = next(r for r in results if r.label.startswith("bb_prove"))
    verify_s = next(r for r in results if r.label.startswith("bb_verify"))
    if len(prove_s.stats.samples) == len(verify_s.stats.samples):
        combined = [p + v for p, v in zip(prove_s.stats.samples, verify_s.stats.samples)]
    else:
        # lengths differ (e.g. different warmup counts) -- pair up to min length
        n_min    = min(len(prove_s.stats.samples), len(verify_s.stats.samples))
        combined = [prove_s.stats.samples[i] + verify_s.stats.samples[i] for i in range(n_min)]

    combined_result = BenchResult(
        label    = "prove_plus_verify (end-to-end)",
        rust_key = "full_pipeline_end_to_end",
        rust_ms  = RUST_MS["full_pipeline_end_to_end"],
        stats    = Stats(combined),
        warmup_n = 0,
    )
    results.append(combined_result)

    # -- Rust prove subtotal pseudo-result for table ---------------------------
    # (synthetic entry so the table shows the Rust prove-side subtotal)
    class _RustProveRow:
        label   = "  [Rust layers 1-3f prove subtotal]"
        rust_ms = RUST_PROVE_MS
        stats   = type("_", (), {"mean": None})()

    # -- Print results ---------------------------------------------------------
    print()
    sep("=")
    print("  CRITERION-STYLE RESULTS")
    sep("=")

    for res in results:
        print_criterion_block(res)
        print()

    print_detailed_stats(results)
    print()
    print_summary_table(results, setup_ms)

    # -- Proof size -------------------------------------------------------------
    proof_path = os.path.join(HERE, "target", "proof_out", "proof")
    if os.path.exists(proof_path):
        proof_bytes = os.path.getsize(proof_path)
        print(f"  Proof artifact  : {proof_bytes:,} bytes  ({proof_bytes // 1024} KB)")
        print(f"  Rust proof size : ~1,200 bytes (10 comms + 13 evals + 13 openings)")
        print()


if __name__ == "__main__":
    main()
