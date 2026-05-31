# Migrating from OpenCode to Sparrow

## Quick Import

```bash
sparrow import opencode [path]   # default: ~/.config/opencode
```

## What Gets Imported

| OpenCode | Sparrow Equivalent |
|---|---|
| `opencode.json` config | `config.toml` (providers, routing, theme) |
| Provider settings | Provider registry (35 providers) |
| Theme | `theme = "captain"` (color tokens from spec §9.2) |

## What You Gain

- **Swarm orchestration**: Planner → Coder → Verifier with adversarial review
- **Autonomy dial**: Continuous trust, not binary
- **Checkpoint/rollback**: Every mutation is reversible
- **Self-improving skills**: Curator loop learns from your sessions
- **Multi-surface**: Telegram, Discord, Slack, CLI — same session
- **Enterprise**: RBAC, audit logs, SSO, air-gapped mode
