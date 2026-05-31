# Contributing to Sparrow

Welcome! Sparrow is a serious open-source project with strict architectural standards.

## Architecture Rules

1. **Every module exposes a trait, not a concrete type.** Modules must be testable in isolation against their trait with mocks.
2. **The runtime is headless.** All surfaces (TUI, CLI, API, messaging) are thin renderers over one event stream. No business logic in surfaces.
3. **Autonomy is a continuous dial, never two modes.** If the implementation collapses to "interactive vs daemon", it is wrong.
4. **Nothing mutating runs without an autonomy decision and a checkpoint.** Reproducibility and rollback are first-class.
5. **Secrets never enter transcripts, logs, or model context** unless explicitly a tool argument. Redact aggressively.
6. **No provider lock-in.** The router selects models; users can switch live.

## Development Setup

```bash
git clone https://github.com/ucav/Sparrow.git
cd Sparrow
cargo build --release
cargo test --release
```

## Testing

- **Unit tests** required for every module against its trait with mock implementations.
- **Integration tests** in `tests/` for cross-module behavior.
- Golden replay tests for deterministic transcript rendering.
- Router simulation tests with synthetic budgets and errors.
- Autonomy matrix tests covering every (level × RiskLevel) combination.
- Redaction tests ensuring secrets never appear in events/transcripts.

```bash
cargo test --release
```

## Pull Request Checklist

- [ ] Tests added/updated for changed behavior
- [ ] Docs updated if public API or CLI surface changes
- [ ] No secrets logged, stored, or passed to model context
- [ ] Autonomy gate and security impact reviewed
- [ ] Rollback/checkpoint behavior considered for mutating changes
- [ ] No provider lock-in introduced
- [ ] Surfaces remain thin renderers (no business logic in TUI/CLI/gateway)
- [ ] `cargo build --release` and `cargo test --release` pass
- [ ] `cargo clippy` passes with no new warnings

## Code Style

- Rust edition 2024
- Follow existing module conventions (trait boundaries, error types)
- No comments in code unless absolutely necessary for clarity
- Public API documentation (rustdoc) on public types and traits

## Commit Messages

- Use conventional commits: `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`
- Keep commits focused on one change

## Questions

Open a [GitHub Discussion](https://github.com/ucav/Sparrow/discussions) for questions about architecture, design decisions, or contribution guidance.
