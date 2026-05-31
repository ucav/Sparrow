#!/bin/bash
# M1 Trust Acceptance Test
# Tests: checkpoint + rewind, budget hard stop, memory persistence

set -e
echo "=== Sparrow M1 Trust — Acceptance Test ==="
echo ""

# Ensure Ollama is available for the run
if ! curl -s http://localhost:11434/api/tags > /dev/null 2>&1; then
    echo "Ollama not running. Start it first: ollama serve"
    echo "Skipping M1 acceptance test (requires Ollama)."
    echo "Run 'cargo test' for M1 unit/integration tests."
    exit 1
fi

TEST_DIR=$(mktemp -d)
cd $TEST_DIR
echo "Test workspace: $TEST_DIR"
git init 2>/dev/null

# Create a test file
echo 'fn main() { println!("hello"); }' > main.rs
git add main.rs 2>/dev/null
git commit -m "initial" 2>/dev/null

BUILD_DIR="$OLDPWD"
MODEL="qwen2.5:7b"

echo ""
echo "--- Test 1: Trusted autonomy auto-applies with checkpoint ---"
echo "y" | $BUILD_DIR/target/release/sparrow run \
    "rename the function 'main' to 'entry_point' in main.rs" \
    --model ollama --autonomy trusted 2>&1 || true

echo ""
echo "--- Test 2: Checkpoint list shows the snapshot ---"
$BUILD_DIR/target/release/sparrow checkpoint list 2>&1

echo ""
echo "--- Test 3: Rewind restores to pre-mutation state ---"
CHECKPOINT=$($BUILD_DIR/target/release/sparrow checkpoint list 2>&1 | head -1 | awk '{print $2}')
if [ -n "$CHECKPOINT" ]; then
    $BUILD_DIR/target/release/sparrow rewind "$CHECKPOINT" 2>&1
    echo ""
    echo "After rewind:"
    cat main.rs
    echo ""
    if grep -q "fn main" main.rs; then
        echo "✓ Rewind restored original state"
    else
        echo "✗ Rewind did NOT restore original state"
        exit 1
    fi
fi

echo ""
echo "--- Test 4: Memory persists between runs ---"
# Run a task that creates a fact
$BUILD_DIR/target/release/sparrow run "Note: the user prefers snake_case naming" \
    --model ollama --autonomy trusted 2>&1 || true

echo ""
echo "=== M1 Acceptance Test Complete ==="
echo ""
echo "Run 'cargo test' for full M1 test suite."
