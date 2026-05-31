# Autonomy & Safety

## Autonomy Levels

Sparrow uses a continuous dial, not binary modes:

| Level | Value | Description |
|---|---|---|
| Supervised | 0.0–0.33 | Every mutating/exec/destructive action asks the user |
| Trusted | 0.34–0.66 | Mutating/exec actions auto-approved with checkpoint+notify |
| Autonomous | 0.67–1.0 | Most actions auto-approved; only destructive asks |

## Risk Levels

Every tool declares a risk level:

- **ReadOnly** — reads files, searches code, lists directories
- **Mutating** — writes files, edits code
- **Exec** — runs shell commands
- **Destructive** — deletes files, drops tables, force pushes
- **Network** — makes HTTP requests

## Gate Decisions

The autonomy gate maps `(autonomy_level, risk_level) → decision`:

| Risk | Supervised | Trusted | Autonomous |
|---|---|---|---|
| ReadOnly | Allow | Allow | Allow |
| Mutating | Ask | Notify+Checkpoint | Allow+Checkpoint |
| Exec | Ask | Notify (sandbox) | Allow (sandbox) |
| Destructive | Deny | Ask | Ask |
| Network | Ask | Allow | Allow |

## Hard Stops

These always halt the run, regardless of autonomy level:

- Budget exceeded
- Sandbox escape signal
- Repeated tool failure (3+ errors)
- Write outside workspace

## Checkpoint Discipline

Before any mutating batch:
1. Snapshot workspace via git (internal ref or stash)
2. Emit `CheckpointCreated` event
3. Execute mutating actions
4. If failure, automatic rollback

`sparrow rewind [id|n]` restores any checkpoint instantly.

## Rollback Model

Every run is reversible. Autonomous runs are safe *because* every batch is checkpointed. The timeline is exposed in the TUI.
