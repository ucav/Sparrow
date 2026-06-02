# Security audit (`sparrow security audit`)

Sparrow ships with a local security auditor that inspects the active config and
hooks for unsafe configuration. It is **read-only**: it never modifies files or
config and never reaches the network.

## CLI

```bash
sparrow security audit          # human-readable summary
sparrow security audit --json   # stable JSON for tooling
```

The summary line is of the form:

```
Security Audit: score 85/100 | 1 critical, 2 warnings, 0 info
```

Findings have a stable shape:

```json
{
  "severity": "Critical" | "Warning" | "Info",
  "category": "permissions" | "gateway" | "tools" | "plugins" | "hooks" | "secrets" | "sandbox",
  "message": "...",
  "detail": "...",
  "recommendation": "..."
}
```

## What is checked

- **permissions**: empty denied-paths list, autonomous mode without tool deny list.
- **gateway**: any enabled `telegram`/`discord`/`slack` surface with no `allow_users`,
  or `email` surface with no `allowed_to`. Wildcard senders are reported as
  critical.
- **tools**: dangerous tools (`exec`, `terminal`, `destructive`) not in the
  permissions deny list.
- **plugins**: installed plugins under `<config_dir>/plugins/*/plugin.toml` that
  do not declare an `allowlist`.
- **hooks**: enabled hooks whose `command` contains known dangerous patterns
  (`rm -rf`, `curl |`, `wget |`, `eval `, `exec(`, `powershell -enc`,
  `base64 -d`), or where the redaction filter spots a secret in the command
  string.
- **secrets**: files in the current directory whose contents contain
  provider-specific API-key prefixes (`sk-ant-`, `ghp_`, `gho_`, `xai-`,
  `nvapi-`, `hf_`, `gsk_`, …).
- **sandbox**: when `defaults.sandbox = "local"` and permission mode is
  `autonomous`, with `exec`/`terminal` not denied.

## Scoring

The base score is `100`. Each `Warning` subtracts 5 points; each `Critical`
subtracts 15. The score is clamped to `0`.

## WebView

`GET /security` returns the same `SecurityAudit` JSON object, suitable for
rendering a security panel.

## Status

**Alpha.** Coverage is intentionally narrow — false positives are rare but the
scanner is not a substitute for a full security review. Add new checks by
extending `src/security.rs`.
