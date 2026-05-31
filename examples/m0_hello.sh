#!/bin/bash
# M0 Acceptance Test: sparrow run with real Ollama
# Requires: Ollama running, qwen2.5:7b pulled (or any model)

set -e

echo "=== Sparrow M0 Kernel — Acceptance Test ==="
echo ""

# Check Ollama
if ! curl -s http://localhost:11434/api/tags > /dev/null 2>&1; then
    echo "Ollama not running. Start it first: ollama serve"
    exit 1
fi

MODEL="qwen2.5:7b"
# Pull model if needed
ollama pull $MODEL 2>/dev/null || true

echo "Building Sparrow..."
cargo build --release
echo ""

echo "Test: sparrow run 'add a hello() function to src/lib.rs'"
echo ""

# Create a test workspace
TEST_DIR=$(mktemp -d)
cd $TEST_DIR
echo '// test lib' > lib.rs
git init 2>/dev/null
git add lib.rs 2>/dev/null
git commit -m "init" 2>/dev/null

echo "Running Sparrow..."
echo "y" | $OLDPWD/target/release/sparrow run "add a hello() function to lib.rs that prints 'hello world'" --model ollama 2>&1

echo ""
echo "Result:"
cat lib.rs
echo ""
echo "=== M0 Acceptance Test Complete ==="
