<div align="center">

# 🐦 Sparrow

**A local-first Rust agent cockpit — route, run, replay, rewind.**

[![CI](https://github.com/ucav/Sparrow/actions/workflows/ci.yml/badge.svg)](https://github.com/ucav/Sparrow/actions/workflows/ci.yml)
[![Security Audit](https://github.com/ucav/Sparrow/actions/workflows/audit.yml/badge.svg)](https://github.com/ucav/Sparrow/actions/workflows/audit.yml)
[![Release](https://img.shields.io/github/v/release/ucav/Sparrow?color=blue&label=release)](https://github.com/ucav/Sparrow/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-green)](LICENSE)
[![Rust 1.96+](https://img.shields.io/badge/rust-1.96%2B-orange)](https://rust-lang.org)

<img src="assets/brand/sparrow-mascot.svg" width="140" alt="Sparrow mascot" />

*One event stream. Terminal UI, WebView cockpit, JSON output, or gateway — your choice.*

[Quick Start](#quick-start) · [Commands](#common-commands) · [Architecture](#architecture) · [Docs](#docs) · [Releases](https://github.com/ucav/Sparrow/releases)

</div>

---

Sparrow is a single-binary CLI agent written in Rust. It routes each task to the **cheapest capable model**, keeps you in control with **Git-backed checkpoints**, and makes every run **replayable**. Local models (Ollama) are always the first hop; cloud providers are explicit fallbacks.

> Inspired by Claude Code, Codex, OpenCode, OpenClaw and Hermes Agent — but intentionally narrower: a Rust-native local cockpit where every run is **visible, replayable, budgeted, and checkpointed**.

---

<div align="center">
<sub>⚡ Powered by &amp; created with</sub>

<br/><br/>

<table>
  <tr>
    <td align="center" width="20%">
      <a href="https://www.nvidia.com/en-us/ai/">
        <picture>
          <source media="(prefers-color-scheme: dark)" srcset="https://cdn.simpleicons.org/nvidia/76B900">
          <img src="https://cdn.simpleicons.org/nvidia/555555" height="40" alt="NVIDIA">
        </picture>
        <br/><sub><b>NVIDIA NIM</b></sub>
      </a>
    </td>
    <td align="center" width="20%">
      <a href="https://openai.com/">
        <picture>
          <source media="(prefers-color-scheme: dark)" srcset="https://cdn.simpleicons.org/openai/ffffff">
          <img src="https://cdn.simpleicons.org/openai/000000" height="40" alt="OpenAI">
        </picture>
        <br/><sub><b>OpenAI</b></sub>
      </a>
    </td>
    <td align="center" width="20%">
      <a href="https://openai.com/codex">
        <picture>
          <source media="(prefers-color-scheme: dark)" srcset="https://cdn.simpleicons.org/openai/a78bfa">
          <img src="https://cdn.simpleicons.org/openai/412991" height="40" alt="Codex">
        </picture>
        <br/><sub><b>Codex</b></sub>
      </a>
    </td>
    <td align="center" width="20%">
      <a href="https://anthropic.com/">
        <picture>
          <source media="(prefers-color-scheme: dark)" srcset="https://cdn.simpleicons.org/anthropic/d4a574">
          <img src="https://cdn.simpleicons.org/anthropic/191919" height="40" alt="Anthropic Claude">
        </picture>
        <br/><sub><b>Claude</b></sub>
      </a>
    </td>
    <td align="center" width="20%">
      <a href="https://github.com/features/copilot">
        <picture>
          <source media="(prefers-color-scheme: dark)" srcset="https://cdn.simpleicons.org/githubcopilot/ffffff">
          <img src="https://cdn.simpleicons.org/githubcopilot/000000" height="40" alt="GitHub Copilot">
        </picture>
        <br/><sub><b>GitHub Copilot</b></sub>
      </a>
    </td>
  </tr>
</table>

</div>

---

## ✨ What's New — v0.3.0

> **WebView Cockpit** — the console is now a real local control surface, not a mockup.

- **3-column layout**: icon rail · sliding drawer · live event stream
- **Typed event cards**: tool calls, diffs, checkpoints, compaction, route changes, streaming text
- **Full composer**: `Cmd/Ctrl+K` slash palette, `@<agent>` picker, history, multiline, drag-and-drop upload
- **Approval modal** wired to `POST /approval`
- **Captain & Paper themes** — both tested, persisted, auto-selected from `prefers-color-scheme`
- **Reduced-motion** fallback disables animations and the boot overlay

---

## Why Explore It

| | |
|---|---|
| **Model routing** | Budget-aware fallback chains across Ollama, NVIDIA, Anthropic, OpenAI-compatible APIs, and 30+ registry entries |
| **WebView cockpit** | Live route/token/cost/context at `http://127.0.0.1:9339/` with drawer panels, slash palette, and agent picker |
| **Terminal-native** | Animated TUI cockpit, `sparrow run`, `sparrow chat`, `--json` output, replay, memory, gateway |
| **Rollback safety** | Auto-checkpoint before any mutating action; `sparrow rewind <id>` to restore |
| **Persistent context** | SQLite memory, SOUL-style `.agent.md` files, guarded skill registry, full transcripts |
| **Gateway** | Telegram, Discord, Slack, WebSocket API — wired with honest errors, not silent failures |

---

## Status

Sparrow is **alpha software** with a green cross-platform CI baseline. The kernel, routing core, console surfaces, replay, checkpoints, and memory are wired and tested; external transports and release packaging still need real-world validation.

<details>
<summary><strong>Full status table</strong> (click to expand)</summary>

| Area | Status | Evidence |
|---|:---:|---|
| CI / Rust build | ✅ Stable | Ubuntu · macOS · Windows; `fmt`, `clippy -D warnings`, `check`, release builds |
| Test suite | ✅ Stable | 109 tests pass (`cargo test`), including 95 integration tests |
| Security audit | ✅ Stable | `rustsec/audit-check` on all three platforms |
| Engine loop | ✅ Stable | Event stream, task classification, fallback execution, auto-checkpoint, auto-compaction |
| WebView console | ✅ Stable | Full cockpit — rail/drawer, typed event stream, themes, composer, approval modal |
| TUI cockpit | ✅ Stable | Animated cockpit, swarm lanes, checkpoint/diff/cost panels, `@` picker, history |
| Plan mode / slash | ✅ Stable | `sparrow plan`, `/plan`, built-in commands, user/project Markdown discovery |
| Permissions / hooks | ✅ Stable | 6 permission modes; `Pre`/`Post` lifecycle hooks for run/tool/checkpoint/compact |
| Declarative agents | ✅ Stable | SOUL TOML + Markdown frontmatter; `agent run`, `agent mention`, CRUD |
| Skills / plugins | ✅ Stable | Progressive references + templates; plugin manifests; CLI install/list/remove |
| Toolsets | ✅ Stable | Toolset/risk/auth/mutation/network/exec metadata; surface filtering |
| Security audit CLI | ✅ Stable | `sparrow security audit [--json]`, WebView `/security` |
| Sandbox policy | ✅ Stable | Protected paths, env allowlist; Docker/SSH/Worktree backends; honest vendor errors |
| Media tools | ✅ Stable | `vision`, `image_generate`, `text_to_speech`, `transcribe`; WebView upload/artifacts |
| GitHub Action | ✅ Stable | `action.yml`, sample workflow, `sparrow github review/status/logs`, `--dry-run` |
| Context / compaction | ✅ Stable | `ContextMeter`, engine auto-trigger at 120k chars, durable `HandoffDoc` |
| Gateway | ✅ Stable | `/status` roundtrip on port 9338; run registry with real abort |
| Replay / memory | ✅ Stable | Recorder, checkpoint, rewind, SQLite facts, bounded `MEMORY.md`, session search |
| Provider routing | 🔶 Alpha | Ollama + NVIDIA tested locally; 92 NVIDIA models discovered |
| First-run setup | 🔶 Alpha | Conversational setup agent + interactive fallback |
| Telegram / Discord / Slack | 🔸 Partial | Transport implementations exist; E2E token validation pending |
| Extra transports | 🧪 Experimental | WhatsApp, Signal, Email, Feishu, WeCom, QQ, Teams adapters present |
| Cloud sandboxes | 🧪 Experimental | Modal, Daytona, Vercel, Singularity — placeholder entries |
| Cross-platform release | 📋 Planned | Workflows exist; signed release artifacts not yet published |

</details>

See [docs/AUDIT.md](docs/AUDIT.md) for module-by-module proof.

---

## Quick Start

```bash
git clone https://github.com/ucav/Sparrow.git
cd Sparrow
cargo build
cargo test --all-targets
```

**Run the WebView cockpit:**

```bash
cargo run -- console
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
