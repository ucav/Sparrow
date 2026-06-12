# Sparrow v0.9.2 Final Validation Report

Date: 2026-06-12

## Gates

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --all-targets -- -D warnings` | pass |
| `cargo test --all-targets` | pass |

`cargo test --all-targets` completed with the main library suite at 186 tests
passing plus all integration test binaries passing.

## Targeted v0.9.2 Coverage

| Area | Evidence |
|---|---|
| Phase 1 anti-stub | CI/nightly grep plus `sparrow audit` reports zero production stubs |
| Phase 2 perf | `artifacts/perf-baseline.md`, `artifacts/perf-report.md`, `.github/workflows/perf.yml` |
| Phase 3 CLI | tests for `audit`, `test`, `commit`, `release prep`, `--plan-first`, `--dry-run`, `--patch` |
| Phase 4 TUI | `builder_mode_renders_builder_menu`, full TUI render suite |
| Phase 5 WebView | `right_sidebar_has_v092_premium_panels_and_endpoints` |
| Phase 6 manifests | `skill_manifest_restricts_tool_specs`, `tool_manifest_declares_permissions_from_risk` |
| Phase 7 intel | `sparrow-intel` unit tests, `disabled_intel_without_explicit_source_refuses_scan_before_network` |

## Real Intel Scan

Command:

```powershell
cargo run -- intel scan --config artifacts\intel-sources-v092.toml --limit 1 --json
```

Result:

- Scanned 2 public GitHub Releases sources.
- Cached 2 items.
- `sparrow intel report --limit 5 --json` returned Rust and Tokio digests.
- `sparrow intel backlog --limit 5 --json` returned scored backlog tickets for
  Tokio and Rust.

## Known Misses / Honest Notes

- Phase 2 performance targets are not all met. The misses and Plan B are
  documented in `artifacts/perf-report.md`.
- The `Roadmap` and `Watched releases` WebView panels intentionally read only
  the local intel cache. Network fetch remains opt-in through `sparrow intel scan`.
- `GET /runs` exposes recorded local transcripts and leaves live local tasks to
  the WebSocket-fed rightbar state.
