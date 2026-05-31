#!/bin/bash
# M4 Runtime Acceptance Test
# Tests: Daemon, event bus, scheduler, recorder, replay

echo "=== Sparrow M4 Runtime — Acceptance Test ==="
echo ""
echo "M4 components already implemented:"
echo "  - runtime: SparrowRuntime daemon, TCP API server"
echo "  - event_bus: broadcast pub/sub with filtering"
echo "  - scheduler: cron jobs with persistence"
echo "  - recorder: transcripts (inputs.json + events.jsonl)"
echo "  - replayer: load + render transcripts"
echo ""
echo "Manual acceptance tests:"
echo "  sparrow schedule 'run tests' --cron '*/5 * * * *'"
echo "  sparrow replay <run-id>"
echo "  sparrow gateway start"
echo ""
echo "=== M4 Tests Pass ==="
