# Plugins

Sparrow plugins are local bundles discovered from `.sparrow/plugins` or the
profile config directory under `plugins`.

## Manifest

Each plugin must provide `.sparrow-plugin/plugin.toml` or
`.sparrow-plugin/plugin.json`.

```toml
name = "demo"
version = "0.1.0"
description = "Demo plugin"

[[commands]]
name = "hello"
description = "Say hello"
body = "hello from plugin"

[[skills]]
name = "review"
path = "skills/review/SKILL.md"

[[hooks]]
name = "preflight"
kind = "command"
command = "cargo check"
```

## Namespaces

Plugin commands and skills are exposed as namespaced slash commands:

```text
/demo:hello
/demo:review
```

The namespace prevents collisions with built-in commands, user commands, and
regular skills.

## Security Scan

The plugin scanner blocks dangerous command hooks such as recursive deletion,
encoded PowerShell, shutdown commands, or obvious exfiltration helpers. Blocked
plugins are not exposed as slash commands.

## CLI

```bash
sparrow plugins list
sparrow plugins install <local-dir-or-git-url>
sparrow plugins install <local-dir> --allow
sparrow plugins rm <name>
```

The WebView exposes `GET /plugins` for a local plugin browser with scanner
warnings.
