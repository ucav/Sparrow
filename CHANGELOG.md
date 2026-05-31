# Changelog

All notable changes to Sparrow will be documented in this file.

## [Unreleased]

### Added
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
- Gateway transports: Telegram, Discord, Slack, WhatsApp, Signal, Email
- CLI grammar (clap) with 20+ subcommands
- `--json` NDJSON output for CI/hooks
- Install script (`install.sh`)
- 31 integration tests: autonomy matrix, router simulation, sandbox escape, provider registry, redaction
- Branding assets: SVG mascot, cockpit mark, ASCII variant, presentation HTML
