# Security Policy

## Reporting a Vulnerability

Please report security issues **privately** via
[GitHub Security Advisories](https://github.com/ucav/Sparrow/security/advisories/new)
(Security → Report a vulnerability). Do **not** open a public issue for a
suspected vulnerability.

**Never paste API keys, tokens, `.env` contents, or `auth.enc` into an issue,
advisory, or transcript.** If a report needs a credential to reproduce, say so
and we will arrange a safe channel — do not share the secret itself.

### What to expect
- Acknowledgement within a few days (this is a small, best-effort project).
- A fix or mitigation plan, and credit in the release notes if you'd like it.
- Coordinated disclosure: please give us a reasonable window before going public.

## In Scope

We especially want reports about:
- **Credential leakage** — keys/tokens reaching logs, transcripts, model context, HTML, or config in plaintext.
- **Prompt injection** that escalates into tool execution, file writes, or data exfiltration.
- **Unsafe tool execution / sandbox escape** — commands reaching outside the workspace or bypassing the permission/autonomy gate.
- **Arbitrary file read/write** outside the intended workspace, including denied-path bypasses.
- **SSRF** via the web tools (e.g. reaching cloud metadata or internal hosts).
- **Install-script issues** (`install.sh`, `install.ps1`) — tampering, missing integrity checks, path injection.
- **Supply-chain** — dependency or build-pipeline vulnerabilities.

## Security Model

Sparrow is **local-first with zero telemetry by default** — nothing leaves your
machine unless you explicitly call a cloud provider.

### Secrets
- **Never in logs, transcripts, or model context.** A redaction filter runs on outbound events and stored memory.
- Credentials are stored in the OS keychain (where available), then an encrypted file (`auth.enc`), then environment variables.
- API keys are never echoed, stored in HTML, or written to config files in plaintext.

### Autonomy & permission gates
- Modes: **Supervised** (asks before exec/mutate), **Trusted**, **Autonomous**.
- ⚠️ **The default is `Trusted`**, which auto-runs `exec` and network tools (the user is notified but not prompted). `Mutating`/`Destructive` escalation: `Destructive` is **Denied** in Supervised and **Asked** in Trusted/Autonomous. Choose `Supervised` in config if you want a prompt before every command.
- A checkpoint is taken before mutating/exec/destructive actions; `sparrow rewind` restores.

### Sandboxing (be precise about what this gives you)
- **Mutating and exec actions** run under a configurable sandbox; the default is `local-hardened`.
- **Always on, every platform:** the working directory is confined to your workspace root, and a denied-path guard blocks known secret paths (`.git`, `.env`, `.ssh`, `id_rsa`, `id_ed25519`). The exec tool also scans the command string for literal references to those paths. **This is defence-in-depth, not isolation** — a shell command can still read what your user account can via globbing/expansion.
- **Linux, when `firejail` or `bwrap` is installed:** commands are additionally wrapped so the filesystem is scoped to the workspace and the **network is denied**. When neither tool is present, Sparrow falls back to the in-process path checks above (no namespace/network isolation).
- **For strong isolation, use the `docker` or `ssh` sandbox backend.**
- Cloud sandboxes (Modal, Daytona, Vercel, Singularity) are **experimental**.

### Autonomy hard stops
- Budget exceeded → halt + notify + checkpoint
- Repeated tool failure → halt + notify + checkpoint
- Destructive operation under Supervised → denied

### Audit trail
- Every approval decision and tool call is recorded in the run transcript (append-only, shareable, replayable).

### Supply chain
- Pinned dependencies (`Cargo.lock`); `rustsec/audit-check` runs in CI.
- Release binaries ship a **SHA256 checksum** (`<artifact>.sha256`); the installers verify it. **Cryptographic signing (GPG/sigstore) is planned, not yet in place** — verify checksums, and don't assume signed provenance.

## Supported Versions

| Version | Supported |
|---|---|
| 0.9.x (current) | ✅ |
| ≤ 0.8.x | ❌ |
