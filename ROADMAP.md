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

## 🔵 v0.4.0 — next

| Item | Status | Notes |
|---|:---:|---|
| Real external memory providers | Planned | mem0, honcho, supermemory integrations |
| Signed cross-platform release artifacts | Planned | GitHub Actions build + sign for Linux/macOS/Windows |
| VS Code extension | Planned | IDE surface for commands and event stream |
| Gateway abort via WebSocket | Planned | Console can cancel a running job in one click |
| Per-model context window compaction | Planned | Replace global 120k-char threshold with per-model window |
| Automatic `MEMORY.md` curation | Planned | Engine prunes stale facts on each session |
| Agent marketplace browser | Planned | WebView `/agents` panel for browsing and installing agent files |

## 🔵 v0.5.0 — later

| Item | Status | Notes |
|---|:---:|---|
| Telegram / Discord / Slack E2E | Partial | Transport implementations exist; real account tokens needed |
| OAuth device flow | Alpha | GitHub/Google/Microsoft login needs client IDs |
| LSP integration | Experimental | Language server protocol for IDE-grade tool calls |
| Cloud sandbox providers | Experimental | Modal, Daytona, Vercel, Singularity — honest errors now, real backends later |

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
