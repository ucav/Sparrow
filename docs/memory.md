# Memory

Sparrow has four memory tiers, all persisted in SQLite.

## Repo Memory

- File tree + symbol index for the workspace
- Scanned on demand, cached to disk
- Symbols extracted for Rust, TypeScript, Python, Go
- Optional: embeddings for semantic search

## Identity Memory

- Persistent SOUL per agent: name, role, personality, rules
- Survives across sessions
- Stored in `identities` table

## Task Memory

- Ephemeral conversation state for one run
- Messages, tool calls, results
- Persisted at end of run for replay/audit
- Stored in `tasks` table as JSON

## Shared Memory

- Cross-agent workspace communication
- Signals: messages between agents (e.g., verifier → coder)
- Working docs: shared documents (e.g., plan, spec)
- Stored in `signals` and `working_docs` tables

## Facts (Durable User Memory)

- Key-value facts about the user and preferences
- Auto-distilled from successful runs
- User-editable via `sparrow memory`
- Redacted: secrets never stored
- Stored in `facts` table with FTS5 recall and LIKE fallback
- Duplicate keys are rejected unless the caller explicitly replaces the same fact id

## Bounded Memory Docs

Sparrow keeps two bounded Markdown-style documents in SQLite:

- `MEMORY.md` — durable project/user operating memory, max 2200 characters.
- `USER.md` — user preference/profile memory, max 1375 characters.

Both are injected into the system context as facts, not executable instructions.
Writes are rejected if they contain prompt-injection phrases, credential
exfiltration intent, invisible Unicode controls, or oversized content.

## Session Search

`SessionStore` indexes saved conversation messages in FTS5 when a session is
saved. `sparrow memory search "<query>"` finds old turns and `sparrow memory
scroll <session> --around <n>` shows neighboring messages.

## CLI

```bash
sparrow memory list
sparrow memory add <key> <value>
sparrow memory replace <id> <key> <value>
sparrow memory forget <id>
sparrow memory recall "<query>" --limit 10
sparrow memory consolidate
sparrow memory docs
sparrow memory search "<query>" --limit 10
sparrow memory scroll <session> --around 4 --before 3 --after 3
```

## WebView

The local console exposes `GET /memory` with fact counts, bounded document usage,
recent facts, and stored `MEMORY.md` / `USER.md` content.

## Redaction

All memory writes pass through a redaction filter. Secrets (API keys, tokens) are never stored in memory or transcripts.
