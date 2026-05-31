# Sparrow

**A local-first Rust agent cockpit for model routing, WebView control, rollback safety, and transparent cost.**

[![CI](https://github.com/ucav/Sparrow/actions/workflows/ci.yml/badge.svg)](https://github.com/ucav/Sparrow/actions/workflows/ci.yml)
[![Security Audit](https://github.com/ucav/Sparrow/actions/workflows/audit.yml/badge.svg)](https://github.com/ucav/Sparrow/actions/workflows/audit.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-green)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.96%2B-orange)](https://rust-lang.org)

<p>
  <img alt="Sparrow mascot" src="assets/brand/sparrow-mascot.svg" width="128">
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

Sparrow is **alpha software**. The kernel builds and has a real integration suite, but several ambitious surfaces remain partial or experimental.

| Area | Status | Evidence |
|---|---:|---|
| Rust build | Stable | `cargo check`, `cargo build` pass locally |
| Test suite | Stable | 84 tests pass with `cargo test --all-targets` |
| Engine loop | Alpha | `src/engine/mod.rs`, integration tests, JSON smoke test |
| Provider routing | Alpha | Ollama + NVIDIA auto-discovery tested locally |
| WebView console | Alpha | HTTP + WebSocket console tested on port 9339 |
| Gateway WebSocket | Alpha | `/status` command roundtrip tested on port 9338 |
| Telegram/Discord/Slack | Partial | Transport implementations exist; real account tokens required for end-to-end validation |
| Extra transports | Experimental | WhatsApp/Signal/Email/Feishu/WeCom/QQ/Teams adapters are present but not all fully wired |
| Cloud sandboxes | Experimental | Modal/Daytona/Vercel/Singularity are placeholders |
| Image/TTS/LSP | Experimental | Tool shells exist; provider/runtime integration remains future work |
| Cross-platform release | Planned | workflows exist; no public release artifact yet |

See [docs/AUDIT.md](docs/AUDIT.md) for module-by-module proof.

## Console Experience

The WebView console mirrors Sparrow's brand demo instead of exposing raw runtime noise:

- local model failures are presented as `modèle local indisponible -> routage modèle cloud`;
- token and cost counters update live from the event stream;
- boot lines, route changes, tool activity, skill learning, and the Sparrow mascot use the same motion language as the presentation HTML;
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

List detected providers:

```bash
cargo run -- model --list
```

Prefer local Ollama:

```bash
cargo run -- --local run "summarize this repository"
```

Force a provider/model route:

```bash
cargo run -- --model nvidia:nvidia/nemotron-3-super-120b-a12b run "explain Sparrow routing"
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

## Common Commands

```bash
sparrow setup
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
