# Roadmap

Sparrow uses evidence-based status labels:

- **Stable**: compiled, tested, and used by at least one runnable path.
- **Alpha**: implemented and tested locally, but needs more real-world validation.
- **Partial**: meaningful code exists, but the end-to-end promise is not fully wired.
- **Experimental**: adapter, shell, or prototype exists.
- **Planned**: not implemented.

---

## ✅ v0.1.0 — shipped 2026-05-31

Initial kernel: Rust engine, 35-provider registry, TUI, WebView, gateway transports, skills, memory, replay, and 84 tests.

## ✅ v0.2.0 — shipped 2026-06-02

Autonomy phases 1–13 landed. Plan mode, permission modes, hooks, declarative agents, skills/plugins, tool metadata, gateway run registry, security audit, sandbox policy, media tools, GitHub Action, context/compaction, TUI `@` picker, theme variants.

## ✅ v0.3.0 — shipped 2026-06-02

WebView Cockpit — 3-column layout, typed event cards, full composer (slash palette, `@` picker, history, drag-and-drop), approval modal, Captain/Paper themes with test coverage.

---

## ✅ v0.4.0 — shipped 2026-06-05

Crates.io publication, first-run wizard, `sparrow demo`, `sparrow share`,
`sparrow hook install`, 10 community skills, humanized error messages.

## ✅ v0.5.0 — shipped 2026-06-06

Phase 1 CLI rich-rendering engine (markdown / code / diff / JSON / table
formatters and a streamed renderer).

## ✅ v0.5.1 — shipped 2026-06-06

Phase 2 streaming + chat composer (live events, progress, persistent
sessions). Phase 3 TTS / STT / `file_search` plus 30 community skills.
Phase 4 Memory CLI + FTS5 session search. Phase 5 `sparrow voice`.

## ✅ v0.5.2 — shipped 2026-06-07

Upward merge of the public launch track with the Tier 2 hardening track:
crate renamed to `sparrow-cli`, `[lib]` section for embedding, drop-in
reader for `~/.claude/{CLAUDE.md, commands/, agents/, settings.json}`.

## ✅ v0.5.3 — shipped 2026-06-07

Packaging: Homebrew tap, Scoop bucket, winget manifests. VS Code
extension scaffold. Opt-in telemetry skeleton (off by default) +
`PRIVACY.md`. Hard-cap CLI flags `--max-cost-usd`, `--max-wall-secs`,
`--max-tokens`, `--bind`. Nightly CI workflow + cosign release-signing
workflow.

## ✅ v0.5.4 — shipped 2026-06-07

Launch-asset polish — real 30-second asciinema cast, branded OG/comparison/
install hero cards. Repo-hygiene additions: `SUPPORT.md`, `.github/
FUNDING.yml`.

## ✅ v0.5.5 — shipped 2026-06-08

Critical: a single model turn emitting N tool calls is now replayed as
one assistant message with `reasoning_content` + `tool_calls[N]`. Fixes
the HTTP 400 loop against DeepSeek / Qwen / Moonshot thinking-mode
providers (and routes like `opencode-go`) that left multi-file tasks
half-done.

## ✅ v0.5.6 — shipped 2026-06-08

TUI `ansi_bridge` renders ANSI-coloured agent output inside cockpit panels.

## ✅ v0.5.7 — shipped 2026-06-09

`web_search` and `code_exec` tools wired into the engine; 8 skills
enriched with their usage.

## ✅ v0.5.8 — shipped 2026-06-09

Windows TUI UTF-8 console (mojibake fixed). WebView protocol-prefix
isolation (no more agent/model selector blobs leaking to the model or
the transcript). DeepSeek inline-markup tool-call recovery. `GET
/healthz` for IDE liveness probes. Audit-clean: all tests + clippy
`-D warnings` green.

## 🔵 Next

| Item | Status | Notes |
|---|:---:|---|
| Telegram / Discord / Slack E2E | Partial | Transport implementations exist; real account tokens needed |
| OAuth device flow | Alpha | GitHub/Google/Microsoft login needs client IDs |
| LSP integration | Experimental | Language server protocol for IDE-grade tool calls |
| Cloud sandbox providers | Experimental | Modal, Daytona, Vercel, Singularity — honest errors now, real backends later |
| Agent marketplace browser | Planned | WebView `/agents` panel for browsing and installing agent files |
| Per-model context-window compaction | Planned | Replace global 120k-char threshold with per-model window |
| External security audit | Planned | Trail of Bits / GitHub Security Lab |

---

## Current Alpha Detail

| Area | Status | Notes |
|---|:---:|---|
| Core event model | ✅ Stable | `Event` is the load-bearing contract across surfaces and recorder |
| Brain/provider abstraction | ✅ Stable | Unified `Brain`, `BrainRequest`, `BrainEvent`, `ToolSpec` |
| Engine loop | ✅ Stable | Think/tool/observe loop with routing, tokens, redaction, event emission |
| Provider registry | ✅ Stable | 30+ providers; boot-time auto-discovery; 24h SQLite cache |
| Routing | ✅ Stable | Budget-aware fallback, capability scoring, tier-aware latency |
| Autonomy | ✅ Stable | 15-combination matrix; continuous float dial via `--autonomy 0.7` |
| Memory | ✅ Stable | SQLite + redaction; Distiller auto-extracts user facts |
| Swarm orchestrator | ✅ Stable | Planner/Coder/Verifier with real tool dispatch; empty-diff guard |
| Model discovery | 🔶 Alpha | `GET /v1/models` for OpenAI-compat; `/api/tags` for Ollama |
| Ollama adapter | 🔶 Alpha | Native `/api/chat` streaming; boot-time local model discovery |
| Anthropic adapter | 🔶 Alpha | Streaming tool-use IDs fixed and tested |
| Session continuity | 🔶 Alpha | Gateway + CLI share `SessionStore`; round-trip tested |
| Checkpoint/rewind | 🔶 Alpha | Git refs/stash; `diff` and `prune` sub-commands |
| Auth OAuth | 🔶 Alpha | Device flow wired; needs client IDs for each provider |
| Skills/Curator | 🔶 Alpha | Filesystem skills + auto-learn from successful runs |
| MCP client | 🔶 Alpha | stdio/HTTP client; `mcp add --command` persists config |
| Scheduler/recorder/replay | 🔶 Alpha | Cron loop; transcript recording in all run modes |
| Telegram/Discord/Slack | 🔸 Partial | Implementations exist; account-token E2E validation pending |
| Extra gateway transports | 🧪 Experimental | WhatsApp/Signal/Email/Feishu return explicit unsupported errors |
| Cloud sandboxes | 🧪 Experimental | Modal/Daytona/Vercel/Singularity return honest errors |
| LSP / tree-sitter | 🧪 Experimental | Feature-gated; not wired to the engine |
| Cross-platform release | 📋 Planned | Workflows exist; signed artifacts not yet published |
