# Skill Registry (Community)

Sparrow's skill registry is a community-driven library of reusable skills.

## Built-in Skills (11)

Located in `skills/`:
1. `onboard-to-repo` — First-contact repo analysis
2. `debug-systematically` — Reproduce → isolate → fix
3. `write-tests` — Nominal + edge + regression
4. `refactor-safely` — Small steps, checkpoint
5. `code-review` — Adversarial review heuristics
6. `upgrade-dependencies` — Bump + build + test
7. `security-audit` — Secrets, injections, deps
8. `write-docs` — Docs from real code
9. `git-workflow` — Branches, atomic commits
10. `performance-profile` — Measure before optimizing
11. `verify-before-claiming` — Meta-skill anti-fabrication

## Installing Community Skills

```bash
# Search the registry
sparrow skills search "rust async"

# Install a skill
sparrow skills install github.com/sparrow-community/async-patterns

# List installed
sparrow skills list
```

## Publishing a Skill

1. Create `SKILL.md` with name, trigger, description, body
2. Publish to a GitHub repo
3. Submit to the registry via PR to `sparrow-community/skill-registry`

## Skill Format

```markdown
# Skill: <name>

**Trigger:** comma, separated, keywords

**Description:** One-line description

## Body
The actual instructions the agent follows when this skill is loaded.
```

## Registry Structure

```
registry/
├── skills/
│   ├── rust-async-patterns/
│   │   └── SKILL.md
│   ├── python-testing/
│   │   └── SKILL.md
│   └── ...
└── index.json
```

Each skill is versioned (git tags) and signed (optional GPG).
