# Skill: Onboard to Repo

**Trigger:** new project, unfamiliar codebase, what does this do, explain the architecture, how is this organized

**Description:** First-contact skill: build repo-map, detect stack/conventions/tests, produce a summary grounded in real code.

## Body
When encountering an unfamiliar repository:
1. Run repo-map scanning to build file tree + symbol index.
2. Identify: language(s), build system (Cargo.toml, package.json, go.mod, etc.), test framework, linting tools.
3. Read the top-level README, main entry points, and key configuration files.
4. Summarize: what this project does, its structure, how to build/test/run it.
5. Cache the repo-map for future sessions.
6. NEVER invent or guess — every finding must be backed by a real file read.
