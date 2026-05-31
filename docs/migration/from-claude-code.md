# Migrating from Claude Code to Sparrow

## Quick Import

```bash
sparrow import claude-code [path]   # default: ~/.claude
```

## What Gets Imported

| Claude Code | Sparrow Equivalent |
|---|---|
| `CLAUDE.md` | SOUL file (`agents/claude-code-import.soul.toml`) |
| `.claude/settings.json` | `config.toml` routing/autonomy settings |
| `.mcp.json` MCP servers | `~/.config/sparrow/mcp/mcp_servers.json` |
| Slash commands | Hooks (`~/.config/sparrow/hooks.toml`) |
| `/compact` | ContextManager auto-compaction |

## What Changes

- **Model freedom**: You're no longer locked to Anthropic. Add any provider via `sparrow auth add`.
- **Autonomy dial**: Continuous, not binary. Supervised → Trusted → Autonomous.
- **Checkpoints**: Every mutating batch is snapshotted. `sparrow rewind` to undo.
- **Swarm**: `sparrow swarm` runs Planner → Coder → Verifier with adversarial review.
- **Multi-surface**: Continue sessions from Telegram, Discord, Slack.
- **Local**: `sparrow run --local` works offline via Ollama, $0.00.

## What Stays the Same

- Tool use (files, git, exec, search)
- MCP servers
- Hooks/custom commands
- VS Code extension (thin client over WebSocket API)
