# Sparrow V1 Completion Audit

This file tracks the current evidence for the user-facing V1 goal. It is not a vision document; every row should be backed by a command, test, local run, CI result, or source inspection.

## Current Evidence

| Requirement | Status | Evidence |
|---|---:|---|
| README reflects current project state | Pass | README status table updated for tests, routing, WebView, TUI, setup, and release state. |
| Local and GitHub are aligned | Pass | `git rev-list --left-right --count HEAD...origin/master` returns `0 0` after pushes. |
| CI is green | Pass | GitHub Actions passed on Ubuntu, macOS, Windows, plus security audit. |
| Core Rust gates pass locally | Pass | `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo check --all-targets --features email`, `cargo test`. |
| Sparrow is not pinned to Nemotron | Pass | `sparrow model --list` shows NVIDIA default chain: Llama 3.1 8B, Step 3.5 Flash, Nemotron Super. |
| NVIDIA discovery uses stored credentials | Pass | `sparrow model --list` and `sparrow doctor` show cached NVIDIA models without exporting `NVIDIA_API_KEY` in the shell. |
| Forced model routes are exact | Pass | `--model nvidia:meta/llama-3.1-8b-instruct` and `--model nvidia:stepfun-ai/step-3.5-flash` run with those models first. |
| Routing explanation understands Sparrow | Pass | The meta-routing question returns Sparrow-specific route criteria and concise fallback summary. |
| WebView runs locally | Pass | `http://127.0.0.1:9339/` returns 200 and `/config` shows NVIDIA configured with credential present. |
| WebView cockpit includes swarm and token/cost UI | Pass | `console.html` contains `swarm-cockpit`, `token-meter`, live cost/token handlers, and route summarization. |
| Gateway WebSocket works | Pass | `gateway start` exposes `ws://127.0.0.1:9338`; `/status` returns ACK then `Engine: online`. |
| TUI launches | Partial | Hidden-process smoke stays alive for 5 seconds; no screenshot regression yet. |
| Release artifact exists | Missing | `v0.1.0-alpha` release has not been published in this audit loop. |
| Visual screenshots/GIFs exist | Missing | README still lacks checked-in WebView/TUI/routing/gateway screenshots. |
| Account-backed gateways are proven | Partial | WebSocket is locally proven; Telegram, Discord, and Slack need real token-backed E2E validation. |

## Current Repair Plan

1. Keep CI/local gates green after each repair.
2. Add provider/reporting polish until CLI, WebView, and docs agree on the same model catalog.
3. Add visual evidence for WebView and TUI before promoting either to stable.
4. Prepare a first alpha release only after release workflow readiness is verified.
5. Validate account gateways with real tokens before moving them from partial to stable.

