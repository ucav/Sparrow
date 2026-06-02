# Context meter, compaction & handoff

Phase 12 ships three things:

1. A pure `ContextMeter` for tracking what fills the model window.
2. `PreCompact` / `PostCompact` hook events for tooling and observability.
3. `sparrow compact` + `HandoffDoc` for a durable summary across sessions.

## ContextMeter

```rust
let mut m = sparrow::context::ContextMeter::new(8000); // max_tokens
m.prompt_chars = 1200;
m.memory_chars = 200;
m.tools_chars = 400;
m.attachments_chars = 0;
m.transcript_chars = 5000;

m.total_chars();        // 6800
m.estimated_tokens();   // 1700  (0.25 tokens / char)
m.usage_ratio();        // 0.2125
m.should_compact(1500); // true when est + reserve > max_tokens
m.summary();            // "ctx 21% · prompt 1200 · memory 200 · ..."
```

Tracks every category that contributes to a request: system prompt, memory
docs (`MEMORY.md` / `USER.md`), tool schemas, attachments injected from the
WebView, and conversation transcript. The token estimate matches the existing
`ContextManager` ratio so the two surfaces agree.

## Hook events

```rust
HookEvent::PreCompact   // fires immediately before compaction
HookEvent::PostCompact  // emitted from Event::Compacted
```

`PreCompact` lets hooks dump or snapshot state before the transcript is
collapsed. `PostCompact` is derived from `Event::Compacted { run,
before_chars, after_chars, handoff_path }` so the WebView and recorder can
surface it in the event stream.

## `sparrow compact`

```bash
sparrow compact                                  # writes .sparrow/handoff/<ts>.md
sparrow compact --task "ship phase 12"
sparrow compact --out path/to/handoff.md
sparrow compact --json                           # JSON to stdout, MD still on disk
```

Generates a `HandoffDoc` with the stable sections:

- **Task** — the `--task` argument (or "ad-hoc handoff")
- **Files modified** — distilled from messages by file-extension scan
- **Decisions** — lines beginning with `decision:`
- **Tests run** — lines containing `cargo test`, `npm test`, or `pytest`
- **Blockers** — lines containing `blocker:` or `blocked by`
- **Next steps** — populated by callers; left empty by the CLI
- **Context** — the `ContextMeter::summary()` line, if attached

Empty sections render as `_none_` so a downstream parser can rely on the
shape.

## Status

**Alpha.** The pieces are independently tested. The engine does not yet
automatically call `sparrow compact` on a `should_compact` trigger; that
glue is intentionally deferred so the meter can land first without changing
runtime semantics for existing sessions.
