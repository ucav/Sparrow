# Swarm Orchestration

## Default Pipeline

```
Planner → Coder → Verifier
              ↑________⟳ REWORK
```

### Planner
- Uses a strong model (typically Hard tier)
- Takes a task → produces structured SPEC with steps, files, acceptance criteria
- Posts spec to shared memory as working doc

### Coder
- Uses a cost-appropriate model (task tier)
- Implements against the spec step by step
- Produces diffs, emits `DiffProposed` events
- Respects file locks for anti-collision

### Verifier
- Uses a medium model for adversarial review
- Checks diff against spec requirements
- Finds bugs, style issues, missing edge cases
- Emits `PASS` or `REWORK` with concrete findings

## REWORK Loop

If verifier returns `REWORK`:
1. Findings are posted as a signal to coder
2. Coder re-implements with fix notes
3. Verifier re-reviews
4. Max reworks: configurable (default 3)

A diff only lands after `PASS`.

## File Locks

Anti-collision via file-level locks in shared workspace. Agents acquire locks before modifying files. Conflicts are reported as errors.

## Subagent Spawn

`subagent_spawn` tool delegates a subtask to a child AgentRun with its own conversation, sandbox, and model. Subagent output is collected and returned.
