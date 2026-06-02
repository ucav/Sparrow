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
| `sparrow plan "<task>"` | Alpha | Produce a read-only plan without tools, edits, exec, or checkpoints |
| `sparrow plan "<task>" --json` | Alpha | Emit the read-only plan as JSON |

## Permissions

| Command | Status | Description |
|---|---|---|
| `sparrow permissions list` | Alpha | Show permission mode plus tool/path/provider/surface allow, ask, and deny rules |
| `sparrow permissions set <mode>` | Alpha | Set `read-only`, `plan`, `supervised`, `trusted`, `autonomous`, or `emergency-stop` |
| `sparrow permissions allow-tool <tool>` | Alpha | Add an explicit tool allow pattern |
| `sparrow permissions ask-tool <tool>` | Alpha | Add a tool pattern that always asks for approval |
| `sparrow permissions deny-tool <tool>` | Alpha | Add an explicit tool deny pattern |
| `sparrow permissions allow-path <path>` | Alpha | Add an allowed path boundary |
| `sparrow permissions deny-path <path>` | Alpha | Add a denied path boundary |

## Memory

| Command | Status | Description |
|---|---|---|
| `sparrow memory list` | Alpha | Show bounded doc usage and stored facts |
| `sparrow memory add <key> <value>` | Alpha | Add a durable fact after redaction/injection checks |
| `sparrow memory replace <id> <key> <value>` | Alpha | Replace an existing fact id |
| `sparrow memory forget <id>` | Alpha | Remove a durable fact |
| `sparrow memory recall "<query>" --limit <n>` | Alpha | Search durable facts |
| `sparrow memory consolidate` | Alpha | Distill facts into bounded `MEMORY.md` / `USER.md` docs |
| `sparrow memory docs` | Alpha | Print stored bounded memory docs |
| `sparrow memory search "<query>" --limit <n>` | Alpha | Search saved session turns |
| `sparrow memory scroll <session> --around <n>` | Alpha | Show neighboring session turns around a hit |

## Slash Commands

| Command | Status | Description |
|---|---|---|
| `/help` | Alpha | List available built-in, project, user, and skill commands |
| `/plan <task>` | Alpha | Produce a read-only plan before accepting execution |
| `/permissions` | Alpha | Open the permission workflow; CLI and WebView mode controls are wired |
| `/memory` | Alpha | Memory workflow entrypoint; CLI/WebView memory APIs are wired |
| `/compact` | Alpha | Context compaction workflow placeholder command |
| `/model` | Alpha | Model routing workflow placeholder command |
| `/agents` | Alpha | Agent workflow placeholder command |
| `/sessions` | Alpha | Session workflow placeholder command |
| `/export` | Alpha | Export workflow placeholder command |

Project commands live in `.sparrow/commands/*.md`; user commands live in the
platform config directory under `commands/*.md` and override project/built-in
commands by name. Skills are exposed as slash commands for reusable workflows.

## Swarm

| Command | Status | Description |
|---|---|---|
| `sparrow swarm "<task>"` | ✅ | Planner → Coder → Verifier pipeline |

## Agents

| Command | Status | Description |
|---|---|---|
| `sparrow agent create <name>` | ✅ | Create a persistent SOUL TOML agent |
| `sparrow agent list` | ✅ | List configured agents and visible tool constraints |
| `sparrow agent edit <name>` | ✅ | Open an agent definition in the editor |
| `sparrow agent rm <name>` | ✅ | Remove an agent definition |
| `sparrow agent run <name> "<task>"` | Alpha | Run a task as a named agent with identity/model/permission hints |
| `sparrow agent mention <name> "<message>"` | Alpha | Send an `@agent` style message to a named agent |

Agents can be declared as existing `*.soul.toml` files or as `*.agent.md`
files with frontmatter fields such as `name`, `description`, `role`, `prompt`,
`tools`, `disallowed_tools`, `model`, `permission_mode`, `mcp_servers`,
`max_turns`, `memory`, `background`, `isolation`, and `color`.

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
| `sparrow skills view <name>` | Alpha | Invoke a skill and load declared references on demand |
| `sparrow skills create <name>` | ✅ | Create a skill |
| `sparrow skills install <local-dir-or-git-url>` | Alpha | Install a skill from a local directory/file or GitHub repo |
| `sparrow skills update <name>` | Alpha | Refresh a skill from the local library |
| `sparrow skills prune` | ✅ | Curator prune |
| `sparrow skills rm <name>` | ✅ | Remove a skill |

## Plugins

| Command | Status | Description |
|---|---|---|
| `sparrow plugins list` | Alpha | List installed local plugins and scanner warnings |
| `sparrow plugins install <local-dir-or-git-url>` | Alpha | Install a plugin manifest from local disk or GitHub |
| `sparrow plugins rm <name>` | Alpha | Remove an installed plugin |

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
