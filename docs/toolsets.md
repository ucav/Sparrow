# Toolsets

Sparrow classifies every known tool with metadata:

- `toolset`
- `risk`
- `requires_auth`
- `mutates_files`
- `network`
- `exec`

Named toolsets:

```text
safe, web, file, terminal, media, debug, skills, memory, session_search, mcp, gateway
```

## Surfaces

Tool visibility can be filtered for surfaces:

- `cli`
- `tui`
- `webview`
- `gateway`
- `subagent`

Gateway defaults are conservative: file mutation, terminal execution, destructive
tools, and exec tools are hidden.

## CLI

```bash
sparrow tools list
sparrow tools list --surface gateway
sparrow tools enable <tool>
sparrow tools disable <tool>
```

`enable` and `disable` update the existing permission allow/deny rules.

## WebView

The local console exposes `GET /tools` for a toolsets panel.
