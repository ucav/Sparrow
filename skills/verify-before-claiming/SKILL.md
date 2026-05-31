# Skill: Verify Before Claiming

**Trigger:** always active, background, meta-skill

**Description:** Meta-skill: enforce that every claim must be backed by real execution. Links to WS1 anti-simulation.

## Body
This meta-skill is always loaded. It enforces:
1. NEVER claim a test result without running the tests.
2. NEVER claim a build succeeds without running the build.
3. NEVER claim output/result X without executing and capturing the raw output.
4. NEVER claim a function/struct exists without reading the actual file.
5. When uncertain: say "I need to verify" and execute the verification.
6. When something looks wrong: say so honestly, don't pretend it's fine.
