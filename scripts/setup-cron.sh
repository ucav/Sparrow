#!/usr/bin/env bash
# Sparrow cron jobs setup — run once to activate background tasks
# Usage: bash scripts/setup-cron.sh

SPARROW="${SPARROW_BIN:-sparrow}"

echo "🔧 Setting up Sparrow cron jobs..."

# 1. Daily health check (every day at 9am)
$SPARROW schedule "check system health: disk space, memory, CPU, running services" \
  --cron "0 9 * * *" 2>/dev/null && echo "  ✓ Health check (daily 9am)" || echo "  ⚠ Health check skipped"

# 2. Git auto-commit (every 4 hours — saves work in progress)
$SPARROW schedule "review current git status and auto-commit WIP changes with descriptive message" \
  --cron "0 */4 * * *" 2>/dev/null && echo "  ✓ Auto-commit (every 4h)" || echo "  ⚠ Auto-commit skipped"

# 3. Dependency audit (every Monday)
$SPARROW schedule "run cargo audit and report any vulnerable dependencies. Suggest fixes" \
  --cron "0 8 * * 1" 2>/dev/null && echo "  ✓ Security audit (weekly Mon)" || echo "  ⚠ Security audit skipped"

# 4. Session cleanup (daily at 3am — remove sessions older than 30 days)
$SPARROW schedule "clean up old sessions and transcripts older than 30 days. Keep at least 10 most recent." \
  --cron "0 3 * * *" 2>/dev/null && echo "  ✓ Session cleanup (daily 3am)" || echo "  ⚠ Cleanup skipped"

# 5. Knowledge distillation (every 6 hours — compress recent learnings into memory)
$SPARROW schedule "review recent session transcripts, extract key facts and lessons, store in memory" \
  --cron "0 */6 * * *" 2>/dev/null && echo "  ✓ Knowledge distillation (every 6h)" || echo "  ⚠ Distillation skipped"

echo ""
echo "✅ Cron jobs configured."
echo "   List:  $SPARROW schedule --list"
echo "   Logs:  journalctl --user -u sparrow-gateway -f"
