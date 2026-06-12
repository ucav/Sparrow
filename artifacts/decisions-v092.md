# Sparrow v0.9.2 Decisions

Date: 2026-06-12

## D-001 Track Markdown Reports in `artifacts/`

Decision: keep bulky generated artifacts ignored, but allow `artifacts/*.md` to
be tracked.

Why: `PLAN_v0.9.2.md` requires audit, perf, decisions and test reports under
`./artifacts/`. The repo ignored the whole directory, which made the Phase 0
deliverable local-only.

Impact: `.gitignore` now keeps `artifacts/` and `artifacts/*.md` visible to git.
Large files, databases, transcripts and non-Markdown generated outputs remain
ignored by existing rules.

## D-002 Use `target/v092-release` for Baseline Builds

Decision: run v0.9.2 release baselines with
`cargo build --release --timings --target-dir target/v092-release`.

Why: the existing `target/release/sparrow.exe` was locked by running Sparrow
processes. Using a separate target dir produced a clean baseline without killing
the user's active local processes.

Impact: the clean release baseline is valid and reproducible, but the extra
target directory increases local disk usage. It stays under ignored `target/`.

## D-003 Install Missing Perf Tools Locally

Decision: install `cargo-bloat` and `hyperfine` with `cargo install`.

Why: Phase 0 requires `cargo bloat` and `hyperfine` baselines. Fallback
PowerShell measurements were useful but not enough for the plan's stated gate.

Impact: two user-level Cargo binaries were added under `C:\Users\abdou\.cargo\bin`.
No project dependency was added.

## D-004 Treat Phase 2 Perf Targets as Misses, Not Release Claims

Decision: keep the Phase 2 measured misses visible in
`artifacts/perf-report.md` instead of claiming the plan targets were reached.

Why: the first workspace split only moved the event contract, which is too small
to reduce rebuild time materially. Windows startup measurements also remained
above the `--version`, `help`, and console `/healthz` targets after `--fast` and
idle prefetch work.

Impact: Phase 2 can proceed only with a documented plan B. Public release notes
must not claim the missed targets until later measurements prove them.

## D-005 Keep `console --fast` Browser-Free

Decision: `sparrow console --fast` prints the WebView URL but does not auto-open
the OS browser.

Why: the fast-start mode is meant to skip boot animation, eager panel preloads,
and discovery. Launching a browser from the CLI adds OS-dependent latency and
makes `/healthz` measurements noisy.

Impact: normal `sparrow console` keeps the existing auto-open behavior. Fast
mode is better for scripts, perf probes, and users who want to choose where to
open the URL.
