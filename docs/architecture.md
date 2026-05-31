# Architecture

## Conceptual Model

Sparrow is built around one primitive: **AgentRun**.

```
AgentRun = Identity + BrainPolicy + AutonomyContract + ToolSet + Memory + Workspace
```

Everything is a configuration of this primitive. A swarm is multiple runs sharing a workspace. A scheduled job is a run triggered by cron.

## Module Tiers

```
Tier 0: config · auth · provider · tools · sandbox     (foundations)
Tier 1: router · engine · memory                        (core logic)
Tier 2: autonomy · agent · capabilities                 (safety + learning)
Tier 3: orchestrator · scheduler · runtime              (coordination)
Tier 4: tui · cli · api · gateway                       (surfaces)
```

Each tier depends only on lower tiers. Every module exposes a trait for testability.

## Event Stream Architecture

All surfaces (TUI, CLI, API, messaging) consume a single unified event stream:

```
Runtime ──EventBus──→ TUI
         ├──────────→ CLI (NDJSON)
         ├──────────→ API (WebSocket)
         └──────────→ Gateway (Telegram/Discord/Slack)
```

Surfaces are thin renderers. No business logic outside the engine.

## Headless Runtime

The runtime owns persistent agents, the scheduler, and the gateway. It serves surfaces over local transport (TCP + WebSocket). The runtime is the single source of truth.

## Agentic Loop

```
classify task → route model → assemble context → stream from brain
    → tool use? → autonomy gate → execute → observe
    → checkpoint if mutating → repeat until done
```
