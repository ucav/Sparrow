# Changelog

All notable changes to Sparrow will be documented in this file.

## [0.3.5] — 2026-06-04

Public polish pass for the WebView cockpit and repository surface.

### Added
- Prompt caching request controls for Anthropic Messages and
  OpenAI-compatible/Responses providers. Anthropic stable system prompts receive
  `cache_control`; OpenAI-compatible requests receive `prompt_cache_key` and
  `prompt_cache_retention`.
- WebView plan mode now has explicit accept, edit, and reject actions.
- Diff cards and the side diff panel now render patches as per-hunk review
  blocks with accept/reject states.
- Local syntax highlighting for streamed code cards, covering common languages
  without a CDN dependency.
- Regression tests that lock code-card styling, metric-spam behavior, and
  WebView typography, plan actions, provider cache payloads, and hunked diffs.

### Fixed
- `CostUpdate` events no longer print one transcript line per streamed metric
  tick; they update the live meters only.
- Empty prose fragments between adjacent fenced code blocks are removed, so
  collapsed cards no longer create large blank gaps.
- Code cards are denser, easier to scan, and preserve copy/line-count behavior.

### Polished
- WebView typography now uses a system UI stack with a modern monospace stack
  for code.
- Root-level scratch logs, local handoffs, and presentation-only artifacts were
  removed from the public tree.
- README status and release metadata now reflect the v0.3.5 build.

## [0.3.3] — 2026-06-03

Public-release hardening pass. Fixes every issue from hands-on testing and
stabilizes the WebView cockpit for local use.

### Fixed (critical)
- **Context truly persists now.** The engine emits the assistant response as
  `ThinkingDelta`, not `Message`, so the previous capture never fired. The
  WebView command loop now accumulates streamed deltas and flushes one
  assistant turn into a persistent history on `RunFinished`. Context survives
  model switches AND separate prompts.
- **Sessions persist across restarts.** The conversation is saved to the
  SQLite `SessionStore` under id `webview` and re-hydrated on launch — it
  appears in the `/sessions` panel and reloads next time.
- **Counters animate live and never stick at zero** (direct `textContent`
  write + easing layer; the rAF-only path failed in background tabs).
- **All models are visible.** `/models` merges live-discovered models with the
  curated registry — NVIDIA jumps from 6 to 82 models in the picker/config.

### Added
- **Stop button** next to Run aborts the active task (`POST /stop`).
- **Mid-run injection**: typing while a task runs injects the text into the
  live run's context instead of starting a new run.
- **Collapsible code cards**: assistant code output is split out of the stream
  into `<details>` blocks with language label, line count, and copy button.
- **Auto-collapsing run summary** that re-expands on click.

### Polished UI
- Fixed vertical char-by-char text wrapping (`overflow-wrap` instead of the
  deprecated `word-break:break-word`).
- Drawer shows only the clicked panel, not all metrics stacked.
- Context bar fills with a green → amber → coral → red gradient.
- Animated pulsing green dot on the active session.
- Removed the duplicated dollar figure; budget shows `unlimited` when unset.
- Removed the misaligned fixed "live" tag (the chrome bar already has one).

## [0.3.2] — 2026-06-03

### Fixed (critical bugs from user report)
- **Conversation context was dropped on every new run/model switch.**
  `main.rs` now keeps a persistent `Arc<Mutex<Vec<Msg>>>` of the dialog
  shared across runs; every command pulls prior turns into `Task.context`
  (previously hardcoded to `vec![]`) and pushes user+assistant messages
  back, capped at the last 40 turns.
- **Cockpit counters never updated** because the rAF-based `countUp`
  didn't fire in background tabs / headless contexts. `refreshTokens()`
  now writes `textContent` directly first, then runs `countUp` as an
  optional easing layer. Tokens, cost, and budget all animate live.
- **No way to see which providers / models are configured.** The full
  35-provider · 60+-model registry is now visible in the config panel
  as expandable cards (was a single dropdown showing one provider at a
  time). Search, default-route picker, and per-provider key input added.

### Added
- **`POST /conversation/reset`** endpoint + UI button (`⟲ new conversation`)
  that clears retained context when the user wants a fresh start.
- **Live "context retained · turn #N"** indicator on every `RunStarted` from
  turn 2 onward; **"● context preserved"** marker on every `ModelSwitched`.
- **BUDGET pill** in cockpit row: live `<pct>% / $<daily> day` with color
  gradient (green → gold → coral → red) as the cap is approached.
- **`/conversation/reset`**, **`/status`**, **`/file`**, **`/models`**
  endpoints fully wired.
- **Config panel** rebuilt with 3 tabs (providers&models / routing&autonomy /
  permissions). Each provider card shows LED health · adapter · notes ·
  per-model row with ★ recommended, ctx window, $/Mtok cost, "set default"
  button, and API key input.

### Polished
- Welcome panel: shows `<N> providers · <M> models · <K> with key` and
  the boot timestamp.
- Route bar: 3-hop fallback chain with animated arrows.
- Tool cards: summary like `2 files · 6 KB` for fs_read, `<N> hits` for
  search, matching the validated mockup.

## [0.3.1] — 2026-06-03

### Added — Rust
- `GET /models` endpoint: returns the full provider registry (35 providers,
  60+ models) with `context_window`, `cost_in`, `cost_out`, and `recommended`
  fields — used by the WebView model picker and provider health dots.
- `RunRequest.model_override`: optional field; when set the backend injects
  `__model:X__` into the task string.
- Engine parses and strips `__model:X__` prefix; filters the routing chain to
  the requested brain ID (falls back to full chain if not found).
- `list_agents` scans three directories (user config, `agents/`, `.sparrow/agents/`)
  and deduplicates by name — the 5 repo soul files appear in the Crew panel
  without any install step.
- `RouteSelected` event now carries `context_window: u64` from the primary
  brain's `caps()`, fixing the hardcoded 32k display in the WebView.

### Added — WebView (`console.html`)
- **Model picker dropdown**: the `⬡ auto` pill expands to a full popover
  grouped by provider; each model shows context window and cost; provider rows
  show a green/dim LED based on `has_credential` from `/config`; selection
  stored in `localStorage` and injected into `POST /run`.
- **Side diff viewer**: `.diff-panel` slides in from the right on every
  `DiffApplied` event; shows file path, `+N −M` stats, and colour-coded diff
  lines; `D` key and `✕` button toggle it; `Esc` closes.
- **Run summary card**: replaces the bare `✓ done` log line with a structured
  card showing cost, token count, files edited, checkpoint count, and duration.
- **Dynamic context bar**: `contextLimit` updates from `RouteSelected.context_window`
  and shows the real model limit (200k for Claude, 131k for NVIDIA/Llama4, etc.).
- **Provider health dots**: model picker dropdown renders a green LED next to
  providers that have a credential configured; dim dot for unconfigured.
- **Crew panel live status**: `AgentSpawned` and `AgentStatus` events update
  each agent row's status pill and note in real time via `updateCrewLiveStatus()`.
- **Checkpoint scrubber**: `addCheckpointNode()` stores checkpoint IDs; each
  node is clickable — confirms and issues `/rewind <id>` via the composer.
- **Dynamic home screen**: `bootIntro()` is now async; fetches `/models` and
  `/config` at startup to show real provider counts and active-provider count.
- **Hero stats**: `hero-active` shows count of providers with credentials;
  `hydrateHero()` caches configured provider IDs for the model picker dots.
- **Keyboard shortcuts**: `D` toggles diff panel; `P` opens model picker;
  both disabled when focus is inside an input/textarea.
- **Escape**: closes diff panel and model picker dropdown in addition to
  existing modals (approval, config, palette, history draft restore).

### Added — Agents / Docs
- Agent SOUL files enriched: `description`, `color`, `max_turns` added to all
  five repo agents (planner, coder, verifier, debugger, researcher); prompts
  tightened with step-by-step rules and better model assignments.
- `docs/keyboard.md`: updated with new shortcuts (`D`, `P`, `Esc` additions)
  and a WebView panels reference table with rail icons and descriptions.

## [0.3.0] — 2026-06-02

WebView Cockpit v0.3.0 is shipped. The console now matches the presentation
direction as a real local cockpit rather than a static mockup: every panel is
fed by Sparrow endpoints, the composer is usable as a primary command surface,
and both Captain/Paper themes are covered by tests.

### Added
- Three-column WebView cockpit: icon rail, persistent drawer, main event
  stream, live cockpit metrics, boot animation, route chain, token/cost/context
  counters, and dynamic swarm row from `GET /agents`.
- Drawer panels wired to real endpoints: `/sessions`, `/memory`, `/plugins`,
  `/tools`, `/permissions`, `/security`, and `/artifacts`.
- Typed event renderers for tool cards, diff cards, compaction banners,
  checkpoint timeline nodes, streaming text caret, skill notifications, route
  updates, autonomy changes, token/cost updates, and model fallback labels.
- Command composer upgrades: `Cmd/Ctrl+K` slash palette, inline `@<agent>`
  picker, server-backed `/history`, multiline `Shift+Enter`, persisted draft,
  paste attachment handling, real `POST /upload`, drag-and-drop overlay, and
  live context meter.
- Approval modal wired to `POST /approval`, replacing inline-only approval
  controls for new approval events.
- Paper theme coverage, theme query override for screenshots/tests, persisted
  theme toggle, and reduced-motion fallback that disables animations, embers,
  transitions, and the boot overlay.
- WebView keyboard shortcuts documented in `docs/keyboard.md`.

### Validation
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all-targets`
- `cargo build --release`
- HTTP smoke: `GET /`, `/history`, `/agents`, `POST /upload`, and
  `GET /artifacts`.

## [0.2.0] — 2026-06-02

Autonomy mega-prompt phases 1–13 are landed and the deferred items have been
closed. Every WebView/CLI surface advertised in the README is wired and tested.

### Added — phases 1–11
- Read-only plan mode + slash command loader (`sparrow plan`, `/plan`).
- Permission modes (`read-only` / `plan` / `supervised` / `trusted` /
  `autonomous` / `emergency-stop`) and lifecycle hooks (`PreRun`, `PreToolUse`,
  `PostToolUse`, `PreCheckpoint`, `PostCheckpoint`, …).
- Declarative agents with `.agent.md` frontmatter and `agent mention`.
- Bounded `MEMORY.md` / `USER.md`, anti-injection memory guards, SQLite FTS
  session search, `memory` tool.
- Progressive skills (references + templates + scripts + assets loaded on
  invoke), plugin manifests, scanner, namespaced commands.
- Tool metadata (toolset / risk / auth / mutation / network / exec) with
  surface-aware filtering.
- Scoped gateway sessions, `gateway health`, `gateway abort`, sessions
  list/export/cleanup.
- `sparrow security audit [--json]` and WebView `/security`.
- Sandbox policy: protected paths, env allowlist, Docker/SSH/Worktree backends,
  honest errors when vendor CLIs (Modal/Daytona/Vercel/Singularity) are missing.
- `vision`, `image_generate`, `text_to_speech`, `transcribe` tools and WebView
  `POST /upload` (10 MB cap, classified) + `GET /artifacts`.
- `sparrow github review|status|logs` CLI plus composite `action.yml` and a
  sample PR-review workflow.

### Added — phase 12 (context & compaction)
- `ContextMeter` over prompt/memory/tools/attachments/transcript.
- `HookEvent::PreCompact` and `PostCompact`, `Event::Compacted`.
- `sparrow compact` writes a durable `HandoffDoc` Markdown.
- Engine auto-compaction: when the transcript exceeds 120k chars the loop
  collapses earlier messages, writes the handoff to `.sparrow/handoff/`, and
  emits `Event::Compacted` so the UI can render the pass.

### Added — phase 13 (UI / TUI)
- WebView `GET /sessions` endpoint.
- TUI `@<name>` inline agent picker with autocomplete.
- Theme variants `captain`, `midnight`, `paper` selected via `$SPARROW_THEME`.
- `docs/keyboard.md` listing every shortcut.

### Added — v0.2.0 stability pass
- `gateway::RunRegistry`: gateway daemon now registers spawned runs and
  honours `sparrow gateway abort <run>` by actually cancelling the matching
  task (previously only a signal file was written).
- Engine reflects auto-compaction in the event stream; the threshold and the
  `keep_last` window are constants so the behaviour is reproducible.
- Docs updated: `docs/keyboard.md`, `docs/security.md`, `docs/sandboxing.md`,
  `docs/media.md`, `docs/github-action.md`, `docs/compaction.md`,
  `docs/cli-reference.md`.
- README status table promoted from Alpha → Stable for everything covered by
  unit + integration tests.
- New regression suite `tests/v0_2_stability.rs` pinning the run-registry
  abort path, the TUI agent picker, and `CARGO_PKG_VERSION = 0.2.0`.

### Validation
All of the following pass on `master` at the v0.2.0 cut:
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all-targets` (26 test binaries, 188 assertions)
- `cargo build --release`

## [0.1.0] — 2026-05-31

Initial kernel and feature surface.

### Added
- Complete Rust kernel: config, auth, provider, tools, sandbox, router, engine, autonomy.
- 35-provider registry with model tags and recommended defaults.
- Agentic engine loop with budget-aware routing and fallback chains.
- Multi-agent swarm orchestrator (Planner → Coder → Verifier).
- 4-tier persistent memory (SQLite): repo, identity, task, shared.
- Self-improving skill system with Curator.
- Cron scheduler with job persistence.
- Run recorder/replayer with transcript format (`inputs.json` + `events.jsonl`).
- Terminal TUI (ratatui) with cockpit, scroll, ASCII mascot.
- WebView console (HTTP + WebSocket + JS event client).
- Gateway transports: Telegram, Discord, Slack, WebSocket API, plus experimental extra adapters.
- CLI grammar (clap) with 20+ subcommands.
- `--json` NDJSON output for CI/hooks.
- Install script (`install.sh`).
- 84 local tests across unit/integration/bench harnesses.
- Branding assets: SVG mascot, cockpit mark, ASCII variant, presentation HTML.
- Honest module audit at `docs/AUDIT.md`.
- README status model based on Stable/Alpha/Partial/Experimental/Planned evidence.
