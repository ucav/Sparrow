# Skill: Rust Development
**Trigger:** rust code, cargo, rust project, write rust
**Description:** Rust development best practices — cargo workflows, error handling, testing, performance.

## Body
1. Project setup: cargo new / cargo init
2. Dependencies: cargo add <crate>, check Cargo.toml
3. Build: cargo build, cargo build --release
4. Test: cargo test, cargo test -- --nocapture
5. Lint: cargo clippy -- -D warnings
6. Format: cargo fmt
7. Error handling: use anyhow for apps, thiserror for libs
8. Use ? operator, avoid unwrap() in production
9. Async: tokio for runtime, async_trait for trait async fns
10. Performance: cargo flamegraph, criterion benchmarks
