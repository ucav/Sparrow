# r/programming post

r/programming has banned Show-style self-promotion. Frame as essay, post
the link inside, not as the title.

---

## Title

```
What "rewindable AI agents" should look like, and why I had to build my own
```

## Body

```
Two weeks ago I checked the Anthropic dashboard and saw $847 spent in 4
days on 11 modified files. The assistant — Claude Code, which I had been
paying for since beta — had silently lost my session twice that week
across crashes. The third time was the one that made me close the laptop.

I want to talk about three things that are missing from every coding
agent I've used, and what a "rewindable" agent actually has to do to be
worth your time. I'll use Sparrow, the tool I ended up writing in Rust,
as the running example, but the argument applies to whichever agent you
prefer.

1. A coding agent without a typed event bus is a black box you pay for

Most agents I've tried have a notion of "events" only inside their own
TUI loop. The TUI is the surface and the surface is the truth. When the
process crashes — which on a $20/mo subscription does happen — the truth
disappears.

The fix is to make the event bus the source of truth and have every
surface (TUI, WebView, JSON stream, gateway transports) subscribe. In
Sparrow this is one `tokio::sync::broadcast` channel feeding a typed
`Event` enum. Crash the TUI, the WebView, all transports, but as long as
the runtime daemon has the bus open, the run is recoverable.

[code pointer: src/runtime/event_bus.rs]

2. Checkpoints have to be at the tool boundary, not the message boundary

"You can use git" is not an answer. The granularity is wrong. A single
agent step often calls four tools in sequence — read, edit, edit, exec.
If exec fails in a way that corrupted the filesystem, you want a
checkpoint *between every edit*, not just before "the assistant's reply".

In Sparrow every mutating tool call goes through an autonomy gate that
emits a checkpoint event *before* the call. `sparrow rewind` walks the
checkpoint timeline backwards and atomically restores files, the
conversation, and the token counter as a unit.

[code pointer: src/orchestrator/mod.rs and src/autonomy/mod.rs]

3. Hard caps belong in the binary, not in the dashboard

Provider dashboards email you a warning after you've spent $200. That's
not a cap, that's a notification. A cap is something the program
*enforces*, in the runtime, with a `HardStop` that aborts the run at the
next safe boundary regardless of what the model is doing.

In Sparrow that's three flags: --max-cost-usd, --max-wall-secs,
--max-tokens. Hitting any of them takes the run to the last checkpoint
and prints a receipt. There is no path around them; the budget runs in
the runtime, not in a config file the user can ignore.

[code pointer: src/engine/mod.rs around the budget check]

Why I had to write this in Rust

The honest answer is "to ship a single binary". Coding agents that ship
as Python or Node packages are unmaintainable as a side project — the
dependency tree breaks, the install fails on Windows, and you spend more
time supporting `pip install` than writing the agent. A 9 MB static
binary that starts in 18 ms removes the entire support category.

Try it

  cargo install sparrow-cli
  sparrow run "explain this repo and write a TODO.md"

Repo: github.com/ucav/Sparrow  (MIT, Rust)

Roast the autonomy gate, the routing policy, or the cockpit. They are
the three modules I am least sure about.
```

---

## Why this works on r/programming

- Essay form, not "Show".
- Three concrete technical claims.
- Link buried at the bottom, not at the top.
- No mention of price/value, just architecture choices.

## What to NOT do

- No emoji.
- No "I" in the title.
- No mention of Claude Code or Anthropic in the title — moderators read
  it as competitor bashing.
