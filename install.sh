#!/bin/sh
# Sparrow install script
# curl -fsSL https://sparrow.dev/install.sh | sh
set -e

BIN_DIR="${HOME}/.local/bin"
BIN_PATH="${BIN_DIR}/sparrow"
CONFIG_DIR="${HOME}/.config/sparrow"
STATE_DIR="${HOME}/.local/state/sparrow"

# Detect platform
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
    linux)
        case "$ARCH" in
            x86_64)  PLATFORM="linux-x86_64" ;;
            aarch64) PLATFORM="linux-aarch64" ;;
            *)       echo "Unsupported architecture: $ARCH"; exit 1 ;;
        esac
        ;;
    darwin)
        case "$ARCH" in
            x86_64) PLATFORM="macos-x86_64" ;;
            arm64)  PLATFORM="macos-arm64" ;;
            *)      echo "Unsupported architecture: $ARCH"; exit 1 ;;
        esac
        ;;
    *)
        echo "Unsupported OS: $OS"
        echo "For Windows, download from: https://github.com/sparrow-dev/sparrow/releases/latest"
        exit 1
        ;;
esac

# Get latest version
LATEST=$(curl -s https://api.github.com/repos/sparrow-dev/sparrow/releases/latest | grep '"tag_name"' | sed 's/.*"tag_name": "\(.*\)".*/\1/')
if [ -z "$LATEST" ]; then
    echo "Cannot determine latest version. Using v0.1.0"
    LATEST="v0.1.0"
fi

DOWNLOAD_URL="https://github.com/sparrow-dev/sparrow/releases/download/${LATEST}/sparrow-${PLATFORM}"

echo "Installing Sparrow ${LATEST} for ${PLATFORM}..."
echo ""

# Create directories
mkdir -p "$BIN_DIR"
mkdir -p "$CONFIG_DIR"
mkdir -p "$STATE_DIR"

# Download binary
echo "Downloading..."
curl -fsSL "$DOWNLOAD_URL" -o "$BIN_PATH.tmp"
chmod +x "$BIN_PATH.tmp"
mv "$BIN_PATH.tmp" "$BIN_PATH"

echo "Sparrow installed to: $BIN_PATH"
echo ""

# Add to PATH if needed
if ! echo "$PATH" | grep -q "$BIN_DIR"; then
    echo "Add to your shell config:"
    echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
    echo ""
fi

# First-run setup
echo "Run 'sparrow' to launch the TUI or 'sparrow setup' for guided configuration."
echo "Documentation: https://sparrow.dev"
echo ""
echo "one cli · grows with you"
