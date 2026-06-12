# Sparrow v0.9.2 Performance Report

Date: 2026-06-12

## Step 1: Workspace Split, Increment 1

Change:

- Added Cargo workspace root.
- Added `crates/sparrow-core`.
- Moved the event contract from `src/event.rs` to
  `crates/sparrow-core/src/event.rs`.
- Re-exported it from the main crate with `pub use sparrow_core::event;`, so
  existing `crate::event::*` imports keep working.

Verification:

```text
cargo check --all-targets: pass
```

Release rebuild timings, using `target/v092-release`:

```text
baseline before split:
  clean release build       311.508s
  touch src/tools/todo.rs   191.405s
  touch src/engine/mod.rs   191.897s

after sparrow-core split:
  post-split release build               214.366s
  touch crates/sparrow-core/src/event.rs 196.230s
  touch src/tools/todo.rs                191.091s
  touch src/engine/mod.rs                190.984s
```

Result:

- The workspace boundary is established.
- Moving `event` alone does not materially improve tool or engine rebuilds.
- Touching `sparrow-core` still recompiles `sparrow-cli`, as expected, because
  the binary crate depends on the core event contract.

Next implication:

- The next high-impact extraction must move larger implementation clusters out
  of `sparrow-cli`, not just shared contracts.
- Candidate order remains close to the plan: providers/router/config contract,
  then tools, then engine surfaces.

## Step 2: Dev Profile

Change:

```toml
[profile.dev]
opt-level = 0
debug = "line-tables-only"
incremental = true

[profile.dev.package."*"]
opt-level = 2
```

Verification:

```text
cargo check --all-targets: pass
```

Observed cost:

- The first `cargo check --all-targets` after changing dev profile rebuilt much
  of the graph and took 3m43s.
- Follow-up dev checks should be measured separately after the cache settles.

## Step 3: Console Fast Start and WebView Idle Prefetch

Change:

- Added `sparrow console --fast`.
- `--fast` skips boot-time provider discovery before the console bind.
- `--fast` opens the WebView URL with `?boot=0&fast=1`.
- In fast mode the browser is not auto-opened, so scripts can measure `/healthz`
  without paying OS browser launch cost.
- WebView drawer/cache preloads now run through `requestIdleCallback` in normal
  mode and are skipped in fast mode until panels are opened.

Verification:

```text
cargo check --all-targets: pass
cargo test cli::tests::console_fast_flag_parses: pass
cargo test console_html_matches_v0_3_visual_polish_contract: pass
```

Measured with the release binary under `target/v092-release`:

```text
sparrow console --fast --port 19443 -> /healthz OK: 1295.46ms
RSS at healthz: 15.76 MB
```

Result:

- Memory is well below the 150 MB target.
- `/healthz` remains above the 800 ms target on this Windows run.
- The path is cleaner than the normal console path because it avoids eager
  discovery, boot animation, eager WebView prefetches, and browser launch.

## Step 4: Release Profile

Change:

```toml
[profile.release]
opt-level = "z"
lto = "thin"
strip = true
codegen-units = 1
```

Measured with `target/v092-release`:

```text
clean-ish release build after profile change: 261.292s
incremental release rebuild after main.rs touch: 180.308s
binary size: 13,421,056 bytes
hyperfine --warmup 2:
  sparrow --version mean: 361.5ms ± 206.4ms
  sparrow help mean:      442.0ms ± 180.7ms
```

Result:

- Binary size stays below the 15,061,000 byte CI threshold derived from
  baseline + 15%.
- CLI startup targets are still missed. The observed Windows variance is high
  (`--version` min 26.6ms, max 723.0ms), but the mean is the gate value and must
  be treated as a miss.
- `cargo bloat --release --target-dir target/v092-release -n 15 --crates`
  timed out at 184s after the profile rebuild; the Phase 0 bloat baseline
  remains the latest successful bloat snapshot.

## Current Target Status

| Metric | Baseline | Current | v0.9.2 target | Status |
|---|---:|---:|---:|---|
| Release clean build | 311.508s | 261.292s latest release rebuild after profile reset | -40% vs baseline | partial |
| Tool release rebuild | 191.405s | 191.091s | <20s | miss |
| Engine release rebuild | 191.897s | 190.984s | <60s | miss |
| `sparrow --version` hyperfine | 236.5ms mean | 361.5ms mean | <100ms | miss |
| `sparrow help` hyperfine | 359.4ms mean | 442.0ms mean | <150ms | miss |
| Console `/healthz` | 640.77ms normal console baseline | 1295.46ms fast measured after rebuild | <800ms | miss |
| Console RSS idle/early | 19.25 MB idle baseline | 15.76 MB at fast healthz | <150 MB | pass |
| Binary size | ~13.10 MB baseline | 13,421,056 bytes | <= baseline / <= CI threshold | partial |

The first workspace increment is structurally useful but insufficient for the
performance targets.

## Plan B for Missed Targets

- Continue workspace extraction with larger implementation clusters. The event
  contract split is too small to reduce tool/engine rebuilds.
- Move provider discovery/routing/config contracts next, then the tool registry.
- Add a true early-exit path for `--version` and `help` before config, memory,
  auth, skill, scheduler, and tracing initialization. The current Clap dispatch
  happens after global state initialization, which dominates measured startup.
- Feature-gate gateway/browser/voice transport stacks in a later focused pass.
  The current dependency graph still compiles the large surface stack by default.
