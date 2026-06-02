# Architecture

## Conceptual model

Sparrow is built around one primitive: **AgentRun**.

```
AgentRun = Identity + BrainPolicy + AutonomyContract + ToolSet + Memory + Workspace
```

Everything else is a configuration of this primitive. A swarm is multiple runs
sharing a workspace. A scheduled job is a run triggered by cron. A gateway
message is a run owned by an external transport.

## Module tiers (v0.2.0)

```
Tier 0  config · auth · provider · tools · sandbox · redaction        (foundations)
Tier 1  router · engine · memory · context · permissions              (core logic)
Tier 2  autonomy · agent · capabilities · hooks · plan · commands     (safety + learning)
Tier 3  orchestrator · scheduler · runtime · github · security        (coordination)
Tier 4  tui · cli · console (WebView) · gateway                       (surfaces)
```

Each tier depends only on lower tiers. Every module exposes a trait (or pure
functions) so tests can wire seams in isolation.

## Event stream architecture

All surfaces consume the same `Event` enum (see `src/event.rs`):

```
Engine ──EventBus──→ TUI         (ratatui cockpit, fold/unfold groups)
              ├────→ CLI         (NDJSON --json)
              ├────→ WebView     (WebSocket /ws + REST panels)
              └────→ Gateway     (Telegram / Discord / Slack / Email / WS)
```

Surfaces are thin renderers. No business logic outside the engine. The same
event stream feeds `FsRecorder` for replay (`sparrow replay <run-id>`).

## Headless runtime

`sparrow daemon` owns persistent agents, the scheduler, and the gateway. It
serves surfaces over local transport (TCP + WebSocket) and is the single
source of truth.

## Agentic loop

```
classify task → route model → assemble context
     → stream tokens
         → tool use? → permission gate → autonomy gate → hook PreToolUse
             → execute in sandbox → hook PostToolUse → observe
         → mutating? → auto-checkpoint (Git) → continue
     → transcript > 120k chars? → auto-compact + HandoffDoc + Event::Compacted
     → repeat until done or guarded
```

Key invariants:

- `permissions.deny` wins over `allow` and overrides autonomy.
- Every `Mutating`/`Exec`/`Destructive` tool triggers a checkpoint AND fires
  `HookEvent::PreCheckpoint`/`PostCheckpoint`.
- A `Mutating` tool also flips `had_mutation`, which schedules the
  `verify_command` after the model says it's done.
- `MAX_TURNS = 60` is a hard safety cap independent of budget.
- Compaction never drops the first user message; it summarises the middle and
  keeps the last `COMPACT_KEEP_LAST = 6` messages verbatim.

## Sandboxes

| Backend | Module | Notes |
|---|---|---|
| `local` | `sandbox::LocalSandbox` | Workdir must be inside root; denied-path + env-allowlist policy applied |
| `local-hardened` | same, network off | Linux: optional firejail/bwrap/unshare wrap |
| `docker` | `sandbox::backends::DockerSandbox` | Mounted workspace, `--network=none` option |
| `ssh:<host>` | `sandbox::backends::SshSandbox` | Remote execution via `ssh` |
| `worktree` | `sandbox::backends::WorktreeSandbox` | Dedicated `git worktree`; fails clearly on non-git roots |
| `modal` / `daytona` / `vercel` / `singularity` | `sandbox::backends::*Sandbox` | Vendor CLI wrappers; honest exit 127 when CLI missing |

## Memory

Two tiers, both in `src/memory/`:

- **SQLite facts** — durable, structured (`Fact { id, key, value, updated_at }`).
- **Bounded docs** — `MEMORY.md` (~2.2k chars) and `USER.md` (~1.4k chars)
  injected as durable context but explicitly NOT executable.

Anti-injection guards reject oversized writes, credential-exfil patterns,
invisible-Unicode, and prompt-injection phrases like
`"ignore previous instructions"`.

## Surfaces

| Surface | Module | Notes |
|---|---|---|
| CLI | `cli::Cli` | `sparrow <verb>` — plan, run, agent, memory, permissions, tools, security, github, compact, … |
| TUI | `tui::Tui` | ratatui cockpit, slash + `@` autocomplete, fold/unfold, theme variants |
| WebView | `console::WebViewServer` | port 9339; `/`, `/run`, `/plan`, `/memory`, `/plugins`, `/tools`, `/permissions`, `/security`, `/sessions`, `/upload`, `/artifacts`, `/ws` |
| Gateway | `gateway::*` | Telegram, Discord, Slack, Email, WS API on port 9338 |
| GitHub Action | `action.yml` + `sparrow github *` | composite action; CLI is the gh wrapper |

## What grows with you

- `MEMORY.md` / `USER.md` accumulate durable facts.
- `~/.config/sparrow/skills/` collects learned skills (curator loop).
- `~/.config/sparrow/plugins/` holds installed plugins.
- `~/.config/sparrow/agents/` holds `.soul.toml` and `.agent.md` agents.
- `<state>/sparrow/transcripts/` records every run for replay.
- `<state>/sparrow/sessions.db` records every session for FTS search and
  cross-surface continuity.
