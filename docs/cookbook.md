# Sparrow Cookbook

Practical recipes for common tasks.

## Recipe 1: Fix a Bug

```bash
sparrow run "the login endpoint returns 500 when the token is expired. Find and fix the bug."
```

What Sparrow does:
1. Reads auth-related files
2. Searches for token validation logic
3. Finds the bug (missing `is_revoked()` check)
4. Applies the fix
5. Runs the test suite
6. Reports: "13 passed, 1 was failing, now 14 pass"

## Recipe 2: Add a Feature

```bash
sparrow swarm "add rate limiting to the API with configurable limits per endpoint"
```

What the swarm does:
- **Planner** breaks into: add config, middleware, tests
- **Coder** implements each step
- **Verifier** checks correctness, edge cases, runs tests
- If Verifier finds issues → REWORK → Coder fixes → re-verify

## Recipe 3: Refactor Safely

```bash
sparrow run "extract the database module into its own crate" --autonomy trusted
```

Sparrow will:
1. Create checkpoints before each batch
2. Move files incrementally
3. Update imports
4. Run tests between batches
5. If anything breaks → auto-rollback to last checkpoint

## Recipe 4: Scheduled Maintenance

```bash
sparrow schedule "update dependencies, run full test suite, open PR if green" --cron "0 2 * * 1" --autonomy autonomous
```

Every Monday at 2 AM, Sparrow:
1. Checks for outdated deps
2. Upgrades one at a time
3. Runs tests after each
4. If all green → opens a PR
5. Reports via Telegram

## Recipe 5: Code Review

```bash
sparrow run "review PR #42 for security issues, performance problems, and edge cases"
```

Sparrow will:
1. Read the PR diff
2. Check for secrets, injections, unsafe operations
3. Look for N+1 queries, large allocations
4. Verify edge case handling
5. Post review as PR comments
