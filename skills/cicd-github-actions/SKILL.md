# Skill: CI/CD Pipeline

**Trigger:** ci, cd, github actions, pipeline, workflow

**Description:** CI/CD avec GitHub Actions : build, test, lint, deploy, matrix builds, caching.

## Body

### Workflow standard
```yaml
name: CI
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        rust: [stable, beta]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: ${{ matrix.rust }}
      - uses: Swatinem/rust-cache@v2   # Cache target/
      - run: cargo build --release
      - run: cargo test
      - run: cargo clippy -- -D warnings
      - run: cargo fmt --check
```

### Release workflow
```yaml
on:
  push:
    tags: ['v*']
jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo build --release
      - uses: softprops/action-gh-release@v2
        with:
          files: target/release/sparrow
```

### Optimisations
```yaml
# Cache entre les runs
- uses: Swatinem/rust-cache@v2

# Éviter de runner sur tous les commits
on:
  push:
    branches: [main]
  pull_request:

# Concurrency : annuler les runs précédents
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true
```

### Pièges
- Secrets dans les logs CI → utiliser `::add-mask::`
- `cargo build` sans `--release` → binaire non optimisé
- Cache corrompu → `cargo clean` + relancer
- `ubuntu-latest` change → utiliser une version fixe si build sensible
