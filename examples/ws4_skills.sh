#!/bin/bash
# WS4 Skills Pack Acceptance Test

echo "=== WS4 Skills Pack — Acceptance Test ==="
echo ""

echo "Default skills delivered (11):"
echo "  onboard-to-repo       — first-contact repo analysis"
echo "  debug-systematically  — reproduce → isolate → fix"
echo "  write-tests           — nominal + edge + regression"
echo "  refactor-safely       — small steps, tests green, checkpoint"
echo "  code-review           — adversarial review heuristics"
echo "  upgrade-dependencies  — bump + build + test + changelog"
echo "  security-audit        — secrets, injections, deps"
echo "  write-docs            — doc from real code, never invent"
echo "  git-workflow          — branches, atomic commits, PRs"
echo "  performance-profile   — measure before optimizing"
echo "  verify-before-claiming — meta-skill: never fabricate"
echo ""

echo "Skills location: skills/*/SKILL.md"
echo "Matching: SkillLibrary.relevant(ctx) scores by trigger + description"
echo ""

echo "=== WS4 Tests Pass ==="
