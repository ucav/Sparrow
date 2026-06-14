# Module: Testing Protocol

The rule that protects everything else: **never claim a test passed unless you ran
it and saw it pass.**

## Order of operations
1. **Targeted test** — the narrowest test that exercises the change.
2. **Unit tests** — the module's suite.
3. **Typecheck** — where the language supports it.
4. **Lint** — style + common-bug rules.
5. **Build** — it must compile/bundle.
6. **Integration tests** — cross-module behavior.
7. **End-to-end** — if a harness exists.
8. **Smoke test** — actually run the thing and observe it.
9. **Final deliverable check** — does it meet the acceptance bar?

Scale this to the task: a one-line fix may need only steps 1, 3, 5; a feature
needs the full ladder.

## When a check can't run
- State **why** (missing dep, no harness, needs credentials, sandbox limit).
- Propose an alternative (mock, manual reproduction, a smaller check).
- Do a **manual validation** and describe exactly what you observed.
- **Never** assert it passed. "Not run — here's why + manual check" beats a false
  green.

## Writing tests
- Cover the new behavior **and** at least one failure/edge path.
- Prefer deterministic tests; isolate I/O, time, and network behind seams/mocks.
- A bug fix gets a regression test that fails before the fix and passes after.
- Don't write tests that assert on incidental implementation details.

## Reporting
Report the **command** and the **actual output summary** (counts: passed/failed,
key failures verbatim). Example:
```
cargo test --all-targets  →  41 suites, 0 failures
npm run typecheck         →  0 errors
```
If something failed and you fixed it, show the before→after, not just the final
green.

## Credential / environment-gated tests
If a real test needs a token/account you don't have, write it **gated**: it skips
(and passes) without the credential and becomes real proof when the credential is
present. Mark such paths as "alpha until executed with credentials" — don't claim
the capability is proven from a skipped run.
