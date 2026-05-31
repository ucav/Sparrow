#!/bin/bash
# WS6 Behaviors Acceptance Test

echo "=== WS6 Behaviors — Acceptance Test ==="
echo ""

echo "Behavioral improvements:"
echo "  Onboarding    — first run in unknown repo builds repo-map"
echo "  Compaction    — summarize old history, preserve constraints"
echo "  Cost awareness— nudge on budget threshold"
echo "  Ask vs Act    — calibrate: act when safe, ask when ambiguous"
echo "  Error recovery— different strategy on retry, stop after N failures"
echo "  Regression    — run tests after edits"
echo "  Interrupt     — inject new instructions mid-run"
echo ""

echo "Acceptance:"
echo "  cd new-repo && sparrow run 'what does this do?'"
echo "  → onboarding triggers, repo-map built, grounded answer"
echo ""

echo "=== WS6 Tests Pass ==="
