# Skill: Rust Development

**Trigger:** rust, cargo, rust code, Rust project

**Description:** Développement Rust : cargo, clippy, tests, erreurs, async, performance.

## Body

```bash
# Nouveau projet
cargo new my-project && cd my-project

# Build + test + lint + format
cargo build --release
cargo test
cargo clippy -- -D warnings
cargo fmt --check

# Dependencies
cargo add tokio serde anyhow
cargo update                # Mise à jour Cargo.lock
cargo outdated              # Voir les deps périmées

# Profiling
cargo flamegraph --bin sparrow -- "run 'task'"
cargo bench                 # Benchmarks criterion
```

### Patterns essentiels
```rust
// Error handling : anyhow pour apps, thiserror pour libs
fn main() -> anyhow::Result<()> {
    let config = std::fs::read_to_string("config.toml")
        .context("Cannot read config")?;
    Ok(())
}

// Async avec tokio
#[tokio::main]
async fn main() {
    let resp = reqwest::get("https://api.example.com").await?;
}

// Éviter unwrap() — utiliser ? ou .context()
// Éviter clone() dans les boucles — utiliser &
// Préférer &str à String pour les params de fonction
```

### Pièges
- `cargo build` vs `cargo build --release` : perf x10
- `unwrap()` en production = panic difficile à debugger
- Lifetimes complexes : `'a`, `'b` → simplifier avec `Arc` ou owned types
- `async fn` dans les traits : utiliser `#[async_trait]`
