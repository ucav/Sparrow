# Changelog

All notable changes to Sparrow will be documented in this file.

## [Unreleased]

### Added
- Honest module audit at `docs/AUDIT.md`
- GitHub repository polish checklist at `docs/GITHUB_POLISH.md`
- README status model based on Stable/Alpha/Partial/Experimental/Planned evidence
- Canonical v1 technical specification (`docs/technical-spec.md`)
- Complete Rust kernel: config, auth, provider, tools, sandbox, router, engine, autonomy
- 35 provider registry with model tags and recommended defaults
- Agentic engine loop with budget-aware routing and fallback chains
- Multi-agent swarm orchestrator (Planner → Coder → Verifier)
- 4-tier persistent memory (SQLite): repo, identity, task, shared
- Self-improving skill system with Curator
- Cron scheduler with job persistence
- Run recorder/replayer with transcript format (`inputs.json` + `events.jsonl`)
- Terminal TUI (ratatui) with cockpit, scroll, ASCII mascot
- WebView console (HTTP + WebSocket + JS event client)
- Gateway transports: Telegram, Discord, Slack, WebSocket API, plus experimental extra adapters
- CLI grammar (clap) with 20+ subcommands
- `--json` NDJSON output for CI/hooks
- Install script (`install.sh`)
- 84 local tests across unit/integration/bench harnesses
- Branding assets: SVG mascot, cockpit mark, ASCII variant, presentation HTML

### Changed
- CI now targets both `master` and `main`
- `Cargo.toml` repository metadata now points to `https://github.com/ucav/Sparrow`
- README no longer marks unproven surfaces as complete
- M0/M6 examples use current CLI syntax and alpha status language
