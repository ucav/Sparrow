# CLI Reference

All planned Sparrow commands.

## Core

| Command | Status | Description |
|---|---|---|
| `sparrow` | ✅ | Launch TUI (default) |
| `sparrow --tui` | ✅ | Launch terminal TUI explicitly |
| `sparrow --web` | ✅ | Launch webview console (HTTP + WebSocket) |
| `sparrow setup` | ✅ | Conversational onboarding |

## Run

| Command | Status | Description |
|---|---|---|
| `sparrow run "<task>"` | ✅ | One agentic run |
| `sparrow run "<task>" --local` | ✅ | Force local/Ollama only |
| `sparrow run "<task>" --model <id>` | ✅ | Force a specific model |
| `sparrow run "<task>" --budget <usd>` | ✅ | Session budget cap |
| `sparrow run "<task>" --autonomy <level>` | ✅ | Override autonomy |
| `sparrow run "<task>" --json` | ✅ | NDJSON event stream for CI |

## Swarm

| Command | Status | Description |
|---|---|---|
| `sparrow swarm "<task>"` | ✅ | Planner → Coder → Verifier pipeline |

## Schedule

| Command | Status | Description |
|---|---|---|
| `sparrow schedule "<task>" --cron "<expr>"` | ✅ | Schedule recurring job |
| `sparrow schedule ... --autonomy <level>` | ✅ | Autonomy for scheduled jobs |

## Model & Auth

| Command | Status | Description |
|---|---|---|
| `sparrow model --list` | ✅ | List configured providers/models |
| `sparrow model --set <route>` | ✅ | Override routing |
| `sparrow auth add <provider>` | ✅ | Add credentials |
| `sparrow auth list` | ✅ | List stored credentials |
| `sparrow auth rm <provider>` | ✅ | Remove credentials |

## Skills

| Command | Status | Description |
|---|---|---|
| `sparrow skills list` | ✅ | List skill library |
| `sparrow skills create <name>` | ✅ | Create a skill |
| `sparrow skills prune` | ✅ | Curator prune |

## MCP

| Command | Status | Description |
|---|---|---|
| `sparrow mcp add <server>` | ✅ | Add MCP connector |
| `sparrow mcp list` | ✅ | List MCP servers |
| `sparrow mcp rm <server>` | ✅ | Remove MCP connector |

## Checkpoint & Replay

| Command | Status | Description |
|---|---|---|
| `sparrow checkpoint list` | ✅ | List checkpoints |
| `sparrow rewind [<id>\|<n>]` | ✅ | Restore checkpoint |
| `sparrow replay <run-id>` | ✅ | Re-render transcript |

## Gateway

| Command | Status | Description |
|---|---|---|
| `sparrow gateway start` | ✅ | Start messaging daemon |
| `sparrow gateway status` | ✅ | Daemon status |
| `sparrow gateway stop` | ✅ | Stop daemon |

## Profile & Import

| Command | Status | Description |
|---|---|---|
| `sparrow profile create <name>` | ✅ | Create isolated profile |
| `sparrow profile list` | ✅ | List profiles |
| `sparrow profile use <name>` | ✅ | Switch profile |
| `sparrow import openclaw [path]` | ✅ | Migrate from OpenClaw |

## Maintenance

| Command | Status | Description |
|---|---|---|
| `sparrow config --edit` | ✅ | Open config.toml in editor |
| `sparrow update` | ✅ | Self-update |
| `sparrow doctor` | ✅ | Diagnostics |
