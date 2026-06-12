# Sparrow v0.9.2 Phase 0 Audit

Date: 2026-06-12
Workspace: `C:\Sparrow`
Branch: `master`
HEAD observed after tests: `cb254229`

## Executive Summary

Phase 0 is the requested audit artifact for `PLAN_v0.9.2.md`.

The v0.9.2 plan is directionally correct: Sparrow is still a single large crate,
the compile hot spot is the local `sparrow-cli` unit, the WebView is still a
large monofile, and CI has no startup/perf regression gate. The current codebase
is functionally healthy enough to continue: `cargo check --all-targets`,
`cargo test --all-targets`, and `cargo clippy --all-targets -- -D warnings`
passed in this audit run.

Initial blockers found:

1. `cargo fmt --all -- --check` currently fails on `src/extras.rs`.
2. `cargo build --release --timings` failed because
   `target\release\sparrow.exe` is locked by running Sparrow processes.

Follow-up in the same autonomous run:

- `cargo fmt --all` fixed the `src/extras.rs` formatting drift.
- `.gitignore` was adjusted to keep `artifacts/*.md` trackable while still
  ignoring bulky generated artifacts.
- A clean release baseline was produced in `target/v092-release` without
  stopping any existing Sparrow process.

## Documents Read

- `PLAN_v0.9.2.md`
- `PLAN_v0.9.1.md`
- `CHANGELOG.md`
- `C:\Users\abdou\.codex\SOUL.md`
- `C:\Users\abdou\.codex\skills\codex-efficient-orchestration\SKILL.md`

## Working Tree at Start

The repository was already dirty before v0.9.2 work began:

```text
 M .gitignore
 D AUDIT_v0.8.0.md
 M CHANGELOG.md
 M README.md
 M console.html
 M src/console.rs
 M src/engine/main_soul.md
 M src/engine/mod.rs
 M src/extras.rs
 M src/main.rs
```

Observed diff stat:

```text
10 files changed, 368 insertions(+), 297 deletions(-)
```

Important implication: v0.9.1 appears to be present as uncommitted work rather
than a completed release commit. `Cargo.toml` still reports version `0.9.0`, and
`CHANGELOG.md` has `v0.9.1` under `[Unreleased]`.

## Repository Shape

Rust source:

```text
SrcRustFiles     : 145
SrcRustLines     : 45971
TestFiles        : 37
TestLines        : 5669
ConsoleHtmlLines : 5532
```

Largest local files:

```text
src\engine\mod.rs                     3551
src\console.rs                        2414
src\tui\mod.rs                        2197
src\main.rs                           1805
src\config\providers.rs               1364
src\memory\mod.rs                     1175
src\orchestrator\mod.rs               1052
src\capabilities\mod.rs                844
src\extras.rs                          841
src\provider\openai_compat.rs          838
src\cli\mod.rs                         812
```

Current package:

```text
name    = sparrow-cli
version = 0.9.0
bin     = sparrow -> src/main.rs
lib     = sparrow -> src/lib.rs
```

Workspace status from `cargo metadata`: one workspace member only:

```text
path+file:///C:/Sparrow#sparrow-cli@0.9.0
```

Conclusion: D1 monocrate is confirmed.

## Cargo Features and Profiles

Current feature surface:

```toml
[features]
default = ["keyring"]
keyring-dep = ["keyring"]
browser = []
treesitter = ["tree-sitter", "tree-sitter-rust", "tree-sitter-python", "tree-sitter-javascript"]
email = ["lettre", "imap", "native-tls"]
```

Current release profile:

```toml
[profile.release]
opt-level = "z"
lto = true
strip = true
codegen-units = 1
```

No `[profile.dev]` optimization block is present yet. Plan §4.2 item 2 is still
open.

## Boot Path Audit

Relevant findings from `src/main.rs`:

- `main()` starts at line 36 and uses a Tokio runtime.
- Provider discovery is kicked off near lines 154-216.
- Discovery is spawned in background tasks for Ollama and credentialed
  providers, so the main thread does not await the network discovery loop there.
- Console startup goes through `run_console_server` around line 1472.
- Console bind is resolved before server creation through
  `console::resolve_bind_addr`.
- The WebView server is created around lines 1842-1852, then `server.serve()`.

Relevant findings from `src/console.rs`:

- `/healthz` is lightweight and directly returns `{"ok":true}`.
- `SPARROW_CONSOLE_HTML` dev-loop is supported.
- `TcpListener::bind(self.addr)` is inside `WebViewServer::serve`.
- Static providers are loaded in endpoints such as `/models` and `/config`.
- Live model discovery is behind `POST /providers/scan`.

Preliminary conclusion: the plan's D5 warning is partially mitigated already for
console startup. A stricter Phase 2 trace should still verify that no network
call sits between `main() -> listener.bind()` for every launch path.

## Console Endpoints

Routes declared in `src/console.rs`:

```text
/
/healthz
/run
/plan
/cli
/commands
/memory
/plugins
/tools
/models
/status
/file
/conversation/reset
/stop
/approval
/config
/permissions
/security
/sessions
/sessions/load
/history
/agents
/agents/:name
/skills
/upload
/artifacts
/providers/scan
/routing
/todos
/preview/scan
/replay
/replays
/mcp/list
/hooks
/update/check
/ws
```

Approximate count: 36 route declarations. The v0.9.2 WebView additions
`/intel/*` and `/runs` do not exist yet.

## WebView Audit

`console.html` is confirmed as a large monofile: 5,532 lines.

Network font load still exists:

```html
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=IBM+Plex+Mono:wght@300;400;500;600;700&display=swap" rel="stylesheet">
```

White theme tokens and `localStorage` persistence exist. The current drawer
rail has 10 panels: crew, sessions, memory, plugins, tools, skills,
permissions, security, route, artifacts. The right tool panel exists with the
six v0.9 tools.

Missing from v0.9.2 plan: Timeline, Costs, Roadmap, Watched Releases,
Autonomous Tasks panels.

## Tool and Skill Architecture

Current `Tool` trait in `src/tools/mod.rs`:

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> serde_json::Value;
    fn risk(&self) -> RiskLevel;
    fn metadata(&self) -> ToolMetadata { ... }
    async fn call(...);
}
```

`ToolMetadata` exists but is not the planned richer `ToolManifest`.
Known tool metadata is static and category/risk based.

Registered engine tools include:

```text
fs_read, fs_list, fs_write, edit, multi_edit, search, web_search, web_fetch,
browser, computer, git, todo, exec, image_generate, text_to_speech, transcribe,
python_rpc, lsp, glob, symbols, memory, knowledge_graph, subagent_spawn
```

Skill v2 manifest permissions are not implemented yet. Current skill loading is
`SKILL.md` focused, with plugin manifests handled separately under
`src/capabilities/plugin.rs`.

## CLI Mode Gap

Current help exposes `review`, `fix`, `mode`, `security`, `hook`, etc.

Still missing from the v0.9.2 mode matrix:

- `sparrow audit` as repo architecture/stub audit command
- `sparrow test [--fix]`
- `sparrow commit`
- `sparrow release prep`
- global `--dry-run`
- `sparrow run --patch`
- `sparrow run --plan-first`
- `sparrow mode builder`

`review` already exists as a read-only diff review command.

## Security Gap

Security scanner has dangerous command patterns including `rm -rf` in
`src/security.rs`, but the planned S2 forced approval guard for destructive
`exec` strings is not yet visible in `src/permissions` or `src/tools/exec.rs`.

Observed grep hits:

```text
src\security.rs:188: "rm -rf"
src\permissions\mod.rs:181: read-only blocks mutating, exec, network, destructive
```

Open v0.9.2 items:

- forced approval even in autonomous for destructive shell patterns
- `sparrow security log`
- commit preflight secret warning integrated into `sparrow commit`

## Stubs, TODOs, and Honest Gaps

`rg` for `todo!`, `unimplemented!`, `TODO`, `FIXME`, `stub`, and `placeholder`
found no production `todo!()` or `unimplemented!()` in `src`, but did find honest
stub/placeholder language:

```text
src\onboarding\enterprise.rs: IDE Integration stubs
src\runtime\mod.rs: No-op stub on Windows / non-unix targets
src\share.rs: placeholder for a demo GIF
README.md: Cloud sandboxes placeholder entries
docs\comparison.md: external memory providers are honest stubs
docs\cli-reference.md: several slash commands are placeholder workflow commands
```

Phase 1 should classify each as either acceptable/honest scope or remove/update
the claim.

## CI Audit

Current CI:

- `.github/workflows/ci.yml`
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo build --release`
  - `cargo test --release`
  - RustSec audit
- `.github/workflows/nightly.yml`
  - `cargo fmt --all --check`
  - `cargo clippy --no-deps -- -D warnings`
  - `cargo test --all-targets`
  - advisory cargo-audit
- no hyperfine benchmark job
- no binary-size regression gate
- no build-timings artifact job
- no `cargo bloat` diff

Conclusion: D4 is confirmed.

## Dependency Duplicate Audit

Command:

```text
cargo tree -d
```

Notable duplicate families:

```text
console      v0.15.11 and v0.16.3
getrandom    v0.2.17, v0.3.4, v0.4.2
hashbrown    v0.14.5, v0.15.5, v0.17.1
itertools    v0.10.5 and v0.13.0
rand         v0.8.6 and v0.9.4
rand_chacha  v0.3.1 and v0.9.0
rand_core    v0.6.4 and v0.9.5
thiserror    v1.0.69 and v2.0.18
unicode-width v0.1.14 and v0.2.0
windows-sys  v0.59.0 and v0.61.2
```

Dupes are largely transitive, but `console` is pulled both directly and through
`dialoguer`/`indicatif`.

## Performance Baseline

### Tool Availability

```text
hyperfine: installed during this run, then used
cargo bloat: installed during this run, then used
```

Fallbacks used:

- PowerShell `Stopwatch` for CLI timing
- Playwright for first paint
- `Get-Process` for RSS
- `cargo build --release --timings` for timing report

### Disk and Binary Size

```text
target directory size : 97.73 GB
repo without .git     : 107.68 GB
existing release binary              : 13,077,504 bytes
clean baseline release binary        : 13,096,448 bytes
clean baseline release PDB           : 7,401,472 bytes
clean baseline release libsparrow    : 32,860,520 bytes
```

### CLI Startup

Fallback measurement with existing `target\release\sparrow.exe`, 10 iterations,
PowerShell Stopwatch:

```text
sparrow --version avg : 34.16 ms
sparrow --version min : 29.44 ms
sparrow help avg      : 32.16 ms
sparrow help min      : 28.38 ms
```

Both are under the v0.9.2 targets of `<100 ms` and `<150 ms` on this machine,
but these are not cold-cache `hyperfine` results.

Official `hyperfine --warmup 2` baseline against the clean release binary:

```text
target\v092-release\release\sparrow.exe --version
  mean 236.5 ms ± 192.9 ms
  range 26.1 ms … 635.5 ms

target\v092-release\release\sparrow.exe help
  mean 359.4 ms ± 116.5 ms
  range 200.8 ms … 525.4 ms
```

Interpretation: local warm process execution can be very fast, but the
hyperfine benchmark is above the v0.9.2 targets on Windows and has high
variance. Startup remains a real Phase 2 target.

### Console Startup and Idle RSS

Measured by launching a temporary console on port `19442` and polling `/healthz`:

```text
HealthzOk    : True
HealthzMs    : 640.77
HealthzBody  : {"ok":true}
WorkingSetMB : 19.25 after 30s idle
```

This meets the plan's console `/healthz` target of `<800 ms` and RSS target of
`<150 MB` for this local release binary.

### WebView First Paint

Measured with Playwright against a temporary console on `?theme=white`:

```json
{
  "wallMs": 2225,
  "perf": {
    "domContentLoaded": 268.9,
    "loadEvent": 454.6,
    "paints": [
      {"name": "first-paint", "start": 284},
      {"name": "first-contentful-paint", "start": 284}
    ],
    "interactive": true,
    "transcript": false,
    "title": "Sparrow — webview console"
  },
  "consoleErrors": [],
  "errorCount": 0
}
```

First paint is under the v0.9.2 target of `<1000 ms`. A stricter TTI script
should be added in Phase 2 because this smoke check only verifies primary
controls exist after 1.5 s.

### Release Build Timing

Initial command:

```text
cargo build --release --timings
```

Result:

```text
EXIT=101
SECONDS=489.653
error: failed to remove file `C:\Sparrow\target\release\sparrow.exe`
Caused by: Accès refusé. (os error 5)
Timing report saved to C:\Sparrow\target\cargo-timings\cargo-timing-20260612T083951139Z-86e94cbb5c03e4c2.html
```

Running Sparrow processes observed:

```text
Id    Path
30248 (path unavailable)
30280 C:\Sparrow\target\release\sparrow.exe
```

The timing HTML still contains useful partial data:

```text
Fresh units : 356
Dirty units : 2
Total units : 358
Error       : 1 job failed

sparrow-cli v0.9.0 total 59.9s
frontend 20.9s (35%)
codegen  39.0s (65%)
```

Clean release build baseline remains incomplete until the running release binary
is closed or the build output path is changed.

Clean build command using a separate target dir:

```text
cargo build --release --timings --target-dir target/v092-release
EXIT=0
SECONDS=311.508
Finished `release` profile [optimized] target(s) in 5m 11s
Timing report:
target/v092-release\cargo-timings\cargo-timing-20260612T090033635Z-86e94cbb5c03e4c2.html
```

Incremental release rebuilds in the same target dir:

```text
touch src/tools/todo.rs     -> 191.405s
touch src/engine/mod.rs     -> 191.897s
```

Conclusion: D1 is severe. In release mode, touching a tool and touching the
engine both recompile the same `sparrow-cli` unit and have effectively the same
cost.

### Bloat Baseline

Command:

```text
cargo bloat --release --target-dir target/v092-release -n 30
cargo bloat --release --target-dir target/v092-release --crates -n 30
```

Top symbol-level entries:

```text
342.0KiB sparrow::async_main::async_fn$0
135.8KiB sparrow::event deserialize
117.2KiB sparrow::cli augment_subcommands
108.0KiB sparrow::engine drive_with_inject
106.4KiB sparrow::engine drive_with_inject
 89.5KiB sparrow::config::providers::provider_registry
```

Top crate-level entries:

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

`.text` section: 7.9 MiB. File size: 12.5 MiB by cargo-bloat; Windows file size
observed as 13,096,448 bytes.

### Build/Check/Test Baseline

Commands:

```text
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

Observed:

```text
cargo check --all-targets: pass, 10.13s
cargo test --all-targets: pass, all observed test binaries green
cargo clippy --all-targets -- -D warnings: pass, 39.14s
cargo fmt --all -- --check: pass after `cargo fmt --all`
```

## Confirmed Debt Matrix

| Debt | Status | Evidence |
|---|---|---|
| D1 monocrate/build coupling | confirmed | one workspace member, 45,971 Rust LOC |
| D2 engine god module | confirmed | `src/engine/mod.rs` 3,551 lines |
| D3 console monofile | confirmed | `console.html` 5,532 lines |
| D4 no perf CI | confirmed | CI has no hyperfine/bloat/size/timings gate |
| D5 boot discovery risk | partially confirmed | background spawn exists; full path trace still needed |
| D6 no competitive intel | confirmed | no `sparrow-intel`, no `/intel/*`, no matrix TOML |
| D7 no builder profile | confirmed | `mode simple|pro|auto`, no `builder` |
| D8 no fast-start mode | confirmed | no `--fast` on console/launch |

## Phase 1 Inputs

Recommended order before implementing new v0.9.2 features:

1. Decide whether the uncommitted v0.9.1 work is the baseline or should be
   committed/tagged first. The v0.9.2 plan says v0.9.1 must be delivered before
   this chantier.
2. Fix or consciously defer `cargo fmt` failure in `src/extras.rs`.
3. Close the running release `sparrow.exe` process before measuring release build
   again, or use a separate target dir for timing.
4. Add a CI stub grep for unassumed `todo!()`/`unimplemented!()` after classifying
   honest stubs.
5. Add repeatable perf scripts before optimization:
   - CLI startup
   - console healthz
   - Playwright first paint/TTI
   - binary size
   - dependency duplicate snapshot

## Phase 0 Command Log

```text
rg --files
git status --short
Get-ChildItem -Force
Get-Content -Raw PLAN_v0.9.2.md
Get-Content -Raw PLAN_v0.9.1.md
Get-Content -Raw CHANGELOG.md
rg -n todo/unimplemented/stub markers
cargo metadata --no-deps --format-version 1
cargo tree -d
cargo build --release --timings
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
target\release\sparrow.exe --version
target\release\sparrow.exe help
temporary console /healthz measurement on port 19442
temporary console Playwright first paint measurement on port 19443
```

## Phase 0 Status

Status: pass with documented limitations.

Pass:

- audit document produced and made trackable via `.gitignore`
- code architecture mapped
- CI/perf gaps confirmed
- tests and clippy are green
- startup/console/browser smoke baselines collected
- release build, incremental build, hyperfine and cargo-bloat baselines collected

Limitations:

- `hyperfine` on Windows shows high startup variance; Phase 2 should add a
  repeatable CI runner baseline.
- `cargo-bloat --crates` triggered an additional build because the two bloat
  commands were started in parallel and contended on Cargo locks; final output
  is still valid.

The next phase is Stabilization. The formatter failure is already fixed; the
remaining baseline decision is how to close the uncommitted v0.9.1 work before
large v0.9.2 feature changes.
