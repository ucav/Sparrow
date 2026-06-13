# Sparrow — Trust Audit v0.9.2 « The Ring »

**Date:** 2026-06-14
**Auditor role:** senior Rust maintainer / security engineer / release manager / product auditor
**Branch:** `chore/trust-audit-v092`
**Method:** evidence-based. A claim is only "proven" when backed by compiled code, a passing test, or a smoke run on this machine. Anything else is labelled Partial / Experimental / Planned / Unverifiable-offline.

---

## 1. Executive summary

Sparrow is a **real, substantial Rust project**, not vaporware. The workspace compiles, the test suite is large and green on this host (39 test binaries, 0 failures), the security thinking is above average (SSRF guard, `SecretString`, permission/autonomy gates, redaction), and the product vision (local-first cockpit, routing, replay, rewind, multi-surface) is coherently implemented at the kernel level.

The problem is **not capability — it is calibration.** The public surface (README, SECURITY.md, comparison table) systematically over-labels. Twenty rows are marked **✅ Stable**, including transports and a GitHub Action that have no end-to-end evidence. SECURITY.md still advertises a "seccomp + namespaces, network-deny by default" sandbox whose strongest form only engages when `firejail`/`bwrap` happen to be installed. `package.json` and `docs/AUDIT.md` still carry the **v0.3.6** version. One architecture link (`src/event.rs`) is broken by the workspace refactor. The installers **auto-launch an agent** by default and do **not verify checksums**, even though the release pipeline already produces `.sha256` files.

None of this is fraud — it is the gap between an ambitious solo build and what a sceptical senior engineer can verify in five minutes. This audit closes that gap: downgrade unproven labels, make the safety story honest, harden the install path, fix the Action, and tighten CI so the labels can't drift again.

**Verdict:** ambitious and real, but the public claims out-run the evidence. Corrected in this branch.

---

## 2. What is real and proven (verified on this host)

| Area | Evidence |
|---|---|
| Workspace builds | `cargo build` and `cargo build --release` succeed (6 crates + bin). |
| Test suite | `cargo test --all-targets` → 39 test binaries, **0 failures** (incl. `security_audit.rs`, `sandbox_policy.rs`, `integration.rs` 98 tests). |
| Clippy clean | `cargo clippy --all-targets -- -D warnings` passes. |
| SSRF defence | `crates/sparrow-tools/src/search_and_web.rs` — v4/v6 private ranges, metadata IP, CGNAT, IPv4-mapped, redirect re-validation, now + DNS-rebinding pin. Unit-tested. |
| Secret handling | `secrecy::SecretString` for credentials; env-var sourcing; no secret logging found. |
| Permission/autonomy gate | `crates/sparrow-config/src/permissions/`, `src/autonomy/` — modes Supervised/Trusted/Autonomous, per-risk decisions, hard stops. Unit-tested. |
| Sandbox path confinement | `LocalSandbox` confines workdir to workspace root + denied-path arg checks. Tested. |
| No production stubs | Only `todo!()` references are inside the stub-detector `src/repo_audit.rs` and its tests. |
| Screenshot asset | `docs/screenshots/webview-captain.png` exists (real file). |
| Release checksums | `.github/workflows/ci.yml` release job emits `<artifact>.sha256` and uploads it. |

## 3. What is partially implemented

| Area | Reality |
|---|---|
| Provider routing | Engine + registry real; **only Ollama + NVIDIA exercised**. Other providers depend on user credentials + provider uptime — not E2E-verified here. README's "38 providers" overstates a registry of ~34 keyed entries. |
| Gateways (Telegram/Discord/Slack) | Transport code present; **no E2E token round-trip proven**. README status says "Partial" but the comparison table marks them ✅ — contradiction. |
| GitHub Action | `action.yml` exists and the `github review --dry-run` path needs no `gh`; but the install step was **broken** (installs crate `sparrow` instead of `sparrow-cli`) and there is no smoke test. Not Stable. |
| Browser / computer-use | Playwright driver + `bwrap` wrapper exist; correctly labelled Alpha. |

## 4. What is experimental

- Extra transports (WhatsApp, Signal, Email, Feishu, WeCom, QQ, Teams) — adapters present, unproven. Correctly labelled 🧪.
- Cloud sandboxes (Modal, Daytona, Vercel, Singularity) — placeholder/vendor-shell entries. Correctly labelled 🧪.
- Voice (`speak`/`transcribe`) — present but marked ✅ in the comparison table without E2E evidence.

## 5. What is planned but not ready

- Package-manager distribution (Homebrew tap, Scoop bucket, winget) — README already hedges "manifests ready, publishing in progress". Keep hedged; do not present as one-command-available.
- crates.io / docs.rs availability — **cannot be verified offline.** Badges will self-report; if `cargo install sparrow-cli` does not yet resolve, the install section must say so. Flagged for human verification.

## 6. Broken or suspicious claims

1. **20× "✅ Stable"** in the README status table — several (WebView console, TUI, GitHub Action, Gateway, Media tools) lack E2E evidence. Downgrade to Alpha/Partial.
2. **Comparison vs status contradiction** — Telegram/Discord/Slack and Voice marked ✅ in the comparison table but Partial/Experimental in the status table.
3. **"38 providers"** — registry has ~34 keyed entries; only 2 exercised. Soften to "30+ registry entries (Ollama + NVIDIA verified)".
4. **SECURITY.md sandbox claim** — "Linux namespaces + seccomp, network deny by default" presented as the default. True only when firejail/bwrap is installed; otherwise falls back to in-process path checks with no network isolation. Made honest.
5. **Default autonomy is `Trusted`** — auto-allows `exec` and `network` with no prompt (notify only). Neither README nor SECURITY.md states this clearly. Surfaced.
6. **Broken link** — README "Load-bearing contracts" points to `src/event.rs`, which moved to `crates/sparrow-core/src/event.rs` in the workspace refactor.

## 7. Version inconsistencies

| File | Was | Should be |
|---|---|---|
| `package.json` | `0.3.6` | `0.9.2` |
| `docs/AUDIT.md` | "after the v0.3.6 finalisation pass" | v0.9.2 « The Ring » |
| `Cargo.toml` | `0.9.2` | ✅ already correct |
| `CHANGELOG.md` | `[0.9.2] … The Ring` | ✅ already correct |
| `rust-toolchain.toml` / README badge | `1.96` / `Rust 1.96+` | ✅ consistent |

## 8. Install / security trust issues

- **Auto-launch by default** — both `install.sh` and `install.ps1` execute `sparrow launch` immediately after install. For a trust-sensitive agent that can run shell commands, the default must be **no auto-launch**; launching is opt-in (`--launch` / `-Launch`).
- **No checksum verification** — installers download a release binary and run it with no integrity check, despite `.sha256` files existing in releases. Added SHA256 verification with safe fallback.
- **Installers do not print the resolved binary path clearly / next step** — partially present; standardised.

## 9. GitHub Action issues

- Install step: `cargo install --git … sparrow` → wrong crate name. Fixed to `--git … --package sparrow-cli --bin sparrow`.
- `--branch "${{ inputs.sparrow-version }}"` fails for **tags** (e.g. `v0.9.2`). Made ref-type tolerant (try branch, then tag/rev).
- No smoke test guarding the install command → can silently regress. Added a CI check asserting `action.yml` carries the correct package/bin.
- Must not be labelled Stable without an E2E/smoke run → downgraded.

## 10. README credibility issues

Covered in §6. Net effect: confident but honest. Ambition retained; every Stable label now maps to evidence, contradictions removed, broken link fixed, safety model stated plainly (incl. the Trusted-default caveat).

## 11. CI / test gaps

- CI runs `cargo test --release`, **not** `cargo test --all-targets` (the documented contributor command). Aligned to `--all-targets`.
- No **install smoke** (`cargo install --path .` → `sparrow --version` / `sparrow help`).
- No **stub gate** (fail on `todo!()`/`unimplemented!()` in shipping code).
- No **install-script syntax check** (`bash -n`, PowerShell parser).
- No **action.yml correctness check**.
All added in a dedicated `trust-gates` job (kept lightweight).

## 12. Release / signature / checksum gaps

- Release pipeline produces SHA256 sidecars ✅ but **nothing consumed them** until now (installers ignored them).
- No GPG/sigstore signing of release artifacts — acceptable for v0.9.x; **document as Planned**, do not claim signed releases.

## 13. Performance / build friction

- `src/main.rs` (2.1k lines), `src/engine/mod.rs` (3.7k lines), `console.html` (5.8k lines) are large single units — real maintenance debt (see §Architecture in this file). Documented, not refactored in this pass (too risky for a trust audit).
- Release build is heavy (cross + musl/darwin/msvc matrix). Per project memory, full rebuild ≈190s; tracked as a known perf follow-up, not a regression.

## 14. Prioritized fix list (this branch)

1. **[done]** Version coherence — `package.json`, `docs/AUDIT.md` → v0.9.2.
2. **[done]** README — downgrade unproven Stable rows, reconcile comparison vs status, soften "38 providers", fix `src/event.rs` link, state Trusted-default + honest safety model.
3. **[done]** SECURITY.md — honest sandbox wording, explicit scope, supported versions, disclosure process, key-safety warning.
4. **[done]** Installers — no auto-launch by default + `--launch`/`-Launch`; SHA256 verification with safe fallback; print binary path + next step.
5. **[done]** `action.yml` — correct `--package sparrow-cli --bin sparrow`; tag/branch tolerant.
6. **[done]** `.github/FUNDING.yml` — drop the self-referential `custom:` link; `github: [ucav]` only.
7. **[done]** CI — `trust-gates` job: `--all-targets`, install smoke, stub gate, install-script syntax, action.yml check.
8. **[carried]** Architecture debt (main.rs/engine god-modules, console monofile) — documented, tracked in ROADMAP; no risky refactor in this pass.

---

## Architecture debt (tracked, not fixed here)

| Item | Size / signal | Recommendation |
|---|---|---|
| `src/main.rs` dispatcher | ~2,078 lines | Extract per-command handlers into `src/cmd_handlers/` (already partially done) — incremental. |
| `src/engine/mod.rs` | ~3,737 lines | Split the agent loop, sandbox wiring, and tool registry assembly into submodules. |
| `console.html` | ~5,842 lines monofile | Modularise JS/CSS; out of scope for a Rust trust pass. |
| Provider registry | ~34 entries, 2 verified | Mark unverified providers in-code or in docs; add per-provider E2E behind a feature/credential gate. |
| Partial E2E | gateways, Action, providers | Build credential-gated smoke tests before promoting any to Stable. |

## Remaining risks / needs human validation

- **crates.io / docs.rs / Homebrew / Scoop / winget availability** — cannot verify offline. Confirm each badge resolves before advertising one-command installs.
- **Linux `HardenedSandbox` path** — `#[cfg(target_os="linux")]`; not compiled on the Windows dev host. Needs a Linux CI run.
- **S1 (product decision):** the default autonomy `Trusted` auto-runs exec/network. Either ship `Supervised` as default or keep `Trusted` and rely on the now-honest docs. Maintainer's call.
- **No artifact signing** — releases are checksummed but unsigned.
