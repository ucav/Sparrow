#!/bin/bash
# Cross-compile Sparrow for all targets
# Requires: cross (cargo install cross)
# Usage: ./scripts/cross-build.sh

set -e
TARGETS=(
    "x86_64-unknown-linux-musl"
    "aarch64-unknown-linux-musl"
    "x86_64-apple-darwin"
    "aarch64-apple-darwin"
    "x86_64-pc-windows-msvc"
)

echo "=== Sparrow Cross-Compilation ==="

for target in "${TARGETS[@]}"; do
    echo ""
    echo "Building: $target"
    if command -v cross &>/dev/null; then
        cross build --release --target "$target" || echo "  (cross build failed — may need OS-specific toolchain)"
    else
        echo "  (cross not installed — run: cargo install cross)"
    fi
done

echo ""
echo "=== Cross-compilation complete ==="
echo "Binaries in target/<target>/release/sparrow*"
