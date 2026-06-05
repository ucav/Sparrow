# Sparrow V1 Completion Audit

This file tracks the current evidence for the user-facing V1 goal. It is not a vision document; every row should be backed by a command, test, local run, CI result, or source inspection.

## Current Evidence

| Requirement | Status | Evidence |
|---|---:|---|
| README reflects current project state | Pass | README status table updated for tests, routing, WebView, TUI, setup, and release state. |
| Local and GitHub are aligned | Pass before this final commit | `git rev-list --left-right --count HEAD...origin/master` returned `0 0` before the final local patch; final alignment is restored by the push for this batch. |
| CI is green | Pending for latest push | Previous GitHub Actions passed on Ubuntu, macOS, Windows, plus security audit; the current final commit needs CI to re-run after push. |
| Core Rust gates pass locally | Pass | `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo check --all-targets`, `cargo test --all-targets`, `cargo build --release`, and `cargo build --release --features treesitter`. |
| Sparrow is not pinned to Nemotron | Pass | `sparrow model --list` shows NVIDIA default chain: Llama 3.1 8B, Step 3.5 Flash, Nemotron Super. |
| NVIDIA discovery uses stored credentials | Pass | `sparrow model --list` and `sparrow doctor` show cached NVIDIA models without exporting `NVIDIA_API_KEY` in the shell. |
| Forced model routes are exact | Pass | `--model nvidia:meta/llama-3.1-8b-instruct` and `--model nvidia:stepfun-ai/step-3.5-flash` run with those models first. |
| Routing explanation understands Sparrow | Pass | The meta-routing question returns Sparrow-specific route criteria and concise fallback summary. |
| WebView runs locally | Pass | `http://127.0.0.1:9339/` returns 200 and `/config` shows NVIDIA configured with credential present. |
| WebView cockpit includes swarm and token/cost UI | Pass | `tests/ui_finalisation.rs` covers swarm hooks, composer/upload, slash palette, code cards, context/cost meters, route summarization, and cost-event filtering. |
| Raw reasoning deltas are hidden from public output | Pass | `Event::is_public`, `ndjson_output`, runtime sockets, console WebSocket, and gateway formatting suppress `ReasoningDelta`; tests cover the NDJSON path and provider/session continuity. |
| Config hot reload works | Pass | `tools::extras::watcher_tests::config_watcher_reloads_after_file_change` proves changed `config.toml` reloads and unchanged files do not emit reload noise. |
| Browser/computer-use works | Pass | `tests/browser_computer_e2e.rs` proves real Playwright screenshot plus click/type on a local page. |
| Gateway WebSocket works | Pass | `gateway start` exposes `ws://127.0.0.1:9338`; `/status` returns ACK then `Engine: online`. |
| TUI launches | Pass | Stability/UI tests cover v0.2/v0.3 contracts, theme lookup, cockpit controls, and agent picker behavior. |
| Release workflow readiness | Pass | README/status target v0.3.6, release workflow/build metadata, and both local release builds are green; the final pushed commit can be tagged through the normal release workflow. |
| Visual screenshots/GIFs exist | Pass | README references checked-in WebView screenshot assets and docs include tutorial videos. |
| Account-backed gateways are proven | Partial | WebSocket is locally proven; Telegram, Discord, and Slack need real token-backed E2E validation. |

## Current Repair Plan

1. Push this final local batch and wait for GitHub CI to validate the exact commit.
2. Keep the provider matrix generated from registry/discovery data.
3. Validate Telegram, Discord, and Slack with real account tokens before moving them from partial to stable.
4. Add a TUI screenshot/pixel regression artifact for future visual drift checks.
