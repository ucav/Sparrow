# Sparrow Audit

This audit reflects the repository state after commit `fdb403c` and the GitHub polish pass. It is intentionally stricter than the product vision: a module is marked **REAL** only when there is compiled code and a tested or manually verified path.

## Acceptance Evidence

| Check | Result |
|---|---:|
| `cargo check` | Pass |
| `cargo build` | Pass |
| `cargo test --all-targets` | Pass, 84 tests |
| `cargo clippy --all-targets -- -D warnings` | Pass |
| `cargo fmt --all -- --check` | Pass |
| CLI routing smoke test | Pass locally |
| WebView console | Pass locally on `127.0.0.1:9339` |
| Gateway WebSocket `/status` | Pass locally on `127.0.0.1:9338` |

## Core Modules

| Module | Status | Evidence / Notes |
|---|---:|---|
| `src/event.rs` | REAL | Central serialized event contract used by engine, gateway, recorder, tests. |
| `src/provider/mod.rs` | REAL | `Brain`, `BrainRequest`, `BrainEvent`, `ModelCaps`, `ToolSpec`. |
| `src/provider/ollama.rs` | REAL | Native Ollama stream adapter exists and compiles. |
| `src/provider/openai_compat.rs` | REAL | Used for NVIDIA/OpenRouter/Groq-style APIs. |
| `src/provider/anthropic.rs` | REAL | Streaming parser exists; tool-use ID mapping repaired. |
| `src/provider/responses.rs` | PARTIAL | OpenAI Responses/Bedrock-style paths exist; AWS signing is not production-complete. |
| `src/router/mod.rs` | REAL | Budget-aware fallback routing with local/free preference and tool/vision penalties. |
| `src/engine/mod.rs` | REAL | `Task`, `Engine`, and `drive()` exist. Signature: `drive(Task, UnboundedSender<Event>) -> anyhow::Result<OutcomeSummary>`. |
| `src/autonomy/mod.rs` | REAL | Autonomy matrix covered by integration tests. |
| `src/redaction.rs` | REAL | Secret redaction has unit/integration coverage. |
| `src/memory/mod.rs` | REAL | SQLite memory persistence covered by tests. |
| `src/tools/*` | PARTIAL | Core fs/edit/exec/git/search tools exist; LSP/image/TTS remain experimental shells. |
| `src/sandbox/mod.rs` | PARTIAL | Local/Docker/SSH surfaces exist; cloud backends are placeholders. |
| `src/orchestrator/mod.rs` | PARTIAL | Swarm flow and REWORK/PASS concepts exist; landed-only-after-PASS needs stronger E2E proof. |
| `src/runtime/*` | PARTIAL | Event bus, scheduler, recorder, replay exist; daemon lifecycle needs more production hardening. |
| `src/gateway/mod.rs` | REAL | Message routing and response redistribution are wired. |
| `src/gateway/ws.rs` | REAL | Client tracking and response delivery tested locally. |
| `src/gateway/telegram.rs` | PARTIAL | Real Telegram long-polling path exists; token-backed E2E not recorded in CI. |
| `src/gateway/discord.rs` | PARTIAL | Discord gateway path exists; account-backed E2E not recorded in CI. |
| `src/gateway/slack.rs` | PARTIAL | Slack Socket Mode path exists; account-backed E2E not recorded in CI. |
| `src/gateway/extra_transports.rs` | EXPERIMENTAL | Some send paths exist; unsupported transports return explicit errors instead of fake success. |
| `src/console.rs` + `console.html` | REAL | Local WebView HTTP/WebSocket surface tested manually. |
| `src/tui/*` | PARTIAL | Ratatui cockpit exists; needs screenshots and UX pass before “stable”. |
| `src/onboarding/*` | PARTIAL | Setup/migration pieces exist; enterprise IDE integrations are template-level. |

## Prompt Reconciliation

The pasted prompt said:

- `src/engine.rs` is missing.
- The project does not compile.
- There are only three tests.
- CI only targets `main`.

Current reality:

- Engine exists at `src/engine/mod.rs`, exported by Rust's module directory convention.
- `cargo check`, `cargo build`, and `cargo test --all-targets` pass locally.
- There are 84 tests across unit, integration, and bench harnesses.
- CI needed branch correction and now targets `master` and `main`.

So the correct action is not to create a competing `src/engine.rs`; it is to document the actual engine signature, keep tests honest, and improve CI/readme trust.

## Feature Status Rules

Use these labels in README and docs:

- **REAL**: compiled, wired, and covered by automated test or manual smoke test.
- **PARTIAL**: meaningful implementation exists but not enough E2E proof.
- **EXPERIMENTAL**: adapter/shell/prototype exists.
- **PLANNED**: not implemented.

Avoid marking a module complete because a file exists. Mark it complete only when the behavior is exercised.

## Remaining Game-Changers To Prove

1. **Verifier gate:** prove coder diffs are not applied until verifier PASS.
2. **Ollama E2E:** keep a lightweight mock in CI and a real local script for developers.
3. **Release trust:** publish a `v0.1.0-alpha` release with binaries and checksums.
4. **Screenshots/GIFs:** WebView console, TUI, routing stream, and gateway command.
5. **Provider matrix:** generated table of configured/available/tested providers.
6. **Security posture:** document redaction, sandbox limits, and gateway exposure clearly.
