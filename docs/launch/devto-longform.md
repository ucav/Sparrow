# Dev.to long-form: backlinks SEO

Post 48h after the HN/X launch, when the conversation has settled.
~1500 words. The angle is "essay about your frustration, with code
pointers", not "feature list". Tag it: `rust`, `ai`, `tooling`, `showdev`.

---

## Title

```
I spent $847 on Claude Code in 4 days, then I rewrote it in Rust
```

## Cover image

`assets/launch/og-card.png`

## Canonical URL

Leave empty (Dev.to becomes the canonical so the SEO comes here).

## Body (paste verbatim, lightly edit timestamps)

```markdown
Two weeks ago I checked my Anthropic billing dashboard at 11 PM after my
kids went to sleep. It said $847.21 spent in the trailing 4 days.
Eleven files changed in those 4 days. The assistant — Claude Code, which
I'd been using since the beta — had silently lost my session three times
that week across crashes. The third crash, on day 4, was the one that
made me close the laptop.

I want to tell that story, then walk through the three architectural
calls I made when I rewrote the thing in Rust over the next 14 days.
The result is [Sparrow](https://github.com/ucav/Sparrow), MIT, one
binary, 9 MB, public beta. If you read to the bottom you'll get a
working install command.

## The bill

The bill itself wasn't the worst part. Claude Code does a lot of work,
and on a fast project $200/day is defensible if the assistant remembers
what it's doing. The worst part was that **three of those four days had
ended with a session crash that took my context with it**. I'd paid for
the model to learn my codebase, then paid again for it to relearn it,
then paid a third time for it to relearn it after the second crash.

The frustration is structural. Claude Code is a TUI binding directly to
Anthropic's API. The TUI process **is** the source of truth. When the
process dies, the truth dies with it. Restoring "where you were" means
re-uploading the codebase context, which means another big input-tokens
bill, which means the cycle keeps going.

I went to bed angry. I woke up and started writing Rust.

## Three calls I made differently

### 1. The event bus is the source of truth, not the TUI

The first thing I did was decide that no surface owns the run. There is
exactly one **typed event stream** — a `tokio::sync::broadcast` channel
emitting a `Event` enum — and three independent surfaces subscribe to
it: a `ratatui` terminal cockpit, a WebView cockpit served by `axum` on
port 9339, and a `--json` NDJSON stdout writer for pipelining. They are
peers; none can corrupt the others.

```rust
// src/runtime/event_bus.rs (excerpt)
pub struct EventBus {
    tx: broadcast::Sender<Event>,
}

impl EventBus {
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }
    pub fn emit(&self, event: Event) {
        let _ = self.tx.send(event);
    }
}
```

What this buys you: when the TUI crashes, the WebView keeps streaming.
When the WebView panics on a bad render, the JSON pipeline keeps
writing. When the daemon process is restarted the persisted recorder
replays missed events. None of the three surfaces ever blocks the run.

### 2. Checkpoints are at the tool boundary, not the message boundary

"You can use git to undo it" is a non-answer. The granularity is wrong.
A typical agent step calls four tools in sequence — read, edit, edit,
exec. If the exec corrupts something, you want a snapshot between
*every edit*, not just before "the assistant's last reply".

So every mutating tool call goes through an autonomy gate that emits a
checkpoint **before** the call. `sparrow rewind --last` walks the
checkpoint timeline backwards and atomically restores **files,
conversation, and token counter** as a unit. Not just the files — the
whole run state.

```rust
// src/autonomy/mod.rs (excerpt)
pub fn evaluate(&self, action: &ProposedAction) -> AutonomyVerdict {
    let decision = self.approve.decide(action);
    let needs_checkpoint = matches!(
        action.risk,
        RiskLevel::Mutating | RiskLevel::Exec | RiskLevel::Destructive
    ) && matches!(decision, Decision::Allow | Decision::AskUser);
    AutonomyVerdict::new(decision, needs_checkpoint, /* ... */)
}
```

What this buys you: "shit I left it running overnight" stops being a
five-hour cleanup. It's one command.

### 3. Hard caps belong in the binary, not in the dashboard

Provider dashboards email you a warning when you've spent $200. That's
not a cap. That's a postmortem.

A cap is something the program **enforces** at the runtime layer, with
a `HardStop` that aborts the run at the next safe boundary regardless
of what the model is doing. In Sparrow that's three flags:
`--max-cost-usd`, `--max-wall-secs`, `--max-tokens`. Hitting any of
them takes the run to the last checkpoint and prints a receipt. There
is no path around them; the budget enforcement runs in the engine
loop, not in a config the user can edit during the run.

```bash
sparrow run "scrape and analyze this domain" \
    --max-cost-usd 0.50 \
    --max-wall-secs 300 \
    --max-tokens 50000 \
    --autonomy supervised
```

What this buys you: I sleep at night.

## Why Rust, honestly

The honest answer is "to ship one binary". Coding agents that ship as
Python or Node packages are unmaintainable as a side project. The
dependency tree breaks. The install fails on Windows. You spend more
time supporting `pip install` than writing the agent.

A 9 MB static binary that starts in 18 ms removes the entire support
category. `cargo install sparrow-cli` either works or fails with a
real error message. Three months from now, when I bump a dep, nothing
in your install breaks.

That, and `tokio::sync::broadcast` is the right primitive for an event
bus and Python doesn't have a good equivalent.

## What you get if you try it

```bash
cargo install sparrow-cli
sparrow launch
```

First run wizard scans your environment for 20+ provider keys. None
found? It offers Groq's free tier or installs Ollama locally. Cockpit
opens at `http://127.0.0.1:9339`.

```bash
sparrow run "add a rate limiter to the auth endpoint" \
            --max-cost-usd 0.10 --max-wall-secs 60
```

Stream the events. Hit Ctrl+C anytime — the run aborts at the last
checkpoint. `sparrow rewind --last` undoes everything.

## What I'd love feedback on

I am one person. The repo is at github.com/ucav/Sparrow. The three
modules I am least sure about are:

- `src/router/mod.rs` — static cost-tier routing
- `src/autonomy/mod.rs` — three-level continuous dial
- `src/sandbox/` — bwrap on Linux, honest "unsupported" on macOS/Windows

If you read any of those files and you think I'm wrong, open an issue.
I will reply.
```

---

## After publishing

- Cross-post to Hashnode 24h later (Hashnode allows canonical pointing
  back to Dev.to so no SEO loss).
- DM the link to anyone who replied "looks interesting" on the HN /
  X / Reddit threads — Dev.to long-form converts skeptics that didn't
  bite on the launch.
- Pin to your GitHub profile README under "Featured posts".
