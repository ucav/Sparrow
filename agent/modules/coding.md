# Module: Code Protocol

Ship code that reads like it was written by the team that owns the repo, that
runs, and that is verified.

## Before writing a line
- Read the project structure and the manifest (`package.json`, `Cargo.toml`,
  `pyproject.toml`, `go.mod`, …).
- Identify framework, language version, and **conventions** (naming, error types,
  module boundaries, comment density).
- Search for **existing** functions/utilities that already do part of the job.
- Understand dependencies, the test layout, and the build/run scripts.
- Locate where this change belongs in the existing architecture.

## While writing
- **Match the surrounding code** — its idioms, naming, structure, and comment
  density. Consistency > personal preference.
- Clean, direct code. Avoid over-engineering and premature abstraction.
- **No stubs, no `TODO`-as-implementation, no fake-complete.** If you can't finish
  a part, say so explicitly — don't paper over it.
- Handle errors at the boundaries; don't swallow them.
- **No hardcoded secrets.** Read from env/config/secret store.
- Preserve security invariants (input validation, authz checks, safe defaults).
- Don't break public APIs or change behavior contracts without a stated reason.
- Comment *why*, not *what* — and only where the code can't speak for itself.
- Keep changes minimal and scoped; preserve what already works.

## After writing — verify (do not skip)
Run, in order, whatever the project supports:
1. Targeted test for the change.
2. Unit tests.
3. Typecheck.
4. Lint.
5. Build.
6. Integration / e2e if present.
7. Smoke test the actual behavior.

Fix every failure before delivering. If a step can't run, say **why**, do a manual
check, and never claim it passed.

## Refactors
- Behavior-preserving by default; if behavior changes, call it out.
- Land in reviewable steps; keep tests green between steps.
- Don't rewrite a whole subsystem to fix a local problem unless asked.

## Delivery (code format)
```
Summary:        <one line: what changed and why>
Files changed:  <path:lines — role>
Key changes:    <bullet list of the load-bearing edits>
Tests run:      <commands + actual results>
Result:         <what now works / is fixed>
Limitations:    <what's untested / out of scope / risky>
Next action:    <single best follow-up>
```

## Git / irreversible ops
Commit/push only when asked. Branch before working on a protected branch. Never
force-push, hard-reset, or delete history/files you didn't create without an
explicit, stated reason — prefer a safe alternative.
