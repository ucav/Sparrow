# Roadmap

All Sparrow milestones as defined in the [technical specification](docs/technical-spec.md).

---

## M0 — Kernel ✅

**Done when:** "Claude Code, but model-agnostic": edit a repo with a chosen/free model.

- [x] Config loading (TOML + env + CLI flags)
- [x] Auth store (env + encrypted file + OS keychain)
- [x] Provider adapters (Anthropic Messages, OpenAI Compatible, OpenAI Responses, Bedrock, Ollama)
- [x] Core tools (fs read/write/list, edit, search, exec, git, todo)
- [x] Sandbox (local + hardened + Docker + SSH + serverless stubs)
- [x] Basic router with scoring + fallback chains
- [x] Agentic engine loop (think → act → observe)
- [x] Supervised autonomy gate
- [x] CLI command grammar
- [x] Terminal TUI (ratatui cockpit)

---

## M1 — Trust ✅

**Done when:** Run Trusted/Autonomous safely; rewind works.

- [x] 4-tier memory (SQLite): repo, identity, task, shared
- [x] Persistent agents (SOUL files)
- [x] Full autonomy dial (Supervised → Trusted → Autonomous)
- [x] Git-based checkpoints with `sparrow checkpoint list` and `sparrow rewind`
- [x] Auto-checkpoint before mutating/exec/destructive actions
- [x] Redaction filter for secrets

---

## M2 — Swarm ✅

**Done when:** Diffs land only after adversarial PASS.

- [x] Orchestrator with default pipeline (Planner → Coder → Verifier)
- [x] Adversarial review loop (REWORK until PASS)
- [x] Shared memory coordination (signals, working docs)
- [x] File-level locks for anti-collision
- [x] Subagent spawn tool

---

## M3 — Grows ✅

**Done when:** Skills are created/curated; MCP tools are usable.

- [x] Skill struct + SKILL.md format
- [x] Filesystem skill library
- [x] Curator (grade → dedupe → prune)
- [x] Auto-generated skills from successful runs
- [x] MCP client (stdio + HTTP transports)
- [x] Skill relevance matching in engine context

---

## M4 — Runtime ✅

**Done when:** Scheduled unattended jobs run; replay works.

- [x] Centralized EventBus (broadcast pub/sub)
- [x] Runtime daemon with TCP API socket
- [x] Cron scheduler with job persistence
- [x] Run recorder (transcripts as `inputs.json` + `events.jsonl`)
- [x] Replayer (load + render transcripts)
- [x] `sparrow replay <run-id>` command

---

## M5 — Everywhere ✅

**Done when:** Continue a session across surfaces.

- [x] Gateway transport trait
- [x] Telegram transport (Bot API, long polling)
- [x] Discord transport (Gateway WebSocket)
- [x] Slack transport (Socket Mode)
- [x] WhatsApp, Signal, Email, Feishu, WeCom, QQBot, Teams stubs
- [x] WebSocket API server
- [x] Message router (command parsing, task routing)
- [x] `sparrow --web` / `sparrow console`

---

## M6 — Polish ⬜

**Done when:** Ships as v1.

- [x] Theming (color tokens from §9.2)
- [x] ASCII logo + boot sequence in TUI
- [x] Self-update (`sparrow update`)
- [x] `sparrow doctor` diagnostics
- [ ] IBM Plex Mono embedded
- [ ] Full cross-compilation (Linux musl, macOS, Windows MSVC)
- [ ] Signed release binaries + checksums
- [ ] `curl | sh` install tested on all platforms
- [ ] Website / landing page
- [ ] Benchmark suite

---

## Beyond v1

- [ ] Python RPC channel (persistent kernel for subagents)
- [ ] Browser automation tool (headless playback)
- [ ] Vision/image generation/TTS tools
- [ ] OAuth device flow for provider setup
- [ ] Real local-hardened sandbox (firejail/bwrap on Linux)
- [ ] Hot-reload config for daemon
- [ ] Multi-profile isolation
- [ ] Plugin system for provider/model extensions
