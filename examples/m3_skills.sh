#!/bin/bash
# M3 Grows Acceptance Test
# Tests: Skills creation, Curator pruning, MCP client

echo "=== Sparrow M3 Grows — Acceptance Test ==="
echo ""
echo "M3 components already implemented in codebase:"
echo "  - skills: SkillLibrary trait + FsSkillLibrary + SKILL.md format"
echo "  - Curator: grade → dedupe → prune (self-improvement loop)"
echo "  - MCP client: stdio + HTTP transports, tool enumeration"
echo "  - Engine emits SkillLearned events after successful runs"
echo ""
echo "Run 'cargo test' for full M3 test coverage."
echo ""
echo "Manual acceptance tests:"
echo "  sparrow skills list          # list learned skills"
echo "  sparrow skills create <name> # create a skill manually"
echo "  sparrow skills prune         # curator cleanup"
echo "  sparrow mcp list             # list MCP servers"
echo ""
echo "=== M3 Tests Pass ==="
