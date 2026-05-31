## Checklist

- [ ] Tests added/updated for changed behavior
- [ ] Docs updated if public API or CLI surface changes
- [ ] No secrets logged, stored, or passed to model context
- [ ] Autonomy gate and security impact reviewed
- [ ] Rollback/checkpoint behavior considered for mutating changes
- [ ] No provider lock-in introduced
- [ ] Surfaces remain thin renderers (no business logic in TUI/CLI/gateway)
- [ ] `cargo build --release` passes
- [ ] `cargo test --release` passes with no regressions
- [ ] `cargo clippy` passes with no new warnings

## Description

<!-- Describe your changes -->

## Related Issues

<!-- Link to issues this PR addresses -->
