#!/bin/bash
# Release signing script — generates checksums and signs binaries
# Usage: ./scripts/sign-release.sh v0.1.0

set -e
VERSION="${1:-v0.1.0}"
RELEASE_DIR="target/release"

echo "=== Sparrow Release Signing ==="
echo "Version: $VERSION"
echo ""

# Build for current platform
cargo build --release

# Generate SHA256 checksums
if [ -f "$RELEASE_DIR/sparrow" ]; then
    sha256sum "$RELEASE_DIR/sparrow" > "$RELEASE_DIR/sparrow.sha256"
    echo "SHA256: $(cat $RELEASE_DIR/sparrow.sha256)"
elif [ -f "$RELEASE_DIR/sparrow.exe" ]; then
    sha256sum "$RELEASE_DIR/sparrow.exe" > "$RELEASE_DIR/sparrow.exe.sha256"
    echo "SHA256: $(cat $RELEASE_DIR/sparrow.exe.sha256)"
fi

# Generate GPG signature (if key available)
if command -v gpg &>/dev/null && [ -n "${GPG_KEY:-}" ]; then
    for bin in "$RELEASE_DIR"/sparrow*; do
        if [ -f "$bin" ] && [[ "$bin" != *.sha256 ]] && [[ "$bin" != *.asc ]]; then
            gpg --detach-sign --armor --local-user "$GPG_KEY" "$bin"
            echo "Signed: $bin.asc"
        fi
    done
fi

echo ""
echo "Release artifacts ready in $RELEASE_DIR/"
echo "Upload to: https://github.com/ucav/Sparrow/releases/tag/$VERSION"
