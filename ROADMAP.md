# Roadmap

Sparrow uses evidence-based status labels:

- **Stable**: compiled, tested, and used by at least one runnable path.
- **Alpha**: implemented and tested locally, but needs more real-world validation.
- **Partial**: meaningful code exists, but the end-to-end promise is not fully wired.
- **Experimental**: adapter, shell, or prototype exists.
- **Planned**: not implemented.

## v0.2.0 snapshot (2026-06-02)

The autonomy mega-prompt phases 1–13 have landed. Everything in the README
status table moved to **Stable** except `Provider routing` and
`First-run setup`, which honestly depend on external runtime (tokens, real
provider responses).

See `CHANGELOG.md` for the per-phase breakdown and `docs/comparison.md` for
where Sparrow now matches / still lags Claude Code, Hermes Agent, and
OpenClaw.

### Next milestones

- **v0.3.0**: wire real external memory providers (mem0, honcho,
  supermemory), publish signed cross-platform release artifacts, package the
  VS Code extension, add the gateway run-registry to the WebSocket API
  surface so the console can abort in one click.
- **v0.4.0**: in-engine compaction tied to per-model context window (instead
  of the global 120k-char default), automatic `MEMORY.md` curation, and the
  agent-marketplace browser in the WebView.

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
| Routing | Stable | Budget-aware fallback, capability-aware scoring (caps inferred from model name), tier-aware latency, `model --set` refreshes base_url/adapter. Model-assisted classification for ambiguous tasks. |
| Autonomy | Stable | 15-combination matrix; continuous float dial via `--autonomy 0.7`. |
| Memory | Stable | SQLite persistence + redaction; Distiller wired to the real run event stream and auto-extracts user facts (test: `distiller_facts`). |
| Session continuity | Alpha | Cross-surface sessions (§8): gateway + CLI share a `SessionStore`, keyed `user:<id>` / `$SPARROW_SESSION`; round-trip test `session_continuity`. CLI↔gateway bridging via `SPARROW_SESSION=user:<id>`. |
| Checkpoint/rewind | Alpha | Git refs/stash implementation; `diff` and `prune` sub-commands added. |
| TUI | Alpha | Terminal cockpit connected to engine; input → drive loop working. |
| WebView console | Alpha | Local HTTP/WebSocket console on port 9339; config persisted on write. |
| Gateway WebSocket | Alpha | Message response roundtrip tested on port 9338; cron scheduler wired. |
| Telegram/Discord/Slack | Partial | Real transport implementations exist; account-token E2E validation still needed. |
| Extra gateway transports | Experimental | WhatsApp/Signal/Email/Feishu adapters present; return explicit unsupported errors. |
| Swarm orchestrator | Stable | Planner/Coder/Verifier with real tool dispatch; per-role `FallbackBrain` survives 404/ratelimit; coder tier floored to Medium; empty-diff guard forces tool use (no false PASS). Proven live: writes files end-to-end. |
| TUI task folding | Alpha | Collapsible task groups (runs/agents/tool calls): Ctrl+↑/↓ focus, Ctrl+O fold/unfold, `/collapse` `/expand`. |
| Auth OAuth | Alpha | `auth login <github\|google\|microsoft>` device flow wired to `OAuthFlow` (needs a client id). API-key providers via `auth add`. |
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
| Release packaging | Planned | CI definitions exist; public release artifacts not yet published. |

## Multimodal / infra — now implemented (configure your own keys)

All implemented as REAL code that returns honest errors when unconfigured —
never fake success. The user supplies credentials at config time.

| Item | State | Notes |
|---|---|---|
| Image generation | Alpha | `image_generate` tool → OpenAI-compatible `/images/generations`; saves PNG. Key: `IMAGE_API_KEY`/`OPENAI_API_KEY`. |
| Text-to-speech | Alpha | `text_to_speech` tool → `/audio/speech`; saves audio. Key: `TTS_API_KEY`. |
| Persistent Python kernel | Alpha | `python_rpc` keeps a long-lived `python3` process; vars/imports persist across calls (JSON-line driver). |
| SSH sandbox | Alpha | Real remote exec over `ssh`; the primary "remote VM" backend. |
| Docker sandbox | Alpha | Real container exec. |
| Cloud sandboxes (modal/daytona/vercel/singularity) | Partial | Shell out to the vendor CLI when installed+authed; otherwise an HONEST exit-127 error pointing to ssh/docker. No fabricated success. |
| Email inbound (IMAP) | Alpha | `email` feature: polls UNSEEN from allowed senders every 30s → `GatewayMessage`. Outbound SMTP via `lettre`. |
| Replay TUI scrubber | Alpha | `sparrow replay <id> --scrub` opens a ←/→ event scrubber in the TUI. |
| Release CI + install.sh | Alpha | 5-platform matrix + checksums + `curl\|sh` installer. Publishing needs a maintainer tag push + signing secrets. |

## Remaining — genuinely needs external resources

1. **Gateway E2E with live tokens** — Telegram/Discord/Slack/Email transports are real but need live bot tokens to validate end-to-end. WhatsApp send is real (Graph API); inbound webhooks need a public URL.
2. **Hardened sandbox on Windows/macOS** — namespace/seccomp is Linux-only; other platforms enforce workspace path-boundary (honest note in `doctor`). Windows Job Objects / macOS seatbelt are future.
3. **Browser automation** — `headless_chrome` behind the `browser` feature; needs a Chrome binary to run.
4. **Signed release publishing** — needs the repo's CI signing secrets + a tag push (maintainer action).

## Beyond Alpha (design-level future work)

- Cross-platform identity unification for sessions (same human across Telegram/Slack ids).
- Plugin system for providers, tools, and surfaces.
- Public website.
