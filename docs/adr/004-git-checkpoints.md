# ADR 004: Git-based checkpoints over custom snapshots

**Status:** Accepted

**Context:** Every mutating batch needs a reversible snapshot. Options: custom file backup, git stashes, filesystem snapshots.

**Decision:** Use git (`git stash create` + `git update-ref`). Every mutating batch creates a checkpoint ref under `refs/sparrow/checkpoints/`. Rewind uses `git reset --hard <ref>`.

**Consequences:** Requires the workspace to be a git repo. Zero-copy snapshots (git object store). Full git history preserved. Rollback is instant.
