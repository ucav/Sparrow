#!/usr/bin/env bash
# Reproducible cold-start benchmark for the Sparrow CLI.
#
# Measures the startup latency of light, frequently-run commands with hyperfine.
# Run it before/after any change that touches main.rs startup, the provider
# registry, or boot-time discovery, and compare against artifacts/perf-startup-*.md.
#
# Usage:
#   cargo build --release
#   scripts/perf-startup.sh                # uses ./target/release/sparrow[.exe]
#   SPARROW_BIN=/path/to/sparrow scripts/perf-startup.sh
set -eu

BIN="${SPARROW_BIN:-}"
if [ -z "$BIN" ]; then
    if [ -x "./target/release/sparrow.exe" ]; then BIN="./target/release/sparrow.exe"
    elif [ -x "./target/release/sparrow" ]; then BIN="./target/release/sparrow"
    else echo "No release binary found — run 'cargo build --release' first." >&2; exit 1
    fi
fi

if ! command -v hyperfine >/dev/null 2>&1; then
    echo "hyperfine not found — install it (cargo install hyperfine) for accurate numbers." >&2
    exit 1
fi

echo "Benchmarking: $BIN"
echo ""

# -N runs without an intermediate shell so we measure the process, not bash.
# These commands must NOT trigger network I/O or boot-time model discovery.
hyperfine -N --warmup 5 -r 30 \
    "$BIN --version" \
    "$BIN help" \
    "$BIN auth list" \
    "$BIN memory list"
