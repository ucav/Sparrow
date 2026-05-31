# GitHub Polish Checklist

Use this checklist for repository settings that cannot be fully represented in source files.

## Repository About

Description:

```text
Local-first Rust agent cockpit with model routing, WebView console, gateway surfaces, replay, memory, and rollback safety.
```

Website:

```text
https://github.com/ucav/Sparrow
```

Topics:

```text
ai-agent
agentic
cli
rust
llm
ollama
nvidia
openai
tui
webview
automation
model-routing
local-first
developer-tools
```

Enable:

- Issues
- Discussions
- Projects if roadmap tracking is desired
- Security advisories

## Pinned Visuals

Add these to the README or release notes when screenshots are available:

- WebView console with route + tokens.
- TUI cockpit.
- `sparrow --json run ...` event stream.
- Gateway WebSocket `/status` roundtrip.

## First Release

Recommended first public tag:

```bash
git tag v0.1.0-alpha
git push origin v0.1.0-alpha
```

Before tagging, confirm:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release
```

Release title:

```text
Sparrow v0.1.0-alpha: routing console and gateway kernel
```

Release notes should say clearly:

- This is alpha software.
- Core build/test/routing/WebView/gateway paths are working.
- Extra transports, cloud sandboxes, image/TTS/LSP, and release packaging are still experimental.
