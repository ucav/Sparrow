# Skill: Debug Systematically

**Trigger:** bug, error, crash, failing test, not working, debugging

**Description:** Systematic debugging: reproduce → isolate → hypothesize → verify. Never "guess and patch".

## Body
When debugging any issue:
1. REPRODUCE: Run the failing command/test to capture the exact error. Paste the raw output.
2. ISOLATE: Add logging/breakpoints to narrow down the failure point.
3. HYPOTHESIZE: Form ONE testable hypothesis about the root cause.
4. VERIFY: Make a minimal change to test the hypothesis. If wrong, go back to step 3.
5. FIX: Apply the verified fix. Run the full test suite to confirm no regressions.
6. NEVER: Guess a fix without evidence. Skip reproduction. Apply multiple fixes at once.
