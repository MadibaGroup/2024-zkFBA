#!/usr/bin/env python3
"""Timing harness for the real-AAPL-data Noir circuits, same 5-phase
methodology as ~/zk_fba_noir/time_proof.py and the same manual CLI sequence
used to produce the fba_protocol_100/1000 numbers in
OPTIMIZED_RATIONALE_AND_RESULTS.md:

  nargo compile -> nargo execute -> bb write_vk -> bb prove -> bb verify

Runs each circuit folder (fba_protocol_real_quotes, fba_protocol_real_trades)
`runs` times and reports mean +/- stddev per phase, plus proof size.

Usage: python3 time_real_data.py [runs]
"""
import os
import shutil
import statistics
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
CIRCUITS = ["fba_protocol_real_quotes", "fba_protocol_real_trades"]

def find_tool(name):
    p = shutil.which(name)
    if p:
        return p
    for candidate in (os.path.expanduser("~/.nargo/bin"), os.path.expanduser("~/.bb")):
        cand = os.path.join(candidate, name)
        if os.path.exists(cand):
            return cand
    print(f"'{name}' not found (checked PATH, ~/.nargo/bin, ~/.bb)")
    sys.exit(1)

NARGO = find_tool("nargo")
BB = find_tool("bb")

def run(cmd, cwd):
    t0 = time.perf_counter()
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True, cwd=cwd)
    ms = (time.perf_counter() - t0) * 1000
    if result.returncode != 0:
        print(f"FAILED: {cmd}\nstdout: {result.stdout[-2000:]}\nstderr: {result.stderr[-2000:]}")
        sys.exit(1)
    return ms

def one_run(pkg_dir, pkg_name):
    t = {}
    t["compile"] = run(f'"{NARGO}" compile --package {pkg_name}', pkg_dir)
    t["execute"] = run(f'"{NARGO}" execute --package {pkg_name}', pkg_dir)
    t["write_vk"] = run(f'"{BB}" write_vk -b ./target/{pkg_name}.json -o ./target/vk', pkg_dir)
    t["prove"] = run(
        f'"{BB}" prove -b ./target/{pkg_name}.json -w ./target/{pkg_name}.gz '
        f'-k ./target/vk/vk -o ./target/proof_out', pkg_dir)
    t["verify"] = run(
        f'"{BB}" verify -k ./target/vk/vk -p ./target/proof_out/proof '
        f'-i ./target/proof_out/public_inputs', pkg_dir)
    proof_path = os.path.join(pkg_dir, "target", "proof_out", "proof")
    proof_bytes = os.path.getsize(proof_path) if os.path.exists(proof_path) else 0
    return t, proof_bytes

def main():
    runs = int(sys.argv[1]) if len(sys.argv) > 1 else 5
    print(f"nargo: {NARGO}\nbb: {BB}\nruns per circuit: {runs}\n")

    for pkg_name in CIRCUITS:
        pkg_dir = os.path.join(HERE, pkg_name)
        print("=" * 70)
        print(f"  {pkg_name}")
        print("=" * 70)
        all_t = {k: [] for k in ("compile", "execute", "write_vk", "prove", "verify")}
        proof_bytes = 0
        for i in range(runs):
            t, pb = one_run(pkg_dir, pkg_name)
            proof_bytes = pb
            for k in all_t:
                all_t[k].append(t[k])
            print(f"  run {i+1}/{runs}: prove={t['prove']:.1f}ms verify={t['verify']:.1f}ms")

        print(f"\n  Mean +/- stddev over {runs} runs (ms):")
        for k in all_t:
            vals = all_t[k]
            mean = statistics.mean(vals)
            sd = statistics.stdev(vals) if len(vals) > 1 else 0.0
            print(f"    {k:<10} {mean:>8.1f} +/- {sd:.1f}")
        prove_plus_verify = statistics.mean(all_t["prove"]) + statistics.mean(all_t["verify"])
        print(f"    {'prove+verify':<10} {prove_plus_verify:>8.1f}")
        print(f"  Proof size: {proof_bytes:,} bytes")
        print()

if __name__ == "__main__":
    main()
