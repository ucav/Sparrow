# ADR 001: Rust over Python/TypeScript

**Status:** Accepted

**Context:** Sparrow needed a language for a single self-contained CLI. Options were Rust, Python, TypeScript/Node.

**Decision:** Rust. Rationale: single static binary (no runtime dependency), sandbox primitives (seccomp, namespaces), performance, ecosystem (ratatui, rusqlite, git2, tokio).

**Consequences:** Steeper learning curve for contributors. Faster execution, smaller binary, true single-file install.
