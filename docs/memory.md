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

## Knowledge Graph

Sparrow also keeps a persistent graph in the same SQLite database:

- `kg_nodes` stores entities such as users, projects, files, decisions, features,
  agents, docs, and long-lived tasks.
- `kg_edges` stores typed relationships such as `works_on`, `depends_on`,
  `decided`, `mentions`, `owns`, or `blocked_by`.
- Nodes and edges have JSON `properties` so agents can keep structured metadata
  without creating new tables.
- Graph writes are redacted and injection-checked like facts.
- The graph survives process restarts because it is stored in the Sparrow state
  DB, not in memory.

Agents can use the `knowledge_graph` tool to `upsert_node`, `upsert_edge`,
`search`, inspect `neighbors`, `export`, delete graph items, or run
`sync_neo4j`.

Neo4j is optional. If a local Neo4j server is available, set:

```bash
NEO4J_URL=http://127.0.0.1:7474/db/neo4j/tx/commit
NEO4J_USER=neo4j
NEO4J_PASSWORD=...
```

Then run `sparrow memory graph sync-neo4j` or let the `knowledge_graph` tool call
`sync_neo4j`. Sparrow uses Neo4j's transactional HTTP API with parameterized
Cypher statements.

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
sparrow memory graph upsert-node user:abdou Abdou --kind user
sparrow memory graph upsert-node project:sparrow Sparrow --kind project
sparrow memory graph upsert-edge user:abdou works_on project:sparrow
sparrow memory graph neighbors user:abdou --direction outgoing
sparrow memory graph search sparrow
sparrow memory graph export
```

## WebView

The local console exposes `GET /memory` with fact counts, bounded document usage,
recent facts, and stored `MEMORY.md` / `USER.md` content.

## Redaction

All memory writes pass through a redaction filter. Secrets (API keys, tokens) are never stored in memory or transcripts.
