# Sparrow press kit

Copy ready to paste. Pick the length that matches the slot.

---

## One-liner (≤ 90 chars)

```
Sparrow — one Rust binary that runs any LLM provider, checkpoints to git, caps the bill.
```

## Tagline (≤ 60 chars)

```
The local-first coding agent that gives you back the bill.
```

---

## 30-second elevator

```
Sparrow is a single-binary command-line coding agent, written in Rust,
that routes each task to the cheapest LLM provider that can handle it.
It auto-checkpoints to git before every mutating step, so one command
undoes a whole run — files, conversation, token counter. Hard budget
caps stop the run when you say so, not when the bill arrives. Local
Ollama is the default first hop, so $0.00 runs are normal. MIT.
```

---

## 2-minute pitch

```
Sparrow is a coding agent in the same family as Claude Code, OpenAI
Codex CLI, Aider, or opencode, but built around three calls those
tools don't make:

First, no surface owns the run. There is one typed event bus and three
independent subscribers — a TUI cockpit, a WebView cockpit on port
9339, and a --json NDJSON pipeline. If the TUI crashes, the WebView
keeps streaming. If the WebView panics, the JSON keeps writing. The
run survives any single surface dying.

Second, checkpoints are at the tool boundary, not the message
boundary. Every mutating tool call — every edit, every exec, every
git op — goes through an autonomy gate that snapshots the workspace
before the call. `sparrow rewind --last` atomically restores files,
conversation, and the token counter as a unit. Not just files. The
whole run state.

Third, hard caps are enforced in the runtime, not warned about in a
dashboard. --max-cost-usd, --max-wall-secs, --max-tokens are real
stops. Hit one, the run aborts at the last checkpoint and prints a
receipt. No config trick can bypass them.

It's one static binary, 9 MB, boots in 18 ms. cargo install
sparrow-cli, brew install ucav/tap/sparrow, scoop install sparrow, or
the one-line curl install on Linux/macOS. 38 providers wired,
including local Ollama as the default first hop. Drop-in reader for
~/.claude/{CLAUDE.md, commands/, agents/, settings.json} so migration
off Claude Code is zero effort.

MIT licensed. Zero telemetry by default. One developer maintains it.
```

---

## 5-minute pitch (use for podcasts, conference Q&A)

```
The story starts on a Tuesday evening. I had been paying for Claude
Code since the beta. I'd just put my kids to sleep. I open the
Anthropic billing dashboard and see $847 spent in four days on eleven
modified files. Three of those four days had ended with a TUI crash
that took my session with it — Claude Code's TUI binary owns the run,
and when it dies, the conversation context dies with it. So I'd been
paying the assistant to relearn my codebase, then paying it to relearn
it again, then paying a third time after the next crash.

I closed the laptop. I opened a Rust project. Fourteen days later
Sparrow was at v0.3 and I started using it for my own work. Today
it's at v0.5.3, public beta, 30+ integration tests, CI green on three
operating systems.

The architectural calls that made it different from Claude Code came
out of those fourteen days of frustration:

The first call: the event bus is the source of truth, not the TUI.
Sparrow uses one tokio broadcast channel emitting a typed Event enum.
Three surfaces — a ratatui TUI, an axum WebView cockpit, a --json
NDJSON pipeline — subscribe as peers. None can corrupt the others.
Crash the TUI, the WebView keeps streaming. Crash the WebView, JSON
keeps writing. The runtime daemon persists missed events via a
recorder so a restart picks up where it left off.

The second call: checkpoints sit at the tool boundary. A single
agent step often calls four tools in sequence. If the exec at step
four corrupts something, you want a snapshot between every edit, not
before "the last assistant reply". So every mutating tool call goes
through an autonomy gate that snapshots before the call. `sparrow
rewind --last` atomically restores files, conversation, and token
counter.

The third call: hard budget caps are enforced in the engine loop,
not in a config file. Three flags — --max-cost-usd, --max-wall-secs,
--max-tokens — and a HardStop variant in the autonomy contract.
Hitting one terminates at the next safe boundary regardless of what
the model is doing.

Beyond those three, the things that ended up mattering more than I
expected: a drop-in reader for ~/.claude/{CLAUDE.md, commands/,
agents/, settings.json} that makes migration from Claude Code
effectively free; a pre-commit secret scanner bundled in the binary;
Ollama as the default first hop in the router so $0.00 runs are the
normal case; a knowledge graph in SQLite with FTS5 search across every
past session; voice in and out via local whisper.cpp and piper.

What I'd want anyone interested to know: I am one developer. The
project is MIT licensed. There is no SaaS, no cloud, no telemetry.
The binary stays on your machine. The repo is github.com/ucav/Sparrow.
If you find a bug today I'll ship the fix tonight.
```

---

## Maker bio (≤ 240 chars)

```
Abdou — solo developer, two kids, post-burnout. Built Sparrow in Rust
after a $847 four-day Claude Code spend convinced me coding agents
should be local-first, single-binary, and rewindable. 61 followers
and counting.
```

---

## Long bio (≤ 600 chars)

```
Abdou is a solo developer based in France who builds developer
tooling. After burning out on three failed SaaS attempts and a $847
four-day spend on Claude Code, he started Sparrow — a local-first
Rust agent cockpit that routes between 38 LLM providers, snapshots
to git before every mutating step, and enforces hard budget caps in
the runtime instead of warning about them in a dashboard.

Sparrow is MIT, single binary, 9 MB, with zero telemetry by default.
The full source is at github.com/ucav/Sparrow.

Reach: github.com/ucav · twitter.com/<your-handle>
```

---

## Logos and images

- Mascot SVG: `assets/brand/sparrow-mascot.svg`
- OG card: `assets/launch/og-card.svg` (export to PNG 1200×630)
- Hook receipt: `assets/launch/x-hook-receipt.svg`
- Comparison card: `assets/launch/comparison-card.svg`
- CLI demo still: `assets/launch/cli-demo-card.svg`
- Install card: `assets/launch/install-card.svg`
- Cockpit screenshot: `docs/screenshots/webview-captain.png`

## Boilerplate "About" paragraph (for press releases, podcast intros)

```
Sparrow is an open-source command-line coding agent written in Rust.
It runs any LLM provider locally, snapshots every change to git, and
enforces hard budget caps in the runtime. The full source is at
github.com/ucav/Sparrow under the MIT license. There is no cloud
service, no subscription, and no telemetry.
```
