# Glossary

**AgentRun** — The single primitive: Identity + BrainPolicy + AutonomyContract + ToolSet + Memory + Workspace.

**Brain** — A model behind a uniform trait. Normalizes messages, streaming, and tool-calling.

**Router** — Selects the model chain per task and budget. No user lock-in.

**Autonomy dial** — Continuous trust contract (Supervised → Trusted → Autonomous). Not two modes.

**Checkpoint** — Reversible git snapshot created before every mutating batch.

**Swarm** — Multiple AgentRuns over a shared workspace. Default: Planner → Coder → Verifier.

**Skill** — A reusable `SKILL.md` capability loaded into agent context when relevant.

**Curator** — The self-improvement loop: grades, consolidates, deduplicates, prunes skills.

**Surface** — A thin client over the headless runtime (TUI, CLI, API, gateway).

**Transcript** — Append-only event log (`inputs.json` + `events.jsonl`) enabling replay.

**SOUL** — Persistent agent identity file (TOML): name, role, personality, rules, default model.

**MCP** — Model Context Protocol: external tool servers connected via stdio/HTTP/SSE.

**OrgPolicy** — Enterprise policy: autonomy ceiling, provider allow-list, budget caps, blocked paths, SSO.

**Hallucination guard** — Prevents agent from claiming results without real tool execution.

**Anti-simulation guard** — Rejects assistant turns that fabricate test/build/output claims.

**Hook** — Lifecycle event handler (shell or builtin), blocking or non-blocking, 12 events.

**Reasoning depth** — Adaptive planning: trivial tasks get minimal steps, complex tasks get deep decomposition.
