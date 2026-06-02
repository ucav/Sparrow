# Sparrow

**A local-first Rust agent cockpit for model routing, WebView control, rollback safety, and transparent cost.**

[![CI](https://github.com/ucav/Sparrow/actions/workflows/ci.yml/badge.svg)](https://github.com/ucav/Sparrow/actions/workflows/ci.yml)
[![Security Audit](https://github.com/ucav/Sparrow/actions/workflows/audit.yml/badge.svg)](https://github.com/ucav/Sparrow/actions/workflows/audit.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-green)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.96%2B-orange)](https://rust-lang.org)

<p>
  <img alt="Sparrow canonical logo" src="assets/brand/sparrow-mascot.svg" width="150">
</p>

Sparrow is an experimental single-binary CLI agent written in Rust. It is built around one event stream that can be rendered by a terminal UI, a WebView console, JSON output, or gateway surfaces. Its main design goal is simple: route each task to the cheapest capable model, keep the user in control, and make every run replayable.

Sparrow is inspired by tools like Claude Code, Codex, OpenCode, OpenClaw, and Hermes Agent, but it is intentionally local-first: Ollama can be the first hop, paid providers can be fallbacks, and checkpoints protect the workspace before mutating actions.

## Why Explore It

- **Model routing:** budget-aware fallback chains across Ollama, NVIDIA, Anthropic, OpenAI-compatible APIs, and other registry entries.
- **WebView console:** local cockpit at `http://127.0.0.1:9339/` with live route, token, cost, and config events.
- **Terminal-native:** TUI, `sparrow run`, `sparrow chat`, `sparrow --json run ...`, replay, memory, setup, and gateway commands.
- **Rollback safety:** Git-based checkpoints and `sparrow rewind`.
- **Persistent context:** SQLite memory, SOUL-style agent files, guarded skill registry, transcripts, and replay.
- **Gateway path:** Telegram, Discord, Slack, and WebSocket API are wired; extra transports are explicit adapters, not silently fake-successing.

## Status

Sparrow is **alpha software** with a green cross-platform CI baseline. The kernel, routing core, console surfaces, replay, checkpoints, and memory are wired and tested; external transports and release packaging still need real-world validation.

| Area | Status | Evidence |
|---|---:|---|
| CI / Rust build | Green | Latest completed GitHub Actions baseline passes on Ubuntu, macOS, and Windows; `cargo fmt`, `clippy -D warnings`, `check`, and release builds are covered |
| Test suite | Green | 109 tests pass locally with `cargo test`, including 95 integration tests |
| Security audit | Green | CI runs `rustsec/audit-check` on Ubuntu, macOS, and Windows |
| Engine loop | Stable | `src/engine/mod.rs`, JSON smoke tests, event stream, task classification, fallback execution, auto-checkpoint before mutating/exec/destructive, and auto-compaction when transcript exceeds the budget are wired |
| Provider routing | Alpha | Ollama + NVIDIA stored-credential discovery tested locally; 92 NVIDIA chat-capable models cached from `/v1/models`; explicit `nvidia:<model>` routing validated |
| WebView console | Stable | Full cockpit on port 9339 with rail/drawer panels, typed event stream, animated route/token/cost/autonomy indicators, canonical logo, dynamic swarm row, approval modal, Captain/Paper themes, context meter, slash palette, `@` picker, history, multiline composer, paste/upload, and drag-and-drop |
| TUI cockpit | Stable | Animated terminal cockpit, canonical ASCII logo, swarm lanes, checkpoint/diff/cost panels, `@<name>` agent picker, slash autocomplete, history, fold/unfold |
| Plan mode / slash commands | Stable | `sparrow plan`, WebView `/plan`, TUI `/plan`, built-in slash commands, user/project Markdown command discovery, and skill-to-slash exposure are wired read-only |
| Permissions / hooks | Stable | `sparrow permissions`, persisted permission modes, tool/path/provider/surface rules, WebView mode control, lifecycle hooks (`Pre`/`Post` for run/tool/checkpoint/compact) |
| Declarative agents | Stable | SOUL TOML plus Markdown frontmatter agents support role, prompt, model, permission mode, tool allow/deny metadata, `agent run`, `agent mention`, and `.agent.md` CRUD |
| Skills / plugins | Stable | Progressive skill references + templates + scripts + assets loaded on invoke, plugin manifests, namespaced plugin slash commands, plugin scanner, CLI install/list/remove, WebView `/plugins` |
| Toolsets | Stable | Known tools declare toolset/risk/auth/mutation/network/exec metadata; CLI `sparrow tools`, surface filtering, gateway-safe defaults, WebView `/tools` |
| Security audit | Stable | `sparrow security audit [--json]`, WebView `/security`, checks for permissions/gateway/tools/plugins/hooks/secrets/sandbox |
| Sandbox policy | Stable | `LocalSandbox` enforces workdir-inside-root, default protected paths (`.git`, `.env`, `.ssh`, …), env allowlist; Docker / SSH / Worktree backends wired; Modal/Daytona/Vercel/Singularity return honest errors when the vendor CLI is missing |
| Media tools | Stable | `vision`, `image_generate`, `text_to_speech`, `transcribe` hit OpenAI-compatible endpoints, honest errors on missing key/non-2xx; WebView `POST /upload` (10 MB cap, classified text/image/audio/pdf) and `GET /artifacts` |
| GitHub Action | Stable | Composite `action.yml`, sample `sparrow-pr-review.yml` workflow, `sparrow github review/status/logs` CLI, `--dry-run` review that needs no token, fails loudly on missing `GITHUB_TOKEN` or `gh` |
| Context / compaction | Stable | `ContextMeter`, `HookEvent::PreCompact`/`PostCompact`, `Event::Compacted` in the stream, engine-level auto-trigger when transcript > 120k chars, `sparrow compact` writes a durable `HandoffDoc` Markdown |
| UI / TUI | Stable | WebView `/sessions` + `/memory` + `/permissions` + `/plugins` + `/security` + `/upload` + `/artifacts` panels; WebView and TUI slash & `@` autocomplete, history, fold/unfold, context bars; theme variants `captain`/`midnight`/`paper`; keyboard shortcut docs |
| Gateway | Stable | `/status` command roundtrip tested on port 9338; scoped gateway sessions, health/abort commands with an in-process run registry that actually cancels active runs, session list/export/cleanup |
| Replay / checkpoints / memory | Stable | Recorder, checkpoint, rewind, transcript, SQLite facts, bounded `MEMORY.md` / `USER.md`, memory tool, and session search are wired with tests |
| First-run setup | Alpha | Conversational setup agent plus fallback interactive setup are wired for provider/model configuration |
| Telegram / Discord / Slack | Partial | Transport implementations exist; real account tokens are still required for end-to-end validation |
| Extra transports | Experimental | WhatsApp, Signal, Email, Feishu, WeCom, QQ, and Teams adapters are present but not all fully wired |
| Cloud sandboxes | Experimental | Modal, Daytona, Vercel, and Singularity entries are placeholders |
| Image / TTS / LSP | Experimental | Tool shells exist; provider/runtime integrations remain future work |
| Cross-platform release | Planned | Workflows exist; no public signed release artifact has been published yet |

See [docs/AUDIT.md](docs/AUDIT.md) for module-by-module proof.

## Console Experience

The WebView console mirrors Sparrow's brand demo instead of exposing raw runtime noise:

- the visible logo uses the exact canonical `sparrow-logo.html` pirate-builder mascot across GitHub, WebView, presentation, identity pages, and terminal-native ASCII fallback;
- local model failures are presented as `modèle local indisponible -> routage modèle cloud`;
- token and cost counters update live from the event stream;
- boot lines, route changes, tool activity, skill learning, swarm lanes, and the Sparrow mascot use the same motion language as the presentation HTML;
- the WebView composer supports `Cmd/Ctrl+K`, `@<agent>`, history, multiline input, paste/upload, drag-and-drop, and a live context bar;
- Captain and Paper themes are both shipped, persisted, and auto-selected from `prefers-color-scheme`;
- learned skills are pattern-based names such as `write-and-fix-tests`, not copied user prompts.

Skill learning is intentionally conservative. Sparrow only proposes reusable workflow patterns after concrete evidence such as tests, fixes, diffs, refactors, or code changes, and it skips repository-specific prompts, URLs, file names, dates, versions, and duplicate skill names.

## Quick Start From Source

```bash
git clone https://github.com/ucav/Sparrow.git
cd Sparrow
cargo build
cargo test --all-targets
```

Run the WebView console:

```bash
cargo run -- console
```

Open:

```text
http://127.0.0.1:9339/
```

Run a routing smoke test:

```bash
cargo run -- --json run "comment sélectionne tu le modèle le plus adapté lors du routing ?"
```

List detected providers and discovered models:

```bash
cargo run -- model --list
```

Prefer local Ollama:

```bash
cargo run -- --local run "summarize this repository"
```

Force a fast NVIDIA route:

```bash
cargo run -- --model nvidia:meta/llama-3.1-8b-instruct run "explain Sparrow routing"
```

Force an NVIDIA coding/reasoning route:

```bash
cargo run -- --model nvidia:deepseek-ai/deepseek-v4-flash run "explain Sparrow routing"
```

## First Configuration

Interactive setup:

```bash
cargo run -- setup
```

Useful environment variables:

```bash
NVIDIA_API_KEY=...
ANTHROPIC_API_KEY=...
OPENAI_API_KEY=...
GROQ_API_KEY=...
OPENROUTER_API_KEY=...
OLLAMA_HOST=http://127.0.0.1:11434
```

Configuration lives in the platform config directory, usually:

```text
%APPDATA%\sparrow\config.toml
```

Sparrow never needs API keys in the repository.

## Provider Routing Notes

Sparrow keeps a static provider registry and expands it with live model discovery when credentials are available. Stored credentials added with `sparrow auth add nvidia` are now used for discovery, so `sparrow model --list` can populate the NVIDIA catalog even when `NVIDIA_API_KEY` is not exported in the shell.

Current NVIDIA defaults are intentionally not a single Nemotron pin:

| Model | Use |
|---|---|
| `meta/llama-3.1-8b-instruct` | fast general chat and cheap smoke tests |
| `stepfun-ai/step-3.5-flash` | fast backup route validated through NVIDIA NIM |
| `nvidia/nemotron-3-super-120b-a12b` | stronger fallback for heavier tasks |

`sparrow model --set nvidia` resets an older pinned NVIDIA config back to this recommended chain. `sparrow --model nvidia:<model> run ...` now respects the explicit cloud route instead of putting local Ollama first for trivial prompts.

## Common Commands

```bash
sparrow setup
sparrow plan "inspect the repo and propose a safe implementation path"
sparrow console
sparrow run "fix the failing test"
sparrow --json run "summarize the repo"
sparrow chat
sparrow model --list
sparrow gateway start
sparrow gateway status
sparrow gateway stop
sparrow replay <run-id>
sparrow checkpoint list
sparrow rewind <checkpoint-id>
sparrow memory list
sparrow doctor
```

Slash commands can be declared as Markdown files in `.sparrow/commands/*.md` or
`%APPDATA%\sparrow\commands\*.md`. User-level commands override project and
built-in commands by name. Skills are also exposed as slash commands so reusable
workflows can become promptable control surfaces.

## Architecture

```text
                 user task
                    |
             routing need classifier
                    |
        budget-aware fallback model chain
                    |
                  Engine
        think -> tool -> observe -> emit
                    |
          one canonical Event stream
                    |
  CLI / TUI / WebView / JSON / Gateway / Recorder
```

Important contracts:

- [src/event.rs](src/event.rs) defines the event stream.
- [src/provider/mod.rs](src/provider/mod.rs) defines the `Brain` abstraction.
- [src/router/mod.rs](src/router/mod.rs) ranks models and fallbacks.
- [src/engine/mod.rs](src/engine/mod.rs) drives the agent loop.
- [src/tools/mod.rs](src/tools/mod.rs) defines tool contracts.
- [src/gateway/mod.rs](src/gateway/mod.rs) routes external messages.

## Docs

- [Module audit](docs/AUDIT.md)
- [V1 completion audit](docs/V1_COMPLETION_AUDIT.md)
- [Architecture](docs/architecture.md)
- [CLI reference](docs/cli-reference.md)
- [Configuration](docs/configuration.md)
- [Routing](docs/routing.md)
- [Autonomy](docs/autonomy.md)
- [Sandboxing](docs/sandboxing.md)
- [Replay](docs/replay.md)
- [Swarm](docs/swarm.md)
- [Migration guides](docs/migration/)
- [Brand assets](assets/brand/)

## What Makes Sparrow Different

Hermes Agent is excellent at presenting a personal agent loop and learning system. OpenClaw is excellent at operator-oriented docs and gateway/security guidance. Sparrow's angle should be narrower and sharper:

> a Rust-native local cockpit for routed agents, where every run is visible, replayable, budgeted, and checkpointed.

That is the experience this repository is converging toward.

## Contributing

Before opening a PR:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Keep docs honest: mark features as `Stable`, `Alpha`, `Partial`, `Experimental`, or `Planned` based on tests and runnable examples.

## License

MIT. See [LICENSE](LICENSE).
