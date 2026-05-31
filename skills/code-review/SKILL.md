# Skill: Code Review

**Trigger:** review, code review, check this code, audit

**Description:** Adversarial code review: security, performance, edge cases, regressions. Used by the Verifier.

## Body
When reviewing code:
1. SECURITY: Check for secret leaks, injection vectors, unsafe operations.
2. CORRECTNESS: Does it match the spec? Are edge cases handled?
3. PERFORMANCE: Any obvious bottlenecks (N+1 queries, large allocations)?
4. REGRESSIONS: Could this break existing functionality?
5. STYLE: Does it follow project conventions?
6. Report findings with file:line references. Be specific, not vague.
7. NEVER approve without checking. Never say "looks good" without concrete verification.
