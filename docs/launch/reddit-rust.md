# r/rust post

r/rust hates marketing. Lead with code, not story. Title is factual.

---

## Title

```
Sparrow — a single-binary Rust CLI agent that routes between 38 LLM providers, with git-backed checkpoints and hard budget caps
```

## Flair

`Project` (mandatory on r/rust)

## Body

```
github.com/ucav/Sparrow  ·  MIT  ·  edition 2024  ·  9 MB release binary

What it does

Sparrow is a CLI coding agent in the same family as Aider / Claude Code /
opencode, but written end-to-end in Rust. Every run is:

  - routed to the cheapest provider that can handle the task (Anthropic,
    OpenAI, Groq, NVIDIA, Gemini, Ollama, OpenRouter, DeepSeek, Mistral,
    xAI, Cerebras, …)
  - git-checkpointed before any mutating tool call, with `sparrow rewind`
    that restores files + conversation + token counter
  - capped at compile-time via `--max-cost-usd`, `--max-wall-secs`,
    `--max-tokens` — hard stops, not warnings
  - emitted as a single typed event stream consumed by three surfaces
    (TUI via ratatui, WebView cockpit, NDJSON via `--json`)

Architecture (the parts you actually want to read)

  - src/router/mod.rs       — the model picker, tier-based fallback chain
  - src/engine/mod.rs       — the agentic loop + tool dispatch
  - src/autonomy/mod.rs     — continuous autonomy dial w/ HardStop enum
  - src/runtime/event_bus.rs — typed channel feeding all three surfaces
  - src/provider/responses.rs / openai_compat.rs / anthropic.rs / ollama.rs
  - src/sandbox/             — bwrap on Linux, honest "unsupported" on
                                Windows/macOS rather than a silent stub

Why Rust specifically

  - One static binary. Boots in 18 ms. Nobody breaks the install by
    bumping a transitive Python or Node dep.
  - tokio for the event bus, axum for the cockpit HTTP/WS, ratatui for the
    TUI, sqlite-rs for persistent memory + FTS5 session search.
  - clippy -D warnings is enforced in CI. Tests cover the engine loop,
    routing fallback, autonomy gate, sandbox policy, memory persistence,
    skill registry, and the WebView contract (it's a versioned HTML file
    locked by an integration test).
  - Edition 2024. No unsafe outside the FFI shim into git2.

State

v0.5.3 released yesterday. 30+ integration tests. CI green on Ubuntu /
macOS / Windows. Documentation in `docs/` covers architecture, replay,
the autonomy contract, the sandbox, the gateway transports, and a docs
search built into `docs/index.html`.

Things I want to be told I'm wrong about

  1. The router heuristic. I use a static cost-tier table + capability
     hints, not a learned policy. Is that going to age badly?
  2. The autonomy dial. Three levels (supervised / trusted / autonomous)
     + HardStop variants. Should I add a fourth, or merge two?
  3. The sandbox story. bwrap on Linux is fine; on Windows/macOS I'd
     rather report "unsupported" than ship a fake sandbox. Is that the
     right call for the user base of this kind of tool?

Cargo

  cargo install sparrow-cli

Or pull the binary from the release page.

Happy to dig into any module.
```

---

## Reply protocol on r/rust

- If asked about clippy lints — show actual `clippy::*` allow list from
  `src/lib.rs` and explain each.
- If asked about async runtime — admit tokio, justify with the cockpit
  WS need; do not pretend it could be async-std.
- If asked about WASM — say "no, this needs sqlite, git, fs access, and
  exec; WASM would force an artificial split".
- If accused of being a Cargo.toml fan-club — link the actual diff of
  `src/engine/mod.rs` (2625 lines, hand-written).

## Things NOT to do on r/rust

- Do not call it "AI-powered". They will roast you.
- Do not mention the X thread, the HN post, or the comparison vs Claude
  Code. They consider it noise.
- Do not edit the title. Mods downvote post-edits.
