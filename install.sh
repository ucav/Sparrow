#!/bin/sh
# Sparrow one-click installer for Linux and macOS.
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/ucav/Sparrow/master/install.sh | sh
#   curl -fsSL https://raw.githubusercontent.com/ucav/Sparrow/master/install.sh | sh -s -- --no-launch
#   curl -fsSL https://raw.githubusercontent.com/ucav/Sparrow/master/install.sh | sh -s -- --from-source
set -eu

REPO="ucav/Sparrow"
INSTALL_DIR="${SPARROW_INSTALL_DIR:-$HOME/.local/bin}"
BIN_PATH="$INSTALL_DIR/sparrow"
LAUNCH=1
FROM_SOURCE=0

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
        --no-launch)
            LAUNCH=0
            ;;
        -h|--help)
            echo "Usage: install.sh [--dir PATH] [--from-source] [--no-launch]"
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

install_from_release() {
    artifact="$(detect_artifact)"
    url="https://github.com/$REPO/releases/latest/download/$artifact"
    mkdir -p "$INSTALL_DIR"
    tmp="$BIN_PATH.tmp"

    echo "Downloading Sparrow release artifact: $artifact"
    if download "$url" "$tmp"; then
        chmod +x "$tmp"
        mv "$tmp" "$BIN_PATH"
        return 0
    fi

    rm -f "$tmp"
    echo "Release artifact unavailable. Falling back to source build." >&2
    install_from_source
}

echo "Installing Sparrow into $INSTALL_DIR"
if [ "$FROM_SOURCE" -eq 1 ]; then
    install_from_source
else
    install_from_release
fi

echo "Sparrow installed: $BIN_PATH"
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
