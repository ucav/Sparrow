# Design: native Landlock sandbox backend (#10c) — PLANNED, not implemented

Status: **design only.** This documents the intended approach honestly; no code
is claimed. Tracked as a roadmap item.

## Why

Today's `local-hardened` backend (see `crates/sparrow-config/src/sandbox/mod.rs`)
gives real isolation **only when `firejail` or `bwrap` is installed**, and
otherwise falls back to in-process path checks with no kernel-enforced
filesystem or network confinement. Depending on an external binary being present
is fragile: most users won't have firejail/bwrap, so the strongest claim in the
docs is conditional.

[Landlock](https://docs.kernel.org/userspace-api/landlock.html) is an
**unprivileged, in-process** Linux LSM (kernel ≥ 5.13, mature ≥ 5.19) that lets a
process restrict *itself* — no setuid helper, no external binary, no root. That
makes it the right primitive for "scoped to the workspace by default" that
actually engages on a normal machine.

## Approach

Add a `LandlockSandbox` alongside `HardenedSandbox`, selected for `local-hardened`
on Linux when the kernel supports Landlock, falling back to the firejail/bwrap
wrapper, then to `LocalSandbox` — strictly monotonic, never weaker.

1. **Crate:** use the `landlock` crate (safe bindings over the syscalls) rather
   than raw `prctl`/`landlock_*`.
2. **Filesystem ruleset:** allow read+write+exec under the workspace root; allow
   read+exec on the system dirs needed to run tools (`/usr`, `/bin`, `/lib`,
   `/lib64`, `/etc/ld.so.cache`); deny everything else — in particular the
   denied paths (`.ssh`, `.env`, …) become unreachable at the kernel level, which
   makes the heuristic `command_touches_denied_path` guard a true backstop rather
   than the front line.
3. **Apply point:** restrict in the child between `fork` and `exec`
   (`Command::pre_exec`) so the restriction covers the spawned shell and all its
   descendants, and the parent agent process stays unrestricted.
4. **Network:** Landlock gained TCP connect/bind scoping in kernel 6.7. Where
   available, deny outbound connect by default (matching `allow_network = false`);
   on older kernels, document that network-deny still requires the bwrap path.
5. **Capability probe:** query the supported Landlock ABI at startup; degrade
   gracefully (best-effort enforcement of whatever the running kernel supports,
   reported in `sparrow doctor`).

## Testing

Mirror the `sandbox-linux` CI job: on a Landlock-capable runner, assert that a
command can write inside the workspace but **cannot** read `$HOME/.ssh/id_rsa`
(placed outside the root), and — on kernel ≥ 6.7 — that an outbound connect is
refused. Gate with `#[cfg(target_os = "linux")]` + a runtime ABI probe so it
skips cleanly on unsupported kernels.

## Why not now

Landlock is Linux-only and kernel-version-sensitive; it cannot be compiled or
exercised on the Windows dev host, and getting the ruleset wrong silently
under- or over-restricts. It deserves its own change with a Landlock-capable CI
runner and the network-scoping ABI gate — not a rushed addition at the tail of a
multi-point pass. The `sandbox-linux` CI job added in this cycle (#10b) is the
foundation it will build on.
