# Migrating from OpenClaw to Sparrow

## Quick Import

```bash
sparrow import openclaw [path]   # default: ~/.openclaw
```

## What Gets Imported

| OpenClaw | Sparrow Equivalent |
|---|---|
| `agents/` (SOUL.md) | `agents/*.soul.toml` |
| `skills/` | `skills/` |
| `cron.json` | Scheduler jobs (SQLite) |
| API keys | Auth store (keychain → encrypted → env) |
| Messaging config | `config.toml` [surfaces] |

## What Improves

- **Rust binary** vs Python: 6MB static, no venv, no pip
- **Checkpoint/rollback**: Every mutation reversible
- **Self-improving**: Curator grades and improves skills automatically
- **Adversarial review**: Verifier blocks bad diffs before they land
- **Full CLI grammar**: 25+ subcommands
