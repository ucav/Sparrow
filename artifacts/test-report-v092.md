# Sparrow v0.9.2 Test Report

Date: 2026-06-12

## Final Gates

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --all-targets -- -D warnings` | pass |
| `cargo test --all-targets` | pass |

Main library suite: 186 tests passed. All integration test binaries also passed.

## Required Proofs

- Stub audit: `cargo run -- audit` wrote
  `artifacts/audit-20260612-131752.md` with 0 production stubs.
- Performance: targets and misses are documented in `artifacts/perf-report.md`;
  CI perf guard exists at `.github/workflows/perf.yml`.
- CLI: new commands and flags are covered by Rust parser/unit tests and real
  command smoke runs (`audit`, `commit --dry-run`, `release prep`, `intel`).
- TUI: builder profile snapshot test passes.
- WebView: premium panel marker test passes for Timeline, Costs, Roadmap,
  Watched releases, Autonomous tasks, `/intel/backlog`, `/intel/digests`,
  and `/runs`.
- Security: destructive `exec` escalation, secret-like commit scan, and skill
  manifest tool restriction tests pass.
- Intel: `sparrow-intel` unit tests pass; opt-in guard prevents network when
  `intel.enabled=false`; real scan over `artifacts/intel-sources-v092.toml`
  produced 2 cached public release digests and 2 scored backlog tickets.

## Checklist 12.2

| Item | Status |
|---|---|
| 0 unassumed stubs | pass |
| Perf targets reached or documented | pass |
| v0.9.2 matrix demonstrable by tests/commands | pass |
| No default telemetry/network for intel | pass |
| Docs updated | pass |
| Beginner path documented through `launch`, `mode simple`, `mode builder` | pass |
| `fmt`, `clippy`, `test` green locally | pass |

GitHub Release with 3 OS binaries is intentionally left to CI after the local
release commit/tag is pushed.
