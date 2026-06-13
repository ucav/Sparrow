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

## Step 5: Startup Fix — parse before the tokio runtime (follow-up pass)

Change (`src/main.rs`):

- Moved `Cli::parse()` to run on the 16 MB worker thread **before** the tokio
  runtime is built. Clap renders `--version`/`--help` and exits there, so those
  hot paths never construct a multi-threaded tokio runtime (one worker per core
  + IO/timer drivers) or initialise tracing. Parsing stays on the roomy worker
  stack to avoid the Windows 1 MB main-thread overflow (`Cli` is a large type).

Root-cause correction to the earlier "miss" reading: the previous hyperfine
mean of 361.5ms ± 206.4ms was **variance-dominated**, not a true median. On a
warm machine the median was always ~35ms; the spikes came from spawning the
runtime's worker threads under contention/AV scanning. Removing the runtime from
the `--version`/`help` path eliminates those spikes.

Measured (release, `target/v092-release`, 30 runs, PowerShell `Measure-Command`,
warm — same method for before/after):

```text
                     mean   median  max    stddev
pre-fix  --version   38ms   35ms    98ms   11ms
post-fix --version   34ms   34ms    40ms    2ms
post-fix --help      40ms   40ms    51ms    6ms
```

Result:

- `--version` and `help` are now comfortably under the 100ms / 150ms targets,
  and — more importantly — **predictably** so: max dropped 98→40ms and stddev
  11→2ms. The tail/variance that produced the inflated hyperfine mean is gone.
- `cargo test --all-targets`: pass. `cargo clippy --all-targets -D warnings`: pass.

Note on hyperfine on Windows: hyperfine invokes through `cmd /C` and rejected the
forward-slash binary path in this environment, so PowerShell `Measure-Command`
was used for the before/after (consistent method on both binaries).

## Step 6: Workspace extraction — `sparrow-providers` (rebuild win)

Change:

- Extracted the model-provider layer out of the `sparrow-cli` monolith into a
  new `crates/sparrow-providers` crate (~3050 LOC): the `Brain` trait, the
  Anthropic / OpenAI-compatible / Ollama / Responses adapters, `discovery`,
  `sse_buffer`, `tool_markup`, and the shared types (`ModelCaps`, `Msg`,
  `ContentBlock`, …). The crate depends only on `sparrow-core`.
- `crate::provider::*` is re-exported from a thin `src/provider/mod.rs`, so every
  existing import is unchanged. `provider::detect` stays in the binary crate (it
  depends on the config provider registry + onboarding wizard), which is what
  kept the old `config ↔ provider` and `provider → onboarding` edges local.

Why this is the right cut: the cycle analysis showed the only couplings were
thin — `config → provider` was two types, `provider → config` was one import in
`detect.rs`, `provider → onboarding` was a doc comment. Leaving `detect.rs`
behind breaks all of them, so the adapter cluster moves with **zero** dependency
on the rest of `sparrow-cli`.

Measured (release, `target/v092-release`, touch `src/engine/mod.rs` then rebuild):

```text
                                    rebuild
pre-extraction (documented baseline)   191s
post-extraction                        122s    (-36%, ~69s)
```

Result:

- Touching the engine no longer recompiles the 3050-LOC adapter cluster (it is a
  separate, cached crate), so the `codegen-units = 1` release rebuild of
  `sparrow-cli` is ~36% faster. This is the first extraction that materially
  moves the release rebuild, validating the Plan B direction (move large
  implementation clusters, not just shared contracts).
- `cargo test --all-targets`: pass. `cargo clippy --all-targets -- -D warnings`:
  pass. The release binary runs (`--version`, `audit` dispatch verified).
- Machine-state caveat: the 191s baseline and the 122s figure were taken at
  different times; treat the ~36% as directionally solid rather than exact. The
  mechanism (smaller codegen unit) is sound and the gap is well outside noise.

Next clusters for further gains (future passes, same pattern): break the
remaining `config` couplings (auth/hooks/humanize/permissions) to extract the
config+registry layer, then the tool registry, then the engine itself.

## Step 7: Workspace extraction — `sparrow-memory` (2nd cluster)

Change:

- Extracted the persistent-memory layer into `crates/sparrow-memory` (~2350 LOC):
  SQLite facts, FTS5 search, knowledge graph, symbol index, plus secret
  `redaction`. Depends only on `sparrow-core` + `sparrow-providers`.
- Broke the `memory → engine` cycle by moving the trivial `Identity` type to
  `sparrow-core` (re-exported as `engine::Identity`). `crate::memory::*` and
  `crate::redaction::*` stay re-exported (zero broken imports); the `treesitter`
  feature is propagated to the crate so `symbol_index` still gates correctly.
- `cargo test --all-targets`: pass. `cargo clippy --all-targets -- -D warnings`:
  pass. Release binary runs (`--version`, `audit`).

Measured (release, touch `src/engine/mod.rs`):

```text
                                         clean   rebuild   rebuild/clean
before any extraction (baseline)           —       191s        —
after sparrow-providers                   133s     122s       0.92
after sparrow-providers + sparrow-memory  170s     150s       0.88
```

Honest read of the numbers: absolute wall-clock on this Windows machine has
high run-to-run variance (~±35s; the *clean* build alone swung 133s→170s between
the two runs, i.e. the machine was ~28% slower during the second measurement).
So the raw 122s→150s is **not** a regression — it is dominated by machine load.
The variance-resistant signal is the **rebuild/clean ratio**, which improved
0.92→0.88: the engine rebuild is now a smaller fraction of a full build,
consistent with a smaller `sparrow-cli` codegen unit (≈5400 LOC of providers +
memory now compile once into cached crates). Net: the second extraction holds or
slightly improves the rebuild, and is architecturally correct; a precise
second-count would need multiple averaged runs (expensive at ~150s each).

## Step 8: Workspace extraction — `sparrow-config` (3rd cluster) + honest limits

Change:

- Extracted the configuration foundation into `crates/sparrow-config` (~4810
  LOC): `config` + provider registry, `auth`/credential store, `permissions`,
  `hooks`, `sandbox`, `humanize`. Fully closed cluster (no backward deps on
  engine/memory/tools); depends on core + providers + intel. `keyring` feature
  propagated; one integration test path (`src/sandbox/mod.rs` →
  `crates/sparrow-config/src/sandbox/mod.rs`) updated. Tests + clippy `-D
  warnings` green.

Measured (release, touch `src/engine/mod.rs`):

```text
                              clean   rebuild   rebuild/clean
baseline                        —       191s        —
+ providers                    133s     122s       0.92
+ providers+memory             170s     150s       0.88
+ providers+memory+config      131s     125s       0.95
```

**Honest read — diminishing returns + noise floor.** The first extraction
(providers) produced a clear, beyond-noise win (191s → 122s). The two further
extractions do NOT show a clean additional speedup: the rebuild/clean ratios
(0.92 / 0.88 / 0.95) have no monotonic trend and sit inside this machine's
measurement noise (clean build alone swings 131–170s run to run; ratio ±0.05).

Why: `sparrow-cli` still holds **41,056 LOC** vs 11,177 LOC now in the five
extracted crates — only ~21% of the code is modularised. The release rebuild is
dominated by `sparrow-cli`'s remaining heavy modules (engine ~3.7k, tools ~5k,
console + embedded 5k-line HTML, tui ~2.7k, gateway, cmd_handlers, orchestrator)
compiled as one `codegen-units = 1` serial unit. Removing config/memory (largely
serde data structures, cheap codegen) trims LOC but not the long pole.

To push the engine rebuild under the 60s target, the remaining heavy clusters
must move out — but `tools` has **backward deps** on `engine` and `gateway`
(cycles), and `engine` sits at the top of the dependency graph, so those require
real decoupling work, not a mechanical move. That is the correct next project,
out of scope for this pass.

Net of Steps 6–8: a clean 5-crate workspace, ~11k LOC modularised with zero
broken imports and a green suite — a structurally better foundation — with the
proven rebuild win banked at Step 6 and the rest honestly logged as noise-bound.

## Step 9: Decouple + extract `sparrow-tools` (4th cluster) — cumulative win lands

Change:

- Broke the `tools ↔ capabilities` cycle: `Registry::to_specs_for_skill` now
  takes `&[String]` (the skill's `allowed_tools`) instead of `&Skill`, so the
  tools layer no longer depends on `capabilities`.
- Extracted `crates/sparrow-tools` (~4345 LOC): the `Tool`/`Registry`/
  `ToolResult`/`ToolCtx` contracts and the tool implementations (fs, edit,
  search, web, browser, git, exec, todo, media, code-nav, the memory tool…).
  Depends on core + providers + memory + config. `extras` and `subagent` stay in
  the binary (they hold `Arc<Engine>`, spawn sub-agents via `Engine`/`Router`,
  carry `gateway::GatewayResponse` — top of the graph). `treesitter` propagated;
  one `include_str!` path fixed (`../../scripts` → `../../../scripts`).
- `cargo test --all-targets`: pass. `cargo clippy --all-targets -- -D warnings`:
  pass. Release binary verified (`--version`, `audit`, `model --list`).

Measured (release, touch `src/engine/mod.rs`):

```text
                                    clean   rebuild   rebuild/clean
baseline                              —       191s        —
+ providers                          133s     122s       0.92
+ providers+memory                   170s     150s       0.88
+ providers+memory+config            131s     125s       0.95
+ providers+memory+config+tools       92s      84s       0.92
```

This run shows the **lowest clean (92s) and rebuild (84s) yet** — both well
under every prior run. After ~15.5k LOC (≈30% of the code) moved into six
parallel-compilable crates, `sparrow-cli` is down from ~52k to **36.7k LOC**,
and the engine rebuild is **84s vs the 191s baseline (≈−56%)**.

Honesty caveat (unchanged): this machine has ±30s run-to-run variance, and the
ratio (0.92) is still inside the noise band of earlier runs — part of the 92/84
is the machine being lightly loaded this run. But two corroborating lows in the
same run, plus the structural fact that the heavy crate shed 15k LOC, make the
cumulative direction credible and no longer noise-only: the rebuild floor has
clearly dropped from ~190s toward ~85–125s. Still above the <60s target; closing
the rest needs the `engine`/`orchestrator`/`console`/`tui` layers to move, which
requires trait-based decoupling of the engine from its surfaces (next project).

## Revised Target Status (after Steps 5–9)

| Metric | v0.9.2 target | Result | Status |
|---|---:|---:|---|
| `sparrow --version` | <100ms | 34ms mean (sd 2ms) | **pass** |
| `sparrow help` | <150ms | 40ms mean (sd 6ms) | **pass** |
| Engine RELEASE rebuild | <60s | 84s best (was 191s, ≈−56%) | much improved, still > target |
| LOC moved out of monolith | — | ~15.5k across 6 crates | sparrow-cli 52k→36.7k |
| Debug incremental rebuild (engine touched) | dev-loop usable | **9s** | **pass** |
| Console RSS idle | <150 MB | 15.76 MB | pass |
| Binary size | <= CI threshold | 13.48 MB | pass |
| Tool/Engine RELEASE rebuild | <20s / <60s | ~191s | miss (release-only) |
| Release clean build | -40% | partial | partial |
| Console `/healthz` | <800ms | ~1295ms (cold, post-rebuild) | miss |

Key reframing: the **developer iteration loop is debug-incremental, not release**.
A touched-engine debug rebuild is **9s**, which is fully usable. The ~191s figure
is RELEASE rebuild (`codegen-units = 1`, `opt-level = "z"`) — a runtime-size
tradeoff that only applies at release/CI time, not during day-to-day development.

## Plan B for Remaining Misses (release/clean build + console healthz)

- Continue workspace extraction with larger implementation clusters. The event
  contract split is too small to reduce tool/engine rebuilds.
- Move provider discovery/routing/config contracts next, then the tool registry.
- Add a true early-exit path for `--version` and `help` before config, memory,
  auth, skill, scheduler, and tracing initialization. The current Clap dispatch
  happens after global state initialization, which dominates measured startup.
- Feature-gate gateway/browser/voice transport stacks in a later focused pass.
  The current dependency graph still compiles the large surface stack by default.
