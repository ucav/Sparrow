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
- User-editable via `sparrow memory` (planned)
- Redacted: secrets never stored
- Stored in `facts` table with LIKE-based recall

## Redaction

All memory writes pass through a redaction filter. Secrets (API keys, tokens) are never stored in memory or transcripts.
