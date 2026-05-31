# Skill: Upgrade Dependencies

**Trigger:** upgrade, update dependencies, bump, outdated packages

**Description:** Safe dependency upgrades: bump → build → test → changelog.

## Body
When upgrading dependencies:
1. List outdated packages (cargo outdated, npm outdated, pip list --outdated).
2. Upgrade ONE package at a time.
3. After each upgrade: build → test. If breaks, investigate or rollback.
4. Check changelog for breaking changes.
5. After all upgrades: run full test suite.
6. NEVER upgrade all packages at once without testing individually.
