# ADR 003: Event stream as single source of truth

**Status:** Accepted

**Context:** Multiple surfaces (TUI, CLI, API, gateway) need to render the same state. The spec says "all surfaces are thin renderers over one event stream."

**Decision:** All observable state flows through the `Event` enum. Surfaces subscribe to the `EventBus` and render from events. No surface has direct access to engine internals.

**Consequences:** Surfaces are truly decoupled. Adding a new surface requires no engine changes. Events are the API contract. Transcripts are trivial (record events).
