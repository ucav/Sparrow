# Build-time analysis — v0.9.x (#2)

Evidence-based, measured on the Windows dev host (16 cores, Rust 1.96).

## Measurements

| Build | Time |
|---|---:|
| Clean `cargo build` (dev) | ~56 s |
| Incremental dev rebuild (touch `src/engine/mod.rs`) | ~9 s |
| Incremental dev rebuild (touch a leaf crate → relinks bin) | ~11 s |
| `cargo build --release` | ~120–190 s |

## Findings — and why no risky change was made

1. **The dev loop is already healthy (~9–11 s incremental).** That is reasonable
   for a ~16k-LOC workspace. There is no friction here worth a risky change.

2. **The slow build is the *release* build, and it is slow *by design*.** The
   `[profile.release]` uses `opt-level="z"`, `lto="thin"`, `codegen-units=1` to
   produce a small, fast binary (~13 MB). Those settings intentionally trade
   build time for artifact quality — correct for a release artifact, and it runs
   on CI, not in the dev loop.

3. **Trimming `tokio = { features = ["full"] }` would yield nothing.** Cargo
   compiles `tokio` once for the whole workspace and *unifies* features across the
   dependency graph. As long as the `sparrow-cli` bin needs `full`, every
   sub-crate's `tokio` is built with `full` regardless of what it declares. So the
   four `features=["full"]` declarations are cosmetically over-broad but have
   **zero build-time cost**. (Trimming them also risks runtime panics — a missing
   feature like `time` is a runtime error, not a compile error — so it is pure
   downside.)

4. **Duplicate dependencies are transitive.** `cargo tree -d` shows `thiserror`
   v1+v2, `rand` 0.8+0.9, `itertools`, `hashbrown`, `console`, `windows-sys` in
   multiple versions. These are pulled by *transitive* deps; forcing single
   versions risks breaking those deps for a marginal win. Not worth it.

5. **The real incremental lever is the monolithic `sparrow-cli` bin** (`main.rs`
   ~2.1k + `engine/mod.rs` ~3.7k in one crate): any change recompiles the whole
   bin. Reducing that means **splitting code into a separate crate** so unchanged
   parts stay cached — that is the god-module refactor (tracked as #3/#9), a large
   and risky change, not a quick build tweak.

## Conclusion

No broadly-safe build-time change is warranted today. Fabricating a risky tweak
(global linker swap, version pinning, feature surgery) to "do something" would
trade real stability for marginal, host-specific gains. The honest improvement
path is the incremental crate split (#3/#9), done deliberately and measured.

## Optional, opt-in developer tip (not forced on contributors)

Linking dominates the incremental rebuild. Developers who want faster local
links can opt in *without* committing a global change, e.g. a personal
`.cargo/config.toml` override using the `rust-lld` that ships with the toolchain,
or `mold`/`lld` on Linux. This is left as a documented opt-in precisely because a
committed linker override can break contributors on platforms we can't test here.
