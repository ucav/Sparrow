<div align="center">

# 🐦 Sparrow

**A local-first Rust agent cockpit — route, run, replay, rewind.**

[![CI](https://github.com/ucav/Sparrow/actions/workflows/ci.yml/badge.svg)](https://github.com/ucav/Sparrow/actions/workflows/ci.yml)
[![Security Audit](https://github.com/ucav/Sparrow/actions/workflows/audit.yml/badge.svg)](https://github.com/ucav/Sparrow/actions/workflows/audit.yml)
[![Release](https://img.shields.io/github/v/release/ucav/Sparrow?color=blue&label=release)](https://github.com/ucav/Sparrow/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-green)](LICENSE)
[![Rust 1.96+](https://img.shields.io/badge/rust-1.96%2B-orange)](https://rust-lang.org)
[![crates.io](https://img.shields.io/crates/v/sparrow-cli?color=orange)](https://crates.io/crates/sparrow-cli)
[![docs.rs](https://img.shields.io/docsrs/sparrow-cli)](https://docs.rs/sparrow-cli)
[![Linux](https://img.shields.io/badge/linux-x86__64%20%C2%B7%20aarch64-2dba4e?logo=linux&logoColor=white)](https://github.com/ucav/Sparrow/releases/latest)
[![macOS](https://img.shields.io/badge/macOS-arm64%20%C2%B7%20x86__64-c1c1c1?logo=apple&logoColor=white)](https://github.com/ucav/Sparrow/releases/latest)
[![Windows](https://img.shields.io/badge/windows-x86__64-0078d4?logo=windows&logoColor=white)](https://github.com/ucav/Sparrow/releases/latest)
[![Downloads](https://img.shields.io/crates/d/sparrow-cli?color=blueviolet&label=crates.io%20downloads)](https://crates.io/crates/sparrow-cli)
[![GitHub stars](https://img.shields.io/github/stars/ucav/Sparrow?style=flat&color=f2a93c)](https://github.com/ucav/Sparrow/stargazers)

<img src="assets/brand/sparrow-mascot.svg" width="140" alt="Sparrow mascot" />

*One event stream. Terminal UI, WebView cockpit, JSON output, or gateway — your choice.*

[![asciicast](https://asciinema.org/a/PLEprnQSdv4lqOH2.svg)](https://asciinema.org/a/PLEprnQSdv4lqOH2)


[Quick Start](#quick-start) · [Why Sparrow](#why-sparrow-vs-claude-code--codex--aider) · [Commands](#common-commands) · [Architecture](#architecture) · [Docs](#docs) · [Releases](https://github.com/ucav/Sparrow/releases)

</div>

---

Sparrow is a single-binary CLI agent written in Rust. It routes each task to the **cheapest capable model**, keeps you in control with **Git-backed checkpoints**, and makes every run **replayable**. Local models (Ollama) are always the first hop; cloud providers are explicit fallbacks.

The project focuses on a narrow product promise: a Rust-native local cockpit where every run is **visible, replayable, budgeted, and checkpointed**.

---

## Why Sparrow vs Claude Code / Codex / Aider

| Capability | Claude Code | OpenAI Codex CLI | Aider | **Sparrow** |
|---|:---:|:---:|:---:|:---:|
| Single static binary, no Node/Python runtime | ❌ | ❌ | ❌ | ✅ |
| Choose any provider, any model | ❌ Anthropic | ❌ OpenAI | ✅ | ✅ **38 providers** |
| Local-first (Ollama OOTB) | ❌ | ❌ | ⚠️ | ✅ |
| Git checkpoints + `rewind` per run | ❌ | ❌ | ⚠️ | ✅ |
| Budget caps (`--max-cost-usd` / `--max-wall-secs`) | ❌ | ❌ | ❌ | ✅ |
| WebView cockpit + TUI + JSON stream | ⚠️ TUI only | ⚠️ TUI only | ⚠️ TUI only | ✅ all three |
| MCP server **host + client** | ✅ | ⚠️ | ❌ | ✅ |
| Drop-in import of `~/.claude/` config | n/a | ❌ | ❌ | ✅ |
| Multi-agent swarm (Planner → Coder → Verifier) | ❌ | ❌ | ❌ | ✅ |
| Telegram / Discord / Slack gateways | ❌ | ❌ | ❌ | ✅ |
| Pre-commit secret scanner bundled | ❌ | ❌ | ❌ | ✅ |
| Voice (`speak`, `transcribe`) | ❌ | ❌ | ❌ | ✅ |
| Replay & share session as URL/Gist | ❌ | ❌ | ❌ | ✅ |
| Source open, MIT | ⚠️ closed | ⚠️ closed | ✅ | ✅ |
| Zero telemetry by default | ⚠️ | ⚠️ | ✅ | ✅ |

See [`docs/comparison/vs-competitors.md`](docs/comparison/vs-competitors.md) for the long form (incl. OpenCode, Hermes, Continue, Cursor).

---

## ✨ What's New — v0.5.2

> **Unified release** — v0.4.0 public launch + v0.5.0/v0.5.1 Phases 1→5 + v0.3.6 distribution pass + Tier2 autonomy hardening, all merged upward.

**Distribution & install**
- **`cargo install sparrow-cli`** — published on crates.io.
- **`sparrow launch`** — one-click first-run with WebView cockpit (`--tui` for terminal).
- **One-click installers** for Windows, macOS, Linux.

**Experience**
- **CLI rich rendering** (Phase 1) — markdown, code, diff, JSON, tables.
- **Streaming + chat composer** (Phase 2) — live progress, persistent sessions.
- **TTS / STT / `file_search`** (Phase 3) — 30 new bundled skills.
- **Memory CLI + FTS5 session search** (Phase 4).
- **`sparrow voice {speak,transcribe,providers}`** (Phase 5).
- **`sparrow demo`** — 30-second self-contained Planner→Coder→Verifier demo.
- **`sparrow share`** — session → GitHub Gist in one command.
- **First-run wizard** — auto-detects 20+ provider keys, ranks by cost tier.

**Safety**
- **Tier2 autonomy** — encrypted credential fallback store, honest hardened-sandbox reporting.
- **`sparrow hook install`** — pre-commit secret scanner.
- **Knowledge graph** — typed SQLite nodes/edges with optional Neo4j sync.
- **Playwright e2e** — real browser/computer-use regression suite.
- **Humanized French errors** for HTTP 401/429/connection/config failures.

---

## Why Explore It

| | |
|---|---|
| **Model routing** | Budget-aware fallback chains across Ollama, NVIDIA, Anthropic, OpenAI-compatible APIs, and 30+ registry entries |
| **WebView cockpit** | Live route/token/cost/context at `http://127.0.0.1:9339/` with drawer panels, slash palette, and agent picker |
| **Terminal-native** | Animated TUI cockpit, `sparrow run`, `sparrow chat`, `--json` output, replay, memory, gateway |
| **Rollback safety** | Auto-checkpoint before any mutating action; `sparrow rewind <id>` to restore |
| **Persistent context** | SQLite facts + knowledge graph, SOUL-style `.agent.md` files, guarded skill registry, full transcripts |
| **Browser/computer-use** | Playwright-backed browser tool and gated screenshot/click/type computer primitive |
| **Gateway** | Telegram, Discord, Slack, WebSocket API — wired with honest errors, not silent failures |

<br>
<p align="center">
  <img src="docs/screenshots/webview-captain.png" alt="Sparrow WebView cockpit" width="90%">
  <br><em>Sparrow WebView cockpit — Captain theme</em>
</p>

---

## Status

Sparrow is **public beta** with a green cross-platform CI baseline. The kernel, routing core, console surfaces, replay, checkpoints, and memory are wired and tested; external transports are being validated by early adopters.

<details>
<summary><strong>Full status table</strong> (click to expand)</summary>

| Area | Status | Evidence |
|---|:---:|---|
| CI / Rust build | ✅ Stable | Ubuntu · macOS · Windows; `fmt`, `clippy -D warnings`, `check`, release builds |
| Test suite | ✅ Stable | Full `cargo test` green on current master |
| Security audit | ✅ Stable | `rustsec/audit-check` on all three platforms |
| Engine loop | ✅ Stable | Event stream, task classification, fallback execution, auto-checkpoint, auto-compaction |
| WebView console | ✅ Stable | Full cockpit — rail/drawer, typed event stream, compact highlighted code cards, themes, composer, approval modal |
| TUI cockpit | ✅ Stable | Animated cockpit, swarm lanes, checkpoint/diff/cost panels, `@` picker, history |
| Plan mode / slash | ✅ Stable | `sparrow plan`, `/plan`, built-in commands, user/project Markdown discovery |
| Permissions / hooks | ✅ Stable | 6 permission modes; `Pre`/`Post` lifecycle hooks for run/tool/checkpoint/compact |
| Declarative agents | ✅ Stable | SOUL TOML + Markdown frontmatter; `agent run`, `agent mention`, CRUD |
| Skills / plugins | ✅ Stable | Progressive references + templates; plugin manifests; CLI install/list/remove |
| Toolsets | ✅ Stable | Toolset/risk/auth/mutation/network/exec metadata; surface filtering |
| Browser / computer-use | 🔶 Alpha | Playwright driver, screenshot blocks, click/type, Linux `bwrap` wrapper when available |
| Security audit CLI | ✅ Stable | `sparrow security audit [--json]`, WebView `/security` |
| Sandbox policy | ✅ Stable | Protected paths, env allowlist; Docker/SSH/Worktree backends; honest vendor errors |
| Media tools | ✅ Stable | `vision`, `image_generate`, `text_to_speech`, `transcribe`; WebView upload/artifacts |
| GitHub Action | ✅ Stable | `action.yml`, sample workflow, `sparrow github review/status/logs`, `--dry-run` |
| Context / compaction | ✅ Stable | `ContextMeter`, engine auto-trigger at 120k chars, durable `HandoffDoc` |
| Gateway | ✅ Stable | `/status` roundtrip on port 9338; run registry with real abort |
| Replay / memory | ✅ Stable | Recorder, checkpoint, rewind, SQLite facts, knowledge graph, optional Neo4j sync, bounded `MEMORY.md`, session search |
| Provider routing | 🔶 Alpha | Ollama + NVIDIA tested locally; 92 NVIDIA models discovered |
| First-run setup | 🔶 Alpha | Conversational setup agent + interactive fallback |
| Telegram / Discord / Slack | 🔸 Partial | Transport implementations exist; E2E token validation pending |
| Extra transports | 🧪 Experimental | WhatsApp, Signal, Email, Feishu, WeCom, QQ, Teams adapters present |
| Cloud sandboxes | 🧪 Experimental | Modal, Daytona, Vercel, Singularity — placeholder entries |
| Cross-platform release | ✅ Stable | Linux · macOS · Windows pre-built binaries on every tag |

</details>

See [docs/AUDIT.md](docs/AUDIT.md) for module-by-module proof.

---

## Quick Start

**Pick any of these — they all give you the same `sparrow` binary:**

```bash
# Universal (Rust toolchain)
cargo install sparrow-cli

# macOS / Linux — one-liner
curl -fsSL https://raw.githubusercontent.com/ucav/Sparrow/master/install.sh | sh

# macOS — Homebrew
brew install ucav/tap/sparrow

# Windows — PowerShell one-liner
irm https://raw.githubusercontent.com/ucav/Sparrow/master/install.ps1 | iex

# Windows — Scoop
scoop bucket add ucav https://github.com/ucav/scoop-bucket
scoop install sparrow

# Windows 11 — winget
winget install ucav.Sparrow
```

**Then:**

```bash
sparrow launch       # first-run picks a free provider, opens cockpit
sparrow run "explain this repo and write TODO.md"
```

That's the [60-second tour](docs/getting-started.md). No API key required —
the first-run wizard offers a free provider (Groq / Gemini / NVIDIA) or local
Ollama (auto-installed if missing).

**Launch Sparrow:**

```bash
sparrow launch
```

`sparrow launch` runs first-launch setup when needed, then opens the WebView cockpit on
`http://127.0.0.1:9339/`. Use `sparrow launch --tui` for the terminal cockpit.

**Build from source:**

```bash
git clone https://github.com/ucav/Sparrow.git
cd Sparrow
cargo build
cargo test --all-targets
```

**Run the WebView cockpit:**

```bash
cargo run -- launch
# → open http://127.0.0.1:9339/
```

**Routing smoke test:**

```bash
cargo run -- --json run "how does Sparrow choose the best model?"
```

**List detected providers and models:**

```bash
cargo run -- model --list
```

**Force a specific route:**

```bash
# Local Ollama first
cargo run -- --local run "summarize this repo"

# Explicit NVIDIA route
cargo run -- --model nvidia:meta/llama-3.1-8b-instruct run "explain routing"

# Coding / reasoning route
cargo run -- --model nvidia:deepseek-ai/deepseek-v4-flash run "refactor this function"
```

---

## First Configuration

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

Config lives in the platform config directory (e.g. `%APPDATA%\sparrow\config.toml` on Windows). Sparrow never needs API keys in the repository.

---

## Provider Routing

Sparrow keeps a static provider registry and expands it with live model discovery when credentials are available. Stored credentials added with `sparrow auth add nvidia` are used for discovery, so `sparrow model --list` can populate the NVIDIA catalog even when `NVIDIA_API_KEY` is not exported.

**Default NVIDIA chain:**

| Model | Use case |
|---|---|
| `meta/llama-3.1-8b-instruct` | Fast general chat and cheap smoke tests |
| `stepfun-ai/step-3.5-flash` | Fast backup route via NVIDIA NIM |
| `nvidia/nemotron-3-super-120b-a12b` | Stronger fallback for heavier tasks |

`sparrow model --set nvidia` resets an older pinned config back to this chain.

---

## Common Commands

```bash
sparrow setup                      # first-run configuration
sparrow plan "propose an approach" # read-only plan mode
sparrow console                    # launch WebView cockpit
sparrow run "fix the failing test"
sparrow --json run "summarize"     # NDJSON output for CI/hooks
sparrow chat                       # interactive session
sparrow model --list               # discovered providers & models
sparrow gateway start              # start gateway (Telegram/Discord/WS)
sparrow gateway status
sparrow gateway stop
sparrow replay <run-id>            # replay a past run
sparrow checkpoint list
sparrow rewind <checkpoint-id>     # restore workspace
sparrow memory list
sparrow memory graph search routing
sparrow security audit
sparrow doctor
```

Custom slash commands can be declared as Markdown files in `.sparrow/commands/*.md` or `%APPDATA%\sparrow\commands\*.md`. User-level commands override project and built-in ones by name. Skills are also exposed as slash commands.

---

## Architecture

```
              user task
                  │
       routing-need classifier
                  │
      budget-aware fallback chain
                  │
                Engine
      think → tool → observe → emit
                  │
       ┌──────────┼───────────┐
      CLI        TUI       WebView
      JSON     Gateway    Recorder
```

**Load-bearing contracts:**

| File | Role |
|---|---|
| [`src/event.rs`](src/event.rs) | Canonical event stream |
| [`src/provider/mod.rs`](src/provider/mod.rs) | `Brain` abstraction |
| [`src/router/mod.rs`](src/router/mod.rs) | Model ranking and fallbacks |
| [`src/engine/mod.rs`](src/engine/mod.rs) | Agent loop |
| [`src/tools/mod.rs`](src/tools/mod.rs) | Tool contracts |
| [`src/gateway/mod.rs`](src/gateway/mod.rs) | External message routing |

---

## Docs

| Document | Topic |
|---|---|
| [docs/AUDIT.md](docs/AUDIT.md) | Module-by-module proof |
| [docs/architecture.md](docs/architecture.md) | System architecture |
| [docs/cli-reference.md](docs/cli-reference.md) | Full CLI reference |
| [docs/routing.md](docs/routing.md) | Routing and provider chains |
| [docs/autonomy.md](docs/autonomy.md) | Permission modes and hooks |
| [docs/sandboxing.md](docs/sandboxing.md) | Sandbox policy and backends |
| [docs/browser-computer.md](docs/browser-computer.md) | Playwright browser and computer-use tools |
| [docs/replay.md](docs/replay.md) | Replay and checkpoints |
| [docs/swarm.md](docs/swarm.md) | Multi-agent swarm |
| [docs/keyboard.md](docs/keyboard.md) | Keyboard shortcuts |
| [docs/configuration.md](docs/configuration.md) | Configuration reference |
| [assets/brand/](assets/brand/) | Brand assets (SVG, HTML, ASCII) |

---

## Contributing

Before opening a PR:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Keep docs honest: mark features as `Stable`, `Alpha`, `Partial`, `Experimental`, or `Planned` based on tests and runnable examples. See [CONTRIBUTING.md](CONTRIBUTING.md).

---

## License

MIT — see [LICENSE](LICENSE).
