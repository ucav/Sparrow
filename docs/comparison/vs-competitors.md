# Sparrow vs Competitors — Honest Comparison

Sparrow is designed to replace five tools. Here's an honest, factual comparison.

## Sparrow vs Claude Code

| Capability | Claude Code | Sparrow |
|---|---|---|
| Agentic coding | ✅ | ✅ |
| Tool use (files, git, exec) | ✅ | ✅ |
| MCP servers | ✅ | ✅ |
| Hooks / custom commands | ✅ | ✅ (WS2) |
| IDE integration (VS Code) | ✅ | ⚠️ Extension manifest ready, needs packaging |
| Model freedom | ❌ Anthropic only | ✅ 35 providers |
| Local/free models | ❌ | ✅ Ollama native |
| Autonomous mode | ❌ Interactive only | ✅ Continuous dial |
| Checkpoint/rollback | ❌ | ✅ Git-based |
| Multi-agent swarm | ❌ | ✅ Planner→Coder→Verifier |
| Self-improving skills | ❌ | ✅ Curator loop |
| Multi-surface (Telegram/Discord) | ❌ | ✅ 11 transports |
| Pricing | Anthropic API costs | Free (Ollama) + any provider |

**Verdict:** Sparrow replaces Claude Code if you want model freedom, local execution, autonomy, and swarm review. Claude Code wins on IDE depth (today).

## Sparrow vs Codex

| Capability | Codex (OpenAI) | Sparrow |
|---|---|---|
| Agentic coding | ✅ | ✅ |
| Cloud execution | ✅ (Codex cloud) | ⚠️ Configurable (modal/daytona/ssh) |
| GitHub integration (issue→PR) | ✅ | ⚠️ Git PR tool built, GitHub App planned |
| Model freedom | ❌ OpenAI only | ✅ 35 providers |
| Local execution | ❌ Cloud only | ✅ Full local |
| Cost control | ❌ Unpredictable | ✅ Budget caps per session/day |
| Swarm review | ❌ | ✅ Adversarial Verifier |
| Memory (grows with you) | ❌ | ✅ SQLite 4-tier |

**Verdict:** Sparrow replaces Codex if you need multi-model, cost control, and offline work. Codex wins on cloud/GitHub seamlessness (today).

## Sparrow vs OpenCode

| Capability | OpenCode | Sparrow |
|---|---|---|
| Agentic coding | ✅ | ✅ |
| Provider breadth | ✅ OpenRouter etc. | ✅ 35 providers |
| Theming | ✅ | ✅ Captain theme + color tokens |
| TUI | ✅ | ✅ ratatui cockpit |
| Swarm/orchestration | ❌ | ✅ Planner→Coder→Verifier |
| Autonomy dial | ❌ | ✅ Continuous |
| Self-improving | ❌ | ✅ Skills + Curator |
| Gateway/messaging | ❌ | ✅ 11 transports |
| Enterprise (RBAC, audit) | ❌ | ✅ OrgPolicy |

**Verdict:** Sparrow replaces OpenCode if you need orchestration, autonomy safety, and team features. OpenCode wins on ecosystem maturity.

## Sparrow vs OpenClaw

| Capability | OpenClaw | Sparrow |
|---|---|---|
| Multi-agent | ✅ | ✅ + swarm review |
| Cron/scheduling | ✅ | ✅ |
| Telegram/Discord/Slack | ✅ | ✅ 11 transports |
| Daemon mode | ✅ | ✅ |
| Migration path | — | ✅ `sparrow import openclaw` |
| Rust (single binary) | ❌ Python | ✅ 6MB static binary |
| Checkpoint/rollback | ❌ | ✅ |
| Self-improving | ❌ | ✅ |
| Model freedom | Partial | ✅ 35 providers |

**Verdict:** Sparrow replaces OpenClaw with a native Rust binary, rollback safety, and self-improvement. Migration is one command.

## Sparrow vs Hermes Agent

| Capability | Hermes Agent | Sparrow |
|---|---|---|
| Agentic coding | ✅ | ✅ |
| Grows with you | ✅ | ✅ |
| Memory & facts | ✅ | ✅ |
| Skills system | ✅ | ✅ + Curator |
| Gateway (11 transports) | ✅ | ✅ |
| Terminal TUI | ✅ | ✅ |
| Rust (single binary) | ❌ Python | ✅ 6MB static |
| Swarm/adversarial review | ❌ | ✅ |
| Autonomy dial | ❌ | ✅ |
| Enterprise (RBAC, audit) | ❌ | ✅ |

**Verdict:** Sparrow replaces Hermes with a native binary, swarm review, and enterprise features. Migration preserves all Hermes data.
