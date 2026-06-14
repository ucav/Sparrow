# Cold-start performance — v0.9.x

Reproduce with [`scripts/perf-startup.sh`](../scripts/perf-startup.sh) after
`cargo build --release`. Measured with `hyperfine -N` (no shell wrapper), release
profile (`opt-level="z"`, `lto="thin"`, `strip`, `codegen-units=1`), Windows,
16 cores. Numbers are wall-clock means ± σ over 30 runs; absolute values are
machine-dependent — what matters is the **delta** and the absence of regressions.

## What was actually slow

The premise that `sparrow --version` cost ~361 ms was **stale**. Measured today,
`--version`/`help` are ~19–27 ms: args are parsed by clap *before* the tokio
runtime is built, so trivial commands never pay for the runtime or tracing init
(already-good engineering in `src/main.rs`).

The real waste was **boot-time model discovery**. `async_main` kicked off a
background Ollama probe (`localhost:11434`) plus a per-provider discovery sweep on
**every** command except `--console --fast` — including read-only/local commands
(`auth list`, `memory list`, `security audit`, …) that never consult a model
catalogue. Beyond the wasted startup, this opened a network connection the user
never asked for, which is poor behaviour for a trust-sensitive agent.

## Fix

`command_wants_model_discovery()` in `src/main.rs` — a conservative **denylist**:
known local/read-only commands skip boot discovery; everything else keeps it, so
no model-consuming command can regress to an empty catalogue on first run.

## Results

| Command | Before | After | Δ |
|---|---:|---:|---:|
| `sparrow --version` | ~27 ms | ~19 ms | unchanged path (warm) |
| `sparrow auth list` | ~47 ms | **~35 ms** | **−25 %** |
| `sparrow memory list` | ~58 ms | **~35 ms** | **−39 %** |
| `sparrow model --list` | ~75 ms | ~57 ms | keeps discovery (unchanged) |

`sparrow doctor` is intentionally network-bound (it probes connectivity) and
therefore noisy (±~100 ms); it is **not** affected by this change, which only
*removes* fire-and-forget work from its path.

## Guard

Run `scripts/perf-startup.sh` before/after startup-affecting changes. A light-
command (`auth list`/`memory list`) creeping back toward the model-discovery
cost (~55 ms+) is the signal that boot discovery has leaked into a path that
shouldn't pay for it.
