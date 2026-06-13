#!/bin/sh
# Sparrow one-click installer for Linux and macOS.
#
# By default this installs the binary and STOPS — it does not launch the agent.
# Run `sparrow launch` yourself when ready, or pass --launch to start at the end.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/ucav/Sparrow/master/install.sh | sh
#   curl -fsSL .../install.sh | sh -s -- --launch            # opt in to auto-launch
#   curl -fsSL .../install.sh | sh -s -- --from-source       # build from git
#   curl -fsSL .../install.sh | sh -s -- --allow-unverified  # skip checksum (NOT recommended)
set -eu

REPO="ucav/Sparrow"
INSTALL_DIR="${SPARROW_INSTALL_DIR:-$HOME/.local/bin}"
BIN_PATH="$INSTALL_DIR/sparrow"
LAUNCH=0          # default: do NOT auto-launch a trust-sensitive agent
FROM_SOURCE=0
SHORTCUT=1
VERIFY=1          # default: verify SHA256 of downloaded release binary

while [ "${1:-}" ]; do
    case "$1" in
        --dir)
            shift
            INSTALL_DIR="${1:-}"
            if [ -z "$INSTALL_DIR" ]; then
                echo "Missing value for --dir" >&2
                exit 1
            fi
            BIN_PATH="$INSTALL_DIR/sparrow"
            ;;
        --from-source)
            FROM_SOURCE=1
            ;;
        --launch)
            LAUNCH=1
            ;;
        --no-launch)
            LAUNCH=0          # back-compat: no-launch is already the default
            ;;
        --allow-unverified)
            VERIFY=0
            ;;
        --no-shortcut)
            SHORTCUT=0
            ;;
        -h|--help)
            echo "Usage: install.sh [--dir PATH] [--from-source] [--launch] [--allow-unverified] [--no-shortcut]"
            echo ""
            echo "  --launch            start the WebView cockpit after install (default: off)"
            echo "  --from-source       build from git instead of a release binary"
            echo "  --allow-unverified  install even if no SHA256 checksum is available (not recommended)"
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
    esac
    shift
done

need() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "Missing required command: $1" >&2
        exit 1
    fi
}

download() {
    url="$1"
    out="$2"
    if command -v curl >/dev/null 2>&1; then
        curl -fL --retry 3 --connect-timeout 20 "$url" -o "$out"
    elif command -v wget >/dev/null 2>&1; then
        wget -O "$out" "$url"
    else
        echo "Install requires curl or wget." >&2
        exit 1
    fi
}

# Compute the SHA256 of a file, portably (Linux: sha256sum, macOS: shasum).
sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | awk '{print $1}'
    else
        echo ""   # no tool available
    fi
}

detect_artifact() {
    os="$(uname -s | tr '[:upper:]' '[:lower:]')"
    arch="$(uname -m)"

    case "$os:$arch" in
        linux:x86_64|linux:amd64) echo "sparrow-linux-x86_64" ;;
        linux:aarch64|linux:arm64) echo "sparrow-linux-aarch64" ;;
        darwin:arm64|darwin:aarch64) echo "sparrow-macos-arm64" ;;
        darwin:x86_64|darwin:amd64) echo "sparrow-macos-x86_64" ;;
        *)
            echo "Unsupported platform: $os $arch" >&2
            echo "Use --from-source if Rust is available, or build from https://github.com/$REPO" >&2
            exit 1
            ;;
    esac
}

install_from_source() {
    need cargo
    need git
    mkdir -p "$INSTALL_DIR"
    tmp="${TMPDIR:-/tmp}/sparrow-install-src-$$"
    rm -rf "$tmp"
    git clone --depth 1 "https://github.com/$REPO.git" "$tmp"
    (
        cd "$tmp"
        cargo build --release
    )
    cp "$tmp/target/release/sparrow" "$BIN_PATH"
    chmod +x "$BIN_PATH"
    rm -rf "$tmp"
}

# Verify $1 (the downloaded binary) against the published <artifact>.sha256.
# Returns 0 = verified, 1 = checksum present but MISMATCH (tampering), 2 = no checksum available.
verify_checksum() {
    bin="$1"
    artifact="$2"
    sums_url="https://github.com/$REPO/releases/latest/download/$artifact.sha256"
    sums="$bin.sha256"

    if ! download "$sums_url" "$sums" 2>/dev/null; then
        rm -f "$sums"
        return 2
    fi

    have="$(sha256_of "$bin")"
    if [ -z "$have" ]; then
        echo "No sha256 tool found (sha256sum/shasum) — cannot verify." >&2
        rm -f "$sums"
        return 2
    fi

    # The sidecar may list any filename in column 2; we only trust the hash token.
    if grep -qi "^$have[[:space:]]" "$sums" || grep -qi "[[:space:]]$have$" "$sums" || grep -qiw "$have" "$sums"; then
        rm -f "$sums"
        return 0
    fi

    rm -f "$sums"
    return 1
}

install_from_release() {
    artifact="$(detect_artifact)"
    url="https://github.com/$REPO/releases/latest/download/$artifact"
    mkdir -p "$INSTALL_DIR"
    tmp="$BIN_PATH.tmp"

    echo "Downloading Sparrow release artifact: $artifact"
    if ! download "$url" "$tmp"; then
        rm -f "$tmp"
        echo "Release artifact unavailable. Falling back to source build." >&2
        install_from_source
        return 0
    fi

    if [ "$VERIFY" -eq 1 ]; then
        verify_checksum "$tmp" "$artifact"
        rc=$?
        if [ "$rc" -eq 1 ]; then
            rm -f "$tmp"
            echo "" >&2
            echo "✗ SHA256 MISMATCH for $artifact — refusing to install a possibly tampered binary." >&2
            echo "  Aborting. Re-run with --from-source to build from source instead." >&2
            exit 1
        elif [ "$rc" -eq 2 ]; then
            rm -f "$tmp"
            echo "" >&2
            echo "⚠ No SHA256 checksum available for $artifact — cannot verify integrity." >&2
            echo "  Falling back to a source build (safer than an unverified binary)." >&2
            echo "  Pass --allow-unverified to install the binary anyway." >&2
            install_from_source
            return 0
        fi
        echo "✓ SHA256 verified."
    fi

    chmod +x "$tmp"
    mv "$tmp" "$BIN_PATH"
}

install_desktop_shortcut() {
    [ "$SHORTCUT" -eq 1 ] || return 0
    desktop_dir="${XDG_DESKTOP_DIR:-$HOME/Desktop}"
    if [ ! -d "$desktop_dir" ]; then
        return 0
    fi
    file="$desktop_dir/Sparrow.desktop"
    cat > "$file" <<EOF
[Desktop Entry]
Type=Application
Name=Sparrow
Comment=Open Sparrow
Exec=$BIN_PATH
Terminal=true
Categories=Development;Utility;
EOF
    chmod +x "$file" 2>/dev/null || true
    echo "Desktop shortcut created: $file"
}

echo "Installing Sparrow into $INSTALL_DIR"
if [ "$FROM_SOURCE" -eq 1 ]; then
    install_from_source
else
    install_from_release
fi

echo ""
echo "Sparrow installed: $BIN_PATH"
install_desktop_shortcut
if ! printf '%s' ":$PATH:" | grep -q ":$INSTALL_DIR:"; then
    echo ""
    echo "Add Sparrow to your PATH:"
    echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
fi

echo ""
echo "Next command:"
echo "  sparrow launch"

if [ "$LAUNCH" -eq 1 ]; then
    echo ""
    echo "Launching Sparrow WebView cockpit..."
    exec "$BIN_PATH" launch
fi
