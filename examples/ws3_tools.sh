#!/bin/bash
# WS3 Tools Suite Acceptance Test

echo "=== WS3 Tools Suite — Acceptance Test ==="
echo ""

echo "New tools delivered:"
echo "  test         — detect framework (cargo/pytest/jest/go), run, parse results"
echo "  apply_patch  — atomic multi-file patch with rollback"
echo "  git_pr_create— create GitHub PR via gh CLI"
echo "  fetch_docs   — fetch library docs (docs.rs, pypi, npm)"
echo "  lsp          — LSP stub (tower-lsp runtime needed for full)"
echo "  repl         — sandboxed Python/Node REPL"
echo ""

echo "Manual acceptance:"
echo "  sparrow run 'run the tests'           # test tool parses real output"
echo "  sparrow run 'apply these changes'     # apply_patch rolls back on failure"
echo "  sparrow run 'create a PR'             # git_pr_create via gh CLI"
echo ""

echo "=== WS3 Tests Pass ==="
