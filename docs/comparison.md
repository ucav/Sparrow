# Sparrow vs Claude Code vs Hermes vs OpenClaw

Honest, factual comparison of what Sparrow v0.2.0 delivers today against the
three reference points that shaped its design.

For the longer per-tool tables (Codex, OpenCode, …), see
[`comparison/vs-competitors.md`](comparison/vs-competitors.md).

## Sparrow vs Claude Code

| Capability | Claude Code | Sparrow v0.2.0 |
|---|---|---|
| Plan mode (read-only before action) | ✅ | ✅ `sparrow plan` + `/plan` |
| Slash commands (built-in + user + project) | ✅ | ✅ `.sparrow/commands/*.md` + skills |
| Permission modes | ✅ | ✅ read-only / plan / supervised / trusted / autonomous / emergency-stop |
| Hooks (PreToolUse, PostToolUse, PreCompact, …) | ✅ | ✅ 14 hook events |
| Subagents (`.agent.md` frontmatter) | ✅ | ✅ + `@<name>` picker in the TUI |
| Skills with progressive disclosure | ✅ | ✅ references + templates + scripts + assets |
| Plugins | ✅ | ✅ `.sparrow-plugin/plugin.toml`, scanner, namespace |
| Toolsets per surface | ✅ | ✅ safe / web / file / terminal / media / debug / skills / memory / session_search / mcp / gateway |
| MCP servers | ✅ | ✅ `sparrow mcp add/list/...` |
| Memory (CLAUDE.md / USER.md style) | ✅ | ✅ bounded `MEMORY.md` + `USER.md` + SQLite facts + FTS5 session search |
| Context meter + auto-compaction | ✅ | ✅ engine auto-trigger at 120k chars, `HandoffDoc` Markdown |
| GitHub Action | ✅ | ✅ composite `action.yml` + `sparrow github review/status/logs` |
| Model freedom | ❌ Anthropic only | ✅ 35-provider registry, Ollama-first |
| Self-improving skills (curator loop) | ❌ | ✅ |
| Multi-agent swarm (Planner → Coder → Verifier) | ❌ | ✅ |
| Gateway (Telegram / Discord / Slack / Email) | ❌ | ✅ |
| Cost control (budget caps) | ❌ | ✅ daily + session caps |

**Where Sparrow wins:** model freedom, local execution, swarm review, gateway
surfaces, budget caps.
**Where Claude Code still wins (today):** IDE depth (VS Code), polished
managed-agent experience.

## Sparrow vs Hermes Agent

| Capability | Hermes Agent | Sparrow v0.2.0 |
|---|---|---|
| Bounded memory docs (~2k / ~1.4k chars) | ✅ | ✅ `MEMORY_MD_LIMIT = 2200`, `USER_MD_LIMIT = 1375` |
| Anti-injection memory guards | ✅ | ✅ |
| Session FTS search & scroll | ✅ | ✅ SQLite FTS5 |
| External memory providers | ✅ mem0 / honcho / supermemory | ⚠ Local provider stable; mem0/honcho/supermemory are honest "not configured" stubs |
| Skills system | ✅ | ✅ + curator |
| Gateway (multi-surface) | ✅ | ✅ |
| Terminal TUI | ✅ | ✅ ratatui cockpit |
| Rust (single binary) | ❌ Python | ✅ ~6 MB static |
| Swarm / adversarial review | ❌ | ✅ |
| Autonomy dial (6 modes) | ❌ | ✅ |
| Checkpoint / rollback | ❌ | ✅ Git-backed |
| Enterprise (RBAC, audit) | ❌ | ✅ `OrgPolicy` |

**Where Sparrow wins:** native binary, swarm review, checkpoint+rollback,
enterprise primitives.
**Where Hermes still wins:** richer ecosystem of external memory providers
out of the box (Sparrow ships an honest stub instead of a fake integration).

## Sparrow vs OpenClaw

| Capability | OpenClaw | Sparrow v0.2.0 |
|---|---|---|
| Multi-agent | ✅ | ✅ + swarm review |
| Cron / scheduling | ✅ | ✅ `sparrow schedule` |
| Telegram / Discord / Slack | ✅ | ✅ + Email + WhatsApp/Signal/Teams/Feishu/WeCom/QQ stubs |
| Daemon mode | ✅ | ✅ `sparrow gateway start` |
| Scoped gateway sessions | ⚠ | ✅ `gateway:<surface>:channel:<chat>:peer:<user>` |
| Gateway abort (cancels active run) | ⚠ | ✅ `RunRegistry` + abort poller |
| Migration path | — | ✅ `sparrow import openclaw` |
| Rust (single binary) | ❌ Python | ✅ |
| Checkpoint / rollback | ❌ | ✅ |
| Self-improving | ❌ | ✅ |
| Model freedom | Partial | ✅ 35 providers |

**Where Sparrow wins:** native Rust, real abort, checkpoint safety,
self-improvement, one-command migration from OpenClaw config.

## Verdict for v0.2.0

Sparrow now matches Claude Code/Hermes/OpenClaw on the **mechanics** (plan,
permissions, hooks, subagents, skills, memory, gateway, compaction, GitHub
Action). What is still honestly Alpha:

- **Provider routing** depends on the user's tokens — covered by tests but
  the end-to-end experience is only as good as the configured providers.
- **First-run setup** has a conversational agent and a fallback flow but UX
  iteration continues.
- **External memory providers** (`mem0`, `honcho`, `supermemory`) are
  scaffolded but not wired to real services — they return clear
  "not configured" errors rather than fake successes.
- **Cross-platform release** workflow exists but no signed public artifact
  has been published yet.

Everything else has been moved from Alpha → Stable in the README status table
because it has unit + integration test coverage.
