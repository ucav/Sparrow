# Changelog

All notable changes to Sparrow will be documented in this file.

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
