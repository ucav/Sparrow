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
| Engine loop | Alpha | Think/tool/observe loop exists with routing, tokens, redaction, and event emission. |
| Provider registry | Alpha | Registry covers many providers; runtime auto-discovers configured env keys. |
| Ollama adapter | Alpha | Native `/api/chat` streaming path exists. |
| OpenAI-compatible adapter | Alpha | Used for NVIDIA and similar providers. |
| Anthropic adapter | Alpha | Streaming tool-use IDs fixed and tested via build suite. |
| Routing | Alpha | Budget-aware fallback, local/free preference, tool/vision penalties. |
| Autonomy | Stable | 15-combination matrix covered in tests. |
| Memory | Alpha | SQLite persistence and redaction tests. |
| Checkpoint/rewind | Alpha | Git refs/stash-based implementation with integration coverage. |
| TUI | Alpha | Terminal cockpit exists; more visual polish needed. |
| WebView console | Alpha | Local HTTP/WebSocket console tested on port 9339. |
| Gateway WebSocket | Alpha | Message response roundtrip tested on port 9338. |
| Telegram/Discord/Slack | Partial | Real transport implementations exist; account-token E2E validation still needed. |
| Extra gateway transports | Experimental | Adapters present; several return explicit unsupported errors rather than fake success. |
| Swarm orchestrator | Partial | Planner/Coder/Verifier loop exists; stronger landed-only-after-PASS proof remains needed. |
| Skills/Curator | Alpha | Filesystem skills and relevance path exist. |
| MCP client | Alpha | stdio/HTTP client surface exists. |
| Scheduler/recorder/replay | Alpha | Runtime pieces and transcript tests exist. |
| Cloud sandboxes | Experimental | Modal/Daytona/Vercel/Singularity placeholders. |
| Browser/LSP/Image/TTS | Experimental | Tool shells exist; full backends are future work. |
| Release packaging | Planned | CI definitions exist; public release artifacts are not published yet. |

## Near-Term Priorities

1. **Public GitHub polish**
   - Add repository description, website, topics, and first alpha release in GitHub settings.
   - Add screenshots or a short GIF for WebView console, TUI, and JSON stream.
   - Keep README status aligned with `docs/AUDIT.md`.

2. **CI/release hardening**
   - Keep CI on `master` and `main`.
   - Make `cargo fmt`, `clippy -D warnings`, build, and tests mandatory.
   - Publish `v0.1.0-alpha` binaries with checksums.

3. **M0 acceptance**
   - Keep `examples/m0_hello.sh` runnable against real Ollama.
   - Add a deterministic no-network mock acceptance path for CI.

4. **Swarm proof**
   - Add an integration test proving coder diffs are not landed until verifier PASS.
   - Record REWORK/PASS events in transcripts.

5. **Gateway proof**
   - Add token-backed manual test guides for Telegram, Discord, and Slack.
   - Add mock gateway tests for command routing and response delivery.

6. **Sandbox reality**
   - Replace placeholder cloud sandbox output with either real adapters or explicit unsupported errors.
   - Add Linux `local-hardened` acceptance tests for path escape/network denial.

## Beyond Alpha

- Persistent Python kernel for subagents.
- Browser automation backend.
- Real image generation/TTS provider integration.
- OAuth/device-flow provider setup.
- Multi-profile isolation end-to-end.
- Plugin system for providers, tools, and surfaces.
- Public website and installation script tested on Windows/macOS/Linux.
