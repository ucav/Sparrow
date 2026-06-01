# Roadmap

Sparrow uses evidence-based status labels:

- **Stable**: compiled, tested, and used by at least one runnable path.
- **Alpha**: implemented and tested locally, but needs more real-world validation.
- **Partial**: meaningful code exists, but the end-to-end promise is not fully wired.
- **Experimental**: adapter, shell, or prototype exists.
- **Planned**: not implemented.

## Current Alpha

| Area | Status | Notes |
|---|---:|---|
| Core event model | Stable | `Event` is the load-bearing contract across surfaces and recorder. |
| Brain/provider abstraction | Stable | Unified `Brain`, `BrainRequest`, `BrainEvent`, `ToolSpec`. |
| Engine loop | Stable | Think/tool/observe loop with routing, tokens, redaction, event emission; 91 tests pass. |
| Provider registry | Stable | 30+ providers; boot-time auto-discovery for env-keyed providers; 24h SQLite cache. |
| Model discovery | Alpha | `GET /v1/models` for OpenAI-compat; `/api/tags` for Ollama; Anthropic `/v1/models`. |
| Ollama adapter | Alpha | Native `/api/chat` streaming path; boot-time discovery for local models. |
| OpenAI-compatible adapter | Alpha | Used for NVIDIA, Groq, DeepSeek, Gemini, and 25+ other providers. |
| Anthropic adapter | Alpha | Streaming tool-use IDs fixed and tested via build suite. |
| Routing | Alpha | Budget-aware fallback, local/free preference, tool/vision penalties. `model --set` writes config. |
| Autonomy | Stable | 15-combination matrix; continuous float dial via `--autonomy 0.7`. |
| Memory | Alpha | SQLite persistence and redaction tests. |
| Checkpoint/rewind | Alpha | Git refs/stash implementation; `diff` and `prune` sub-commands added. |
| TUI | Alpha | Terminal cockpit connected to engine; input → drive loop working. |
| WebView console | Alpha | Local HTTP/WebSocket console on port 9339; config persisted on write. |
| Gateway WebSocket | Alpha | Message response roundtrip tested on port 9338; cron scheduler wired. |
| Telegram/Discord/Slack | Partial | Real transport implementations exist; account-token E2E validation still needed. |
| Extra gateway transports | Experimental | WhatsApp/Signal/Email/Feishu adapters present; return explicit unsupported errors. |
| Swarm orchestrator | Alpha | Planner/Coder/Verifier with real tool dispatch; REWORK→PASS gate integration-tested. |
| Skills/Curator | Alpha | Filesystem skills and relevance path; auto-learn from successful runs. |
| MCP client | Alpha | stdio/HTTP client surface; `mcp add --command` persists server config. |
| Scheduler/recorder/replay | Alpha | Runtime cron loop; transcript recording in all run modes; replay + re-execute. |
| Profiles | Alpha | `profile use` persists active profile; `--profile` flag loads alternate config dir. |
| Auth | Alpha | `auth add` stores keys via rpassword; keychain → encrypted file → env priority chain. |
| `sparrow daemon` | Alpha | Headless SparrowRuntime with cron, interrupt (CancellationToken), TCP API on 9337. |
| Cloud sandboxes | Experimental | Docker backed; Modal/Daytona/Vercel return explicit unsupported errors. |
| Hardened sandbox | Partial | Linux: firejail/bwrap/unshare; Windows/macOS: path-boundary only (documented). |
| Browser/LSP/Image/TTS | Experimental | Tool shells exist; tree-sitter parser integrated; full backends are future work. |
| Python kernel | Alpha | Persistent subprocess via `python_rpc` tool; full RPC channel planned for v2. |
| Rate limiter | Alpha | Per-provider token-bucket rate limiter in `runtime/ratelimit.rs`. |
| Sessions | Alpha | Session tracking in `runtime/session.rs`. |
| Release packaging | Planned | CI definitions exist; public release artifacts not yet published. |

## Near-Term Priorities

1. **Gateway E2E validation**
   - Token-backed manual test guides for Telegram, Discord, and Slack.
   - Mock gateway tests for command routing and response delivery.

2. **CI/release hardening**
   - Publish `v0.1.0-alpha` binaries with checksums for Linux/macOS/Windows.
   - Ensure `cargo fmt`, `clippy -D warnings`, and all 91 tests stay green.

3. **Hardened sandbox on non-Linux**
   - Implement Windows Job Objects (`CREATE_NEW_PROCESS_GROUP` + CPU/memory limits) as a real hardening layer.
   - macOS: use `sandbox-exec` profile or seatbelt.

4. **Live `sparrow status`**
   - Surface active run IDs from `SparrowRuntime::active_runs` map.
   - Show per-session cost accumulator.

5. **Public GitHub polish**
   - Repository description, website, topics, first alpha release tag.
   - Screenshots / GIF for WebView console, TUI cockpit, and NDJSON stream.

## Beyond Alpha

- OAuth/device-flow provider setup (Qwen, GitHub Copilot).
- Persistent Python kernel with real IPC (ZeroMQ or unix socket).
- Browser automation backend (Playwright/CDP).
- Real image generation / TTS provider integration.
- Plugin system for providers, tools, and surfaces.
- Public website and installation script tested on Windows/macOS/Linux.
