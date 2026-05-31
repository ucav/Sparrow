<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/brand/sparrow-mascot.svg">
  <img alt="Sparrow" src="assets/brand/sparrow-mascot.svg" width="140">
</picture>

# Sparrow

**The only CLI you install.**

> one cli · grows with you · pirate & builder

[![Status: Specification & Kernel](https://img.shields.io/badge/status-specification%20%2B%20kernel%20in%20progress-amber)](https://github.com/ucav/Sparrow)
[![License: MIT](https://img.shields.io/badge/license-MIT-green)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.96%2B-orange)](https://rust-lang.org)

---

Sparrow is a **self-contained, single-binary CLI** that fuses the best ideas of Claude Code, Codex, OpenCode, OpenClaw and Hermes Agent into one native Rust tool. One install. One binary. Any model. No other CLI required.

---

## What Sparrow Is

| Capability | In Sparrow |
|---|---|
| **One binary** | Static Rust binary — no Python, Node, or external CLIs |
| **Any model** | Anthropic, OpenAI, NVIDIA, Groq, Ollama, OpenRouter… 35 providers |
| **Local-first** | Full task offline via Ollama, `$0.00`, no account needed |
| **Agentic loop** | `think → act → observe` with tool use, context management |
| **Multi-agent swarm** | `Planner → Coder → Verifier` with adversarial review |
| **Autonomy dial** | Supervised → Trusted → Autonomous, continuous, not two modes |
| **Checkpoint & rewind** | Every mutating batch snapshotted; `rewind` restores instantly |
| **Persistent memory** | 4-tier memory (repo, identity, task, shared) with SQLite |
| **Self-improving** | Skills created from experience, curated automatically |
| **Replayable runs** | Every run recorded as `inputs.json` + `events.jsonl` |
| **Headless + scriptable** | `--json` NDJSON stream, exit codes, CI/hook friendly |
| **Multi-surface** | CLI · TUI · API · Telegram · Discord · Slack |

---

## Quick Start (planned)

```bash
curl -fsSL https://sparrow.dev/install.sh | sh
sparrow setup          # conversational onboarding
sparrow                # launch TUI
sparrow run "fix the failing auth test"
```

---

## Architecture

```
┌──────────────────────────────────────────────────────┐
│                  SPARROW RUNTIME                     │
│  config · auth · provider · tools · sandbox          │  Tier 0
│  router · engine · memory                            │  Tier 1
│  autonomy · agent · capabilities                     │  Tier 2
│  orchestrator · scheduler · runtime                  │  Tier 3
├──────────────────────────────────────────────────────┤
│  tui · cli · api · gateway (telegram/discord/slack)  │  Tier 4
└──────────────────────────────────────────────────────┘
```

**Everything is a configuration of one primitive: `AgentRun`.**

```
AgentRun = Identity + BrainPolicy + AutonomyContract + ToolSet + Memory + Workspace
```

Every surface (TUI, CLI, API, messaging) is a **thin renderer** over a single event stream. No business logic in surfaces. No provider lock-in.

---

## CLI (planned)

```bash
sparrow                                    # launch TUI
sparrow run "fix the failing auth test"    # one agentic run
sparrow run "summarize git log" --local    # offline via Ollama
sparrow swarm "add token revocation"       # planner → coder → verifier
sparrow schedule "run tests" --cron "0 2 * * *"
sparrow model --list                       # show active routing
sparrow replay <run-id>                    # replay from transcript
sparrow rewind <checkpoint-id>             # restore snapshot
sparrow --json run "task"                  # NDJSON for CI
```

---

## Safety Model

- **Autonomy is a dial**, never two modes
- Every **mutating action** requires an autonomy decision
- Every **mutating batch** creates a checkpoint
- **Destructive actions** are never silently allowed in Supervised
- **Secrets** are redacted from transcripts, logs, and context
- **Transcripts** are append-only and audit-friendly
- **Sandboxing** is core: local-hardened, Docker, SSH, remote

| Risk Level | Supervised | Trusted | Autonomous |
|---|---|---|---|
| ReadOnly | Allow | Allow | Allow |
| Mutating | Ask | Notify+Checkpoint | Allow+Checkpoint |
| Exec | Ask | Notify (sandbox) | Allow (sandbox) |
| Destructive | Deny | Ask | Ask |
| Network | Ask | Allow | Allow |

---

## Project Status

| Component | Status |
|---|---|
| Specification | ✅ Complete |
| Branding & Visual Identity | ✅ Complete |
| Rust Kernel (M0) | ✅ 54 tests, cargo build --release OK |
| CLI Grammar | ✅ 25+ subcommands |
| Provider Registry | ✅ 35 providers (Hermes Agent parity) |
| Native Ollama Adapter | ✅ /api/chat + NDJSON streaming |
| Routing Engine | ✅ Scoring + fallback + budget-aware |
| Autonomy Gate | ✅ 15-combination matrix tested |
| Checkpoint/Rewind | ✅ Git-based, rewind restores cleanly |
| Memory (SQLite 4-tier) | ✅ FTS5 + redaction + persistence |
| Agent SOUL Files | ✅ 5 agents (planner, coder, verifier, researcher, debugger) |
| TUI (ratatui) | ✅ Cockpit, scroll, ASCII mascot, autocomplete |
| WebView Console | ✅ HTTP + WebSocket + JS client + config panel |
| Swarm Orchestrator | ✅ Planner→Coder→Verifier + REWORK + file locks |
| Skills + Curator | ✅ 11 default skills + self-improving loop |
| Scheduler | ✅ Cron + persistence |
| Recorder/Replayer | ✅ Transcripts + golden replay |
| Gateway | ✅ 11 transports (Telegram, Discord, Slack, ...) |
| Reasoning Layer | ✅ Anti-simulation + hallucination guard + self-critique |
| Hooks System | ✅ 12 lifecycle events, blocking/non-blocking |
| Builder Tools | ✅ test, apply_patch, git PR, fetch_docs, LSP, REPL |
| Phase 1 (M0-M6) | ✅ Complete |
| Phase 2 (WS1-WS7) | ✅ Complete |
| Cross-compilation | ⬜ Linux musl, macOS, Windows (CI configured) |

---

## Roadmap

| Milestone | Contents | Status |
|---|---|---|
| **M0 Kernel** | config, auth, provider, tools, sandbox, router, engine, CLI, TUI | ✅ |
| **M1 Trust** | memory, agents, autonomy dial, checkpoints, rewind | ✅ |
| **M2 Swarm** | orchestrator, planner→coder→verifier, anti-collision | ✅ |
| **M3 Grows** | skills, Curator, MCP client | ✅ |
| **M4 Runtime** | daemon, event bus, scheduler, recorder, replayer | ✅ |
| **M5 Everywhere** | gateway (Telegram, Discord, Slack), WebSocket API | ✅ |
| **M6 Polish** | theming, self-update, install, full TUI, docs | ⬜ |

[Full roadmap →](ROADMAP.md)

---

## Branding

Sparrow's mascot is a **chubby pirate sparrow** — two-feather crest, thick eyebrow, open eye + pirate patch, coral beak, pink cheek, cream belly, key in wing.

[View branding assets →](assets/brand/)

```
        ^^
      .-~~~-.
     /__     \
    | o   ██  |
    |    v    |
    | .       |
     \ \__/  /
      '-..-'
      /|  |\  ╤━o
     '_|  |_'
```

---

## Contributing

Sparrow follows strict architecture rules:

- **Every module exposes a trait**, testable in isolation with mocks
- **No business logic in surfaces** — TUI/CLI are thin renderers
- **No provider lock-in** — switch model with `sparrow model`
- **No secrets in logs, transcripts, or context** — redact aggressively
- **Autonomy is a continuous dial**, never two modes

[Contributing guide →](CONTRIBUTING.md)

---

## License

MIT — see [LICENSE](LICENSE).

Built with Rust. Inspired by the best coding agents — locked to none.
