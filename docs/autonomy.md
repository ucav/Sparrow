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

Sparrow evaluates permissions first, then autonomy. Permission modes can deny or
force approval by tool, path, provider, or surface before the autonomy matrix is
consulted.

| Permission mode | Behavior |
|---|---|
| Read-only | Allows read-only tools; denies mutating, exec, network, and destructive tools |
| Plan | Denies tool execution entirely; use for planning-only sessions |
| Supervised | Defers to the Supervised autonomy gate unless an explicit permission rule matches |
| Trusted | Defers to the Trusted autonomy gate, still protected by denied paths and checkpoints |
| Autonomous | Defers to the Autonomous autonomy gate, still protected by denied paths and budget hard stops |
| Emergency stop | Denies every tool execution |

Default denied path boundaries include `.git`, `.env`, `.env.local`, `.ssh`,
`id_rsa`, and `id_ed25519`. Add or inspect rules with `sparrow permissions list`.

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
