# Migrating from Hermes Agent to Sparrow

## Quick Import

```bash
sparrow import hermes [path]   # default: ~/.hermes
```

## What Gets Imported

| Hermes | Sparrow Equivalent |
|---|---|
| Agents (SOUL files) | `agents/*.soul.toml` |
| Skills | `skills/` |
| `hermes.yaml` config | `config.toml` |
| Cron jobs | Scheduler (SQLite) |
| Gateway config | `config.toml` [surfaces] |

## What Improves

- **Rust binary** vs Python: 6MB, no dependencies, single file install
- **Swarm review**: Planner → Coder → Verifier with REWORK loop
- **Autonomy dial**: Continuous trust contract, not just daemon mode
- **Enterprise features**: OrgPolicy, audit logs, SSO, air-gapped
- **Checkpoint/rollback**: Git-based, every mutation reversible
- **35 providers**: Including all Hermes providers + merged additions
