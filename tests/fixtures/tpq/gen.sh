#!/usr/bin/env bash
# Regenerate all tpq reference fixtures (input PNGs, quantized output PNGs,
# and manifest.json) for bit-exact Rust-port parity testing.
#
# Assumes tilepalquant has been built on the `perf-fixes` branch at:
#   ../../../../tilepalquant/tpq   (relative to this script's directory)
# e.g.:
#   cd ../../../../tilepalquant && git checkout perf-fixes && \
#     c++ -std=c++11 -O2 -Isrc src/*.cpp -o tpq
#
# Usage: ./gen.sh   (run from anywhere; paths are resolved relative to this script)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

TPQ_BIN="../../../../tilepalquant/tpq"

if [[ ! -x "$TPQ_BIN" ]]; then
    echo "error: tpq binary not found/executable at $SCRIPT_DIR/$TPQ_BIN" >&2
    echo "Build it first from the tilepalquant repo's perf-fixes branch:" >&2
    echo "  c++ -std=c++11 -O2 -Isrc src/*.cpp -o tpq" >&2
    exit 1
fi

echo "== Generating input images =="
python3 gen_inputs.py

echo "== Running tpq cases (TPQ_FIXED_SHUFFLE=1) =="
FAILED=0
while IFS=$'\t' read -r -a fields; do
    input="${fields[0]}"
    output="${fields[1]}"
    case_name="${fields[2]}"
    args=("${fields[@]:3}")

    echo "-- $case_name  ($input -> $output)"
    if ! TPQ_FIXED_SHUFFLE=1 "$TPQ_BIN" "$input" -o "$output" "${args[@]}"; then
        echo "   FAILED: $case_name" >&2
        FAILED=1
    fi
done < <(python3 gen_manifest.py --print-commands)

echo "== Writing manifest.json =="
python3 gen_manifest.py

if [[ "$FAILED" -ne 0 ]]; then
    echo "One or more cases failed; see manifest.json for per-case status." >&2
    exit 1
fi

echo "Done."
