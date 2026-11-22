#!/usr/bin/env bash
# Times witness generation, proving and verification over repeated runs.
# Usage: bench.sh OUT.csv RUNS THREADS BOOK.csv...
set -euo pipefail
export PATH="$PATH:$HOME/.nargo/bin:$HOME/.bb"

src="$(cd "$(dirname "$0")" && pwd)"
out="$(realpath "$1")"
runs=$2
export HARDWARE_CONCURRENCY=$3
shift 3
work="$HOME/zq_noir_bench_work"
target=noir-recursive

# Runs a command quietly and sets ms to its wall time.
timed() {
    local t0
    t0=$(date +%s%N)
    "$@" >"$work/last.log" 2>&1
    ms=$(( ($(date +%s%N) - t0) / 1000000 ))
}

echo "file,ticks,circuit,gates,threads,run,witness_ms,vk_ms,prove_ms,verify_ms,prove_kb,proof_bytes,output" >"$out"
for book in "$@"; do
    book="$(realpath "$book")"
    rm -rf "$work" && mkdir -p "$work"
    cp -r "$src/Nargo.toml" "$src/lib" "$src/clearing" "$src/no_cross" "$work/"
    cd "$work"

    # Input file, book size and which circuit applies.
    info=$(python3 "$src/book_to_toml.py" "$book" input.toml)
    ticks=$(echo "$info" | grep -o 'ticks=[0-9]*' | cut -d= -f2)
    circuit=$(echo "$info" | grep -o 'circuit=[a-z_]*' | cut -d= -f2)
    sed -i "s/^global N: u32 = [0-9]*;/global N: u32 = $ticks;/" "$work"/*/src/main.nr
    cp input.toml "$circuit/Prover.toml"
    nargo compile --package "$circuit" >/dev/null
    gates=$(bb gates -b "target/$circuit.json" 2>/dev/null | grep -o '"circuit_size": *[0-9]*' | grep -o '[0-9]*$')
    timed bb write_vk -t "$target" -b "target/$circuit.json" -o target/vk
    vk_ms=$ms

    for run in $(seq 0 $((runs - 1))); do
        timed nargo execute --package "$circuit"
        witness_ms=$ms
        output=$(grep -o 'Circuit output.*' "$work/last.log" | sed 's/\x1b\[[0-9;]*m//g' | tr -d ',\n' | tr -s ' ')
        timed /usr/bin/time -o "$work/mem.txt" -f "%M" bb prove -t "$target" -b "target/$circuit.json" -w "target/$circuit.gz" -k target/vk/vk -o target/proof
        prove_ms=$ms
        timed bb verify -t "$target" -k target/vk/vk -p target/proof/proof -i target/proof/public_inputs
        verify_ms=$ms
        echo "$(basename "$book"),$ticks,$circuit,$gates,$HARDWARE_CONCURRENCY,$run,$witness_ms,$vk_ms,$prove_ms,$verify_ms,$(cat "$work/mem.txt"),$(stat -c %s target/proof/proof),$output" >>"$out"
    done
done
