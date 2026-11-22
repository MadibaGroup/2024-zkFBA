#!/usr/bin/env bash
# Proves and verifies the clearing of one order-book CSV.
# Usage: prove.sh BOOK.csv [WORK_DIR]
set -euo pipefail
export PATH="$PATH:$HOME/.nargo/bin:$HOME/.bb"

src="$(cd "$(dirname "$0")" && pwd)"
book="$(realpath "$1")"
work="${2:-$HOME/zq_noir_standalone_work}"
# Zero-knowledge UltraHonk with a Poseidon2 transcript.
target=noir-recursive

rm -rf "$work" && mkdir -p "$work"
cp -r "$src/Nargo.toml" "$src/lib" "$src/clearing" "$src/no_cross" "$work/"
cd "$work"

# Input file, book size and which circuit applies.
info=$(python3 "$src/book_to_toml.py" "$book" input.toml ${SALT:+--salt "$SALT"})
ticks=$(echo "$info" | grep -o 'ticks=[0-9]*' | cut -d= -f2)
circuit=$(echo "$info" | grep -o 'circuit=[a-z_]*' | cut -d= -f2)
sed -i "s/^global N: u32 = [0-9]*;/global N: u32 = $ticks;/" "$work"/*/src/main.nr
cp input.toml "$circuit/Prover.toml"
nargo compile --package "$circuit" >/dev/null

# Runs a command quietly and prints its wall time and peak memory.
timed() {
    local label=$1
    shift
    /usr/bin/time -o "$work/time.txt" -f "$label: %e s, %M KB" "$@" >"$work/$label.log" 2>&1
    cat "$work/time.txt"
}

timed "witness" nargo execute --package "$circuit"
echo "output: $(grep -o 'Circuit output.*' "$work/witness.log" | sed 's/\x1b\[[0-9;]*m//g')"
if [ "${EXECUTE_ONLY:-0}" = 1 ]; then exit 0; fi
gates=$(bb gates -b "target/$circuit.json" 2>/dev/null | grep -o '"circuit_size": *[0-9]*' | grep -o '[0-9]*$')
timed "vk" bb write_vk -t "$target" -b "target/$circuit.json" -o target/vk
timed "prove" bb prove -t "$target" -b "target/$circuit.json" -w "target/$circuit.gz" -k target/vk/vk -o target/proof
timed "verify" bb verify -t "$target" -k target/vk/vk -p target/proof/proof -i target/proof/public_inputs
echo "circuit: $circuit, ticks: $ticks, gates: $gates"
echo "proof: $(stat -c %s target/proof/proof) bytes, public inputs: $(stat -c %s target/proof/public_inputs) bytes, vk: $(stat -c %s target/vk/vk) bytes"
echo "verified"