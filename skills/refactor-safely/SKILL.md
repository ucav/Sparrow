# Skill: Refactor Safely

**Trigger:** refactor, restructure, clean up, reorganize, simplify

**Description:** Safe refactoring: small steps, tests green between each, checkpoint every batch.

## Body
When refactoring:
1. Ensure tests pass BEFORE any changes. Run them and confirm.
2. Plan refactoring in small, reversible steps.
3. For each step: make ONE change → run tests → checkpoint if tests pass.
4. If tests fail: revert to last checkpoint immediately.
5. After all steps: run full test suite one final time.
6. NEVER: refactor and add features simultaneously. Skip testing between steps.
