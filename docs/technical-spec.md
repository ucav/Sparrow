# Sparrow — Technical Specification v1

> **This document is the canonical v1 architecture reference for Sparrow.**

**Purpose.** This is the authoritative build spec for the models implementing Sparrow. It defines a complete v1, not an MVP.

**One-line pitch.** Sparrow is a single self-contained CLI that fuses the best of Claude Code, Codex, OpenCode, OpenClaw and Hermes Agent into one tool. You install one binary, add provider keys, and get an agentic coding system with model-freedom, a multi-agent swarm with built-in review, graduated autonomy with instant rollback, a self-improving skill loop, persistent memory, and presence across CLI + messaging surfaces.

**North star.** Sell **trust**, not power. Engagement comes from craft + flow + legibility, never retention hooks.

---

## 0. Hard Rules for Implementers

1. **One binary, zero external CLIs.** Sparrow does NOT shell out to claude-code/codex/opencode. It re-implements their logic natively. The only external dependencies are model-provider HTTP endpoints and MCP servers.
2. **Every module exposes a trait, not a concrete type.** Modules are testable in isolation against their trait with mocks.
3. **The runtime is headless.** All surfaces (TUI, CLI, API, messaging) are thin renderers over one event stream. No business logic in surfaces.
4. **Autonomy is a continuous dial, never two modes.** If the implementation collapses to "interactive vs daemon", it is wrong.
5. **Nothing mutating runs without an autonomy decision and a checkpoint.** Reproducibility and rollback are first-class, not afterthoughts.
6. **Secrets never enter transcripts, logs, or model context** unless explicitly a tool argument. Redact aggressively.

---

## 1. Tech Stack & Global Decisions

| Concern | Decision | Rationale |
|---|---|---|
| Language | **Rust** (edition 2021+) | Single static binary, no runtime, sandbox primitives, perf |
| Async | `tokio` | Streaming, concurrency for swarm |
| TUI | `ratatui` + `crossterm` | Reference Rust TUI |
| Serialization | `serde` (+ `serde_json`, `toml`) | Config, events, transcripts |
| HTTP | `reqwest` (rustls) | Provider + MCP calls |
| Storage | `rusqlite` (bundled SQLite) | State, memory, run index |
| Search | embedded `ripgrep` | Code search tool |
| Diff | `similar` | Patch generation/application |
| Errors | `thiserror` + `anyhow` | Typed errors at module boundaries |
| Logging | `tracing` | Structured logs separate from event stream |

---

## 2. Conceptual Model

**The single primitive is the `AgentRun`:**

```
AgentRun = Identity (SOUL) + BrainPolicy (model via Router) + AutonomyContract
         + ToolSet + MemoryHandle + Workspace (sandboxed) + the Engine loop
```

Everything is a configuration of this primitive:
- **Claude Code** ≈ one run, neutral identity, autonomy=Supervised, workspace=local repo.
- **Codex** ≈ one run, variable autonomy, workspace=isolated sandbox.
- **A swarm** ≈ multiple runs sharing one workspace.

**Three orthogonal axes** the whole system rides on:
1. **Model** = a Router decision (not a user choice).
2. **Autonomy** = a dial (Supervised → Trusted → Autonomous).
3. **Surface** = a thin client over the headless runtime.

---

## 3. Module Map

```
Tier 0  config · auth · provider · tools · sandbox
Tier 1  router · engine · memory
Tier 2  autonomy · agent · capabilities
Tier 3  orchestrator · scheduler · runtime
Tier 4  surfaces (tui · cli · api · gateway)
```

### Tier 0

- **config** — Loads/merges config from defaults → `config.toml` → env (`SPARROW_*`) → CLI flags. Hot-reloadable for daemon.
- **auth** — Stores credentials. Priority: OS keychain → encrypted file (age/chacha20poly1305) → env. Per-backend, not all-or-nothing.
- **provider** — Uniform interface over every model vendor. Normalizes messages, streaming, and tool-calling.
- **tools** — What an agent can do. Every tool declares a JSON schema and a risk level.
- **sandbox** — Isolates exec/mutating actions. Backends: local, local-hardened, docker, ssh, modal, daytona, vercel.

### Tier 1

- **router** — Turns "describe a task" into "the right model now". Delivers the no-lock-in promise.
- **engine** — The heart. Drives one `AgentRun` to completion: classify → assemble context → stream → tool use → observe → checkpoint → repeat.
- **memory** — Four tiers: repo (code intelligence), identity (persistent SOUL), task (conversation state), shared (cross-agent workspace).

### Tier 2

- **autonomy** — Continuous trust contract attached to every run. State machine, not modes.
- **agent** — Persistent agent definitions (SOUL files). Survives across sessions.
- **capabilities** — Skills + self-improvement + MCP client.

### Tier 3

- **orchestrator** — Runs multiple AgentRuns over a shared workspace. Default pipeline: Planner → Coder → Verifier.
- **scheduler** — Cron + natural-language → cron triggers. Jobs run unattended.
- **runtime** — Headless daemon + event bus. Owns persistent agents, scheduler, gateway.

### Tier 4

- **tui** — ratatui flagship. Cockpit, swarm lanes, diff view, checkpoint timeline, replay scrubber.
- **cli** — One-shot headless: `sparrow run "..." --json`.
- **api** — Local Unix socket + WebSocket.
- **gateway** — Telegram, Discord, Slack, WhatsApp, Signal, Email.

---

## 4. The Event Model

The load-bearing contract connecting runtime ↔ surfaces ↔ replay:

```rust
pub enum Event {
    RunStarted, RouteSelected, ModelSwitched, ThinkingDelta, Message,
    ToolUseProposed, ApprovalRequested, ApprovalResolved, ToolUseStarted, ToolOutput,
    DiffProposed, DiffApplied, TestResult,
    AgentSpawned, AgentStatus,
    CheckpointCreated, SkillLearned,
    CostUpdate, TokenUsage, AutonomyChanged,
    RunFinished, Error,
}
```

Every surface renders from this stream; replay records from it.

---

## 5. Autonomy & Checkpoints (The Moat)

**Autonomy dial**: Supervised (0.0) → Trusted (0.5) → Autonomous (1.0).

| RiskLevel | Supervised | Trusted | Autonomous |
|---|---|---|---|
| ReadOnly | Allow | Allow | Allow |
| Mutating | Ask | Notify+checkpoint | Allow+checkpoint |
| Exec | Ask | Notify (sandbox) | Allow (sandbox) |
| Destructive | Deny | Ask | Ask |
| Network | Ask | Allow | Allow |

**Checkpoints**: Before any mutating batch, snapshot workspace via git. Expose timeline. `rewind [n]` restores.

---

## 6. Routing & Cost Engine

- Classification: heuristic + tiny model call for ambiguous cases.
- Selection scoring: `score = capability_fit − cost_weight·$est − latency_weight·lat`
- Fallback: on 429/5xx/timeout/refusal, advance chain. Local (Ollama) is terminal fallback.
- Budgets: per-session and per-day. On breach, downgrade or hard-stop.

---

## 7. Self-Improvement Loop

1. Run completes → engine proposes skill candidates.
2. Curator (cron) grades, consolidates, dedupes, prunes.
3. Memory distills durable user facts (redacted, user-editable).
4. Net effect: Sparrow gets more capable and personalized the longer it runs.

---

## 8. Reproducibility & Replay

Every run recorded as append-only transcript:
- `inputs.json` (task, config snapshot, model/seed, repo HEAD)
- `events.jsonl` (the Event stream)
- `checkpoints` refs

Replayable: `sparrow replay <run-id>` re-renders and can re-execute against chosen model.

---

## 9. CLI Grammar (v1)

```
sparrow                                  # launch TUI
sparrow setup                            # conversational onboarding
sparrow run "<task>"                     # one agentic run
sparrow swarm "<task>"                   # planner→coder→verifier
sparrow schedule "<task>" --cron "<expr>"
sparrow model --list                     # show routing
sparrow auth add <provider>             # add credentials
sparrow skills list|create|prune         # skill library
sparrow mcp add|list|rm <server>         # MCP connectors
sparrow checkpoint list                  # snapshots
sparrow rewind [<id|n>]                  # restore checkpoint
sparrow replay <run-id>                  # re-render transcript
sparrow gateway start|status|stop        # messaging daemon
sparrow profile create|list|use          # isolated instances
sparrow import openclaw [path]           # migrate from OpenClaw
sparrow config [edit]                    # config.toml
sparrow update                           # self-update
sparrow doctor                           # diagnostics
```

---

## 10. Testing Strategy

- Unit per module against its trait with mock Brain/Tool/Sandbox.
- Golden replay tests: record transcript, assert deterministic re-render.
- Router simulation: synthetic budgets/errors → assert fallback order + budget stops.
- Autonomy matrix tests: every (level × RiskLevel) → expected Decision.
- Sandbox escape tests: writes outside workspace / network when denied → blocked.
- Swarm pipeline test: inject failing diff → assert REWORK → PASS lands only after verify.
- Redaction tests: secrets never appear in events/transcripts/memory.

---

## 11. Configuration Reference

```toml
[defaults]
autonomy = "trusted"
sandbox  = "local-hardened"
theme    = "captain"

[routing]
free_first = true
policy = { trivial = "local", small = "groq", medium = "nvidia", hard = "anthropic", vision = "anthropic" }
on_budget = "downgrade"

[budget]
daily_usd = 5.0
session_usd = 1.0

[providers.anthropic]
adapter = "anthropic-messages"
models  = ["claude-sonnet-4-6"]

[providers.ollama]
adapter = "ollama"
base_url = "http://localhost:11434/v1"
models = ["qwen3.5:32b"]

[surfaces.telegram]
enabled = true
allow_users = [1780070685]
```

---

## 12. Brand Identity

- **Name**: Sparrow. **Persona**: pirate + builder.
- **Tagline**: `one cli · grows with you`.
- **Voice**: concise, competent, a wink of pirate-builder character; never sycophantic.
- **Font**: IBM Plex Mono everywhere.
- **Mascot**: chubby pirate sparrow — two-feather crest, thick eyebrow, open eye + patch, coral beak, pink cheek, cream belly, key in wing.

---

*End of spec. Build in the M0→M6 order; treat the Event model and autonomy+checkpoints as the load-bearing contracts.*
