# Sparrow v0.9.2 Performance Baseline

Date: 2026-06-12
Target dir: `target/v092-release`
Binary: `target\v092-release\release\sparrow.exe`

## Build

```text
cargo build --release --timings --target-dir target/v092-release
EXIT=0
SECONDS=311.508
Finished `release` profile [optimized] target(s) in 5m 11s
```

Incremental release rebuilds:

```text
touch src/tools/todo.rs -> 191.405s
touch src/engine/mod.rs -> 191.897s
```

## Startup

`hyperfine --warmup 2`:

```text
sparrow --version mean 236.5 ms ± 192.9 ms, range 26.1 ms … 635.5 ms
sparrow help      mean 359.4 ms ± 116.5 ms, range 200.8 ms … 525.4 ms
```

PowerShell Stopwatch fallback, existing release binary:

```text
sparrow --version avg 34.16 ms, min 29.44 ms
sparrow help      avg 32.16 ms, min 28.38 ms
```

## Console and WebView

Temporary release console on port `19442`:

```text
/healthz OK in 640.77 ms
RSS after 30s idle: 19.25 MB
```

Playwright against `?theme=white`:

```text
domContentLoaded: 268.9 ms
loadEvent: 454.6 ms
first-paint: 284 ms
first-contentful-paint: 284 ms
console errors: 0
```

## Size

```text
target directory size: 97.73 GB before target/v092-release cleanup
release binary:        13,096,448 bytes
release PDB:            7,401,472 bytes
release libsparrow:    32,860,520 bytes
```

`cargo bloat --release --target-dir target/v092-release -n 30`:

```text
.text section size: 7.9 MiB
file size by cargo-bloat: 12.5 MiB
top symbols:
  342.0KiB sparrow::async_main::async_fn$0
  135.8KiB sparrow::event deserialize
  117.2KiB sparrow::cli augment_subcommands
  108.0KiB sparrow::engine drive_with_inject
  106.4KiB sparrow::engine drive_with_inject
   89.5KiB sparrow::config::providers::provider_registry
```

`cargo bloat --release --target-dir target/v092-release --crates -n 30`:

```text
2.3MiB sparrow
1.1MiB std
948.8KiB unknown
401.3KiB axum
317.1KiB toml_edit
298.8KiB rustls
235.1KiB regex_automata
226.9KiB tokio
167.4KiB clap_builder
136.5KiB reqwest
```

## Dependency Duplicates

Notable `cargo tree -d` duplicate families:

```text
console v0.15.11 / v0.16.3
getrandom v0.2.17 / v0.3.4 / v0.4.2
hashbrown v0.14.5 / v0.15.5 / v0.17.1
itertools v0.10.5 / v0.13.0
rand v0.8.6 / v0.9.4
thiserror v1.0.69 / v2.0.18
unicode-width v0.1.14 / v0.2.0
windows-sys v0.59.0 / v0.61.2
```
