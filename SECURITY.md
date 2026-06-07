# Security Policy

## Responsible Disclosure

If you discover a security vulnerability in Sparrow, please report it via email to the repository maintainers. Do not open a public issue.

## Security Model

Sparrow is built on a **trust-first** security model:

### Secrets

- **Never in logs, transcripts, or model context.** A redaction filter runs on all outbound events and stored memory.
- Credentials are stored in the OS keychain (where available), then an encrypted file (`auth.enc`), then environment variables.
- API keys are never echoed, stored in HTML, or written to config files in plaintext.

### Sandboxing

- **Mutating and exec actions** run under a configurable sandbox.
- Default: `local-hardened` (Linux namespaces + seccomp, filesystem allow-list, network deny by default).
- Also supported: Docker, SSH remote, serverless (Modal, Daytona, Vercel).
- **Sandbox escape signals** trigger a hard stop and notify the user.

### Autonomy Hard Stops

- Budget exceeded → halt + notify + checkpoint
- Sandbox escape signal → halt + notify + checkpoint
- Repeated tool failure → halt + notify + checkpoint
- Destructive operations → Deny in Supervised, Ask in Trusted/Autonomous

### Audit Trail

- Every approval decision and tool call is recorded as an event in the run transcript.
- Transcripts are append-only, shareable, and replayable.
- Full reproducibility: same inputs, same model, same seed → same output.

### Supply Chain

- Pinned dependencies (`Cargo.lock`)
- `cargo audit` in CI
- Reproducible builds
- Signed release binaries with checksums

## Supported Versions

| Version | Supported |
|---|---|
| 0.3.x (current) | ✅ |
| 0.2.x             | ❌ |
| 0.1.x             | ❌ |
