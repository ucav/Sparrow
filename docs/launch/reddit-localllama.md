# r/LocalLLaMA post

r/LocalLLaMA wants: offline, Ollama, privacy, no API key, no telemetry,
no cloud server. Lead with that.

---

## Title

```
Sparrow — a local-first coding agent that runs full tasks on Ollama with $0.00 spent and no telemetry
```

## Body

```
Hey r/LocalLLaMA — I built a CLI agent in Rust that treats Ollama as the
*default* provider, not a fallback. Wanted to share because most coding
agents in the wild still gate the killer features behind a cloud API.

The local story

  - First run: if no API keys are detected, the wizard offers to install
    Ollama if missing and pull a default model (qwen2.5-coder-7b out of
    the box). After that, every task can run at $0.00.
  - Even with cloud keys configured, Ollama is tried first when capability
    matches. The router is honest about when a local model can't do it
    and explains the fallback in the event stream.
  - All session data — transcripts, knowledge graph, FTS5 search index,
    encrypted credential store — stays in ~/.sparrow/ on your machine.
  - PRIVACY.md is explicit: no telemetry by default, no usage ping, no
    error beacon. You can `tcpdump -i any port 443` the binary on first
    run and see for yourself.

What you get vs running Ollama bare

  - Multi-step agentic loop with tool use (read, edit, exec, git, search,
    memory, knowledge graph) — not just a chat wrapper.
  - Git-backed checkpoints before every file change + `sparrow rewind` to
    restore in one command.
  - Persistent memory in SQLite (facts table + typed knowledge graph
    nodes/edges, FTS5 search over every past session).
  - TUI cockpit, WebView cockpit, or pure NDJSON output — same event bus.
  - Cap your run with --max-cost-usd / --max-wall-secs / --max-tokens
    (even local, --max-wall-secs is useful when a 7B starts looping).
  - Voice in/out via `sparrow voice {speak,transcribe}` — wired for local
    whisper.cpp and piper.

What you can do today, fully offline

  $ sparrow run "scaffold a CLI in Rust that reads /etc/hosts and \
                 redacts private IPs" --local

  $ sparrow chat --model qwen2.5-coder-32b-instruct

  $ sparrow plan "refactor the auth module" --json | jq .

Repo: github.com/ucav/Sparrow  (MIT)
Install: cargo install sparrow-cli

Honest caveats

  - Ollama with anything under 14B will sometimes ask for help on
    multi-file refactors. Plan mode (`sparrow plan`) is the workaround.
  - Browser/computer-use tools spawn Playwright; that's heavy. Disabled
    if you don't enable the `browser` feature.
  - I am ONE person. If the local-first wiring breaks on your machine,
    open an issue and I will reply same day.

What I want to hear from you

  - Which local models you use for code today and how Sparrow compares.
  - Whether you'd want me to wire llamafile / vllm / lmstudio as
    additional local backends (Ollama is currently the only one).
  - Whether the FTS5 session search (search across every past task you
    ran) is useful or noise.
```

---

## Tone

This audience is **the most credulity-resistant of all**. Do not oversell.
Do not call it a "framework". Do not say "10x". Pre-empt critique by
naming the model-size limit yourself.

## Reply protocol

- "Does it work with Vulkan/ROCm?" — yes, via Ollama. Sparrow doesn't
  touch the GPU directly.
- "Is the FTS5 index encrypted at rest?" — no. Plan in CHANGELOG. Be
  explicit.
- "How does it pick which local model?" — link `src/router/mod.rs` and
  let them read.
