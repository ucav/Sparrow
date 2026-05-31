# Migrating from Codex to Sparrow

## Quick Import

```bash
sparrow import codex [path]   # default: ~/.codex
```

## What Gets Imported

| Codex | Sparrow Equivalent |
|---|---|
| `AGENTS.md` | SOUL file (agent definition) |
| `codex.yaml` config | `config.toml` routing/autonomy |
| Cloud execution | `sparrow cloud run` (modal/daytona/ssh backends) |
| GitHub integration | `git_pr_create` tool + GitHub App (WS11) |

## Key Differences

- **Multi-model**: Codex is OpenAI-only. Sparrow supports 35 providers.
- **Offline**: Sparrow runs fully offline via Ollama.
- **Cost control**: Budget caps per session/day. No unpredictable cloud costs.
- **Swarm review**: Planner → Coder → Verifier catches bugs before they land.
- **Self-improving**: Sparrow learns skills from experience.
