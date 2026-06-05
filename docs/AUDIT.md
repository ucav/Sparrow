# Sparrow Audit

This audit reflects the current `master` branch after the v0.3.6 finalisation pass. It is intentionally stricter than the product vision: a module is marked **REAL** only when there is compiled code and a tested or manually verified path.

## Acceptance Evidence

| Check | Result |
|---|---:|
| `cargo check --all-targets` | Pass |
| `cargo build --release` | Pass |
| `cargo build --release --features treesitter` | Pass |
| `cargo test --all-targets` | Pass |
| `cargo clippy --all-targets -- -D warnings` | Pass |
| `cargo fmt --all -- --check` | Pass |
| Reasoning deltas | Pass; `ReasoningDelta` is persisted for provider continuity but filtered from public NDJSON/WebSocket streams |
| Config hot reload | Pass; watcher reloads changed `config.toml` without emitting unchanged-file noise |
| WebView console | Pass by source tests for slash palette, composer, upload, code cards, cost/token meters, swarm lanes, and paper/captain themes |
| Browser/computer-use | Pass; Playwright screenshot/click/type E2E is green |

## Core Modules

| Module | Status | Evidence / Notes |
|---|---:|---|
| `src/event.rs` | REAL | Central serialized event contract used by engine, gateway, recorder, tests. |
| `src/provider/mod.rs` | REAL | `Brain`, `BrainRequest`, `BrainEvent`, `ModelCaps`, `ToolSpec`. |
| `src/provider/ollama.rs` | REAL | Native Ollama stream adapter exists and compiles. |
| `src/provider/openai_compat.rs` | REAL | Used for NVIDIA/OpenRouter/Groq-style APIs. |
| `src/provider/anthropic.rs` | REAL | Streaming parser exists; tool-use ID mapping repaired. |
| `src/provider/responses.rs` | REAL | Responses adapter serializes images/cache controls, captures reasoning deltas, and reinjects `reasoning_content`; Bedrock is an explicit unsupported-provider error, not a fake success. |
| `src/router/mod.rs` | REAL | Budget-aware fallback routing with local/free preference, explicit provider override, tool/vision penalties, and regression coverage. |
| `src/engine/mod.rs` | REAL | `Task`, `Engine`, and `drive()` exist. Signature: `drive(Task, UnboundedSender<Event>) -> anyhow::Result<OutcomeSummary>`. |
| `src/autonomy/mod.rs` | REAL | Autonomy matrix plus rich verdicts (`decision`, `needs_checkpoint`, `notify`, `reason`) are covered by tests and wired into the engine. |
| `src/agent/mod.rs` | REAL | Persistent TOML and Markdown-frontmatter agents survive sessions; unsafe traversal names are rejected. |
| `src/capabilities/*` | REAL | Skill library, curator, progressive references/templates/scripts/assets, plugins, and MCP stdio/HTTP clients are covered by focused tests; skill paths are traversal-safe and curator preserves assets. |
| `src/redaction.rs` | REAL | Secret redaction has unit/integration coverage. |
| `src/memory/mod.rs` | REAL | SQLite memory persistence covered by tests. |
| `src/tools/*` | REAL | Core fs/edit/exec/git/search, LSP, media, browser/computer-use, symbols, memory, and knowledge graph tools compile and have focused tests. Unsupported integrations return explicit errors. |
| `src/sandbox/mod.rs` | REAL | Local policy, path isolation, env allowlist, worktree, Docker/SSH surfaces, and honest unsupported cloud/local-hardened failures are covered by tests. |
| `src/orchestrator/mod.rs` | REAL | Planner/coder/verifier flow is covered by a verifier-gate test proving diffs are emitted only after PASS. |
| `src/runtime/*` | REAL | Event bus, scheduler, recorder, replay, public-event filtering, and socket streaming paths exist; unsupported platform pieces are explicit no-ops/errors. |
| `src/gateway/mod.rs` | REAL | Message routing and response redistribution are wired. |
| `src/gateway/ws.rs` | REAL | Client tracking and response delivery tested locally. |
| `src/gateway/telegram.rs` | PARTIAL | Real Telegram long-polling path exists; token-backed E2E not recorded in CI. |
| `src/gateway/discord.rs` | PARTIAL | Discord gateway path exists; account-backed E2E not recorded in CI. |
| `src/gateway/slack.rs` | PARTIAL | Slack Socket Mode path exists; account-backed E2E not recorded in CI. |
| `src/gateway/extra_transports.rs` | EXPERIMENTAL | Some send paths exist; unsupported transports return explicit errors instead of fake success. |
| `src/console.rs` + `console.html` | REAL | Local WebView HTTP/WebSocket surface plus slash commands, uploads, context meter, compact code cards, cost filtering, approval modal, and typed events are covered by UI tests. |
| `src/tui/*` | REAL | Ratatui cockpit exists with animated brand, swarm lanes, checkpoint/diff/cost panels, history and agent picker; stability tests cover v0.2/v0.3 contracts. |
| `src/onboarding/*` | PARTIAL | Setup agent, fallback interactive setup, and migration pieces exist; enterprise IDE integrations are template-level. |

## Prompt Reconciliation

The pasted prompt said:

- `src/engine.rs` is missing.
- The project does not compile.
- There are only three tests.
- CI only targets `main`.

Current reality:

- Engine exists at `src/engine/mod.rs`, exported by Rust's module directory convention.
- `cargo check --all-targets`, `cargo build --release`, `cargo build --release --features treesitter`, `cargo test --all-targets`, `cargo clippy --all-targets -- -D warnings`, and `cargo fmt --all -- --check` pass locally.
- The integration suite now covers routing, fallback wording, blank-theme normalization, non-chat discovery filtering, concise routing-chain summarization, checkpoint rewind, knowledge graph persistence, sandbox policy, browser/computer-use, UI finalisation, provider reasoning continuity, and symbol indexing.
- CI needed branch correction and now targets `master` and `main`.

## Current Repair Notes

- `model --list` awaits model discovery before printing, using either environment variables or credentials stored by `sparrow auth add`.
- NVIDIA is no longer represented as a single Nemotron entry. The static recommended chain includes `meta/llama-3.1-8b-instruct`, `stepfun-ai/step-3.5-flash`, and `nvidia/nemotron-3-super-120b-a12b`, while live discovery adds the wider chat-capable API catalog and filters embeddings/retrieval/parse/safety-only entries.
- `sparrow model --set nvidia` can clear an older one-model local config and restore the recommended NVIDIA chain.
- `--model nvidia:<model>` keeps the exact requested cloud model first for trivial/small prompts instead of forcing Ollama or another discovered NVIDIA model ahead of it.
- DeepSeek/Qwen/Moonshot-style `reasoning_content` is captured and persisted, but public output suppresses raw `ReasoningDelta` fragments.
- The daemon config watcher has a regression test proving changed config reloads while unchanged files do not spam reload events.
- `treesitter` remains opt-in and the release build passes both with and without it.
- Tier 2 is now locked by tests: trusted autonomy emits checkpoint/notify intent, persistent agents reject path traversal, skill invocation cannot escape its skill folder, and the curator no longer deletes progressive-disclosure assets.

So the correct action is not to create a competing `src/engine.rs`; it is to document the actual engine signature, keep tests honest, and improve CI/readme trust.

## Feature Status Rules

Use these labels in README and docs:

- **REAL**: compiled, wired, and covered by automated test or manual smoke test.
- **PARTIAL**: meaningful implementation exists but not enough E2E proof.
- **EXPERIMENTAL**: adapter/shell/prototype exists.
- **PLANNED**: not implemented.

Avoid marking a module complete because a file exists. Mark it complete only when the behavior is exercised.

## Remaining Game-Changers To Prove

1. **Provider matrix:** keep a generated configured/available/tested provider table in sync with the registry and discovery cache.
2. **Gateway account E2E:** validate Telegram/Discord/Slack with real account tokens before marking those transports stable.
3. **TUI screenshot regression:** add a pixel/snapshot artifact for the Ratatui cockpit.
4. **Release trust:** keep signed binaries/checksums attached to every public tag.

Recently proved:

- Verifier gate: `tests/orchestrator_gate.rs` proves diffs are emitted only after verifier PASS.
- Ollama stream path: `tests/ollama_stream.rs` keeps a local mock stream in CI.
- Security posture: redaction, sandbox path isolation, org policy, and no-hardcoded-secret tests pass; security docs live in `docs/memory.md`, `docs/sandboxing.md`, and `docs/autonomy.md`.
