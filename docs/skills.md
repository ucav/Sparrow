# Skills

Sparrow skills are reusable operating procedures stored as `SKILL.md` files.

## Progressive Disclosure

A skill can keep its main `SKILL.md` short and declare optional files that are
loaded only when the skill is explicitly invoked:

```markdown
# Skill: deep-review

**Trigger:** review, diff

**Description:** Review code changes carefully.

**References:** references/checklist.md

**Templates:** templates/report.md

**Scripts:** scripts/audit.ps1

**Assets:** assets/example.png

## Body
Short operating instructions.
```

Automatic relevance loading uses the short body only. `sparrow skills view
<name>` invokes the skill and loads declared references on demand.

## Commands

```bash
sparrow skills list
sparrow skills view <name>
sparrow skills create <name>
sparrow skills install <local-dir-or-git-url>
sparrow skills update <name>
sparrow skills prune
sparrow skills rm <name>
```

Skills also appear as slash commands using their slug, for example
`/deep-review`.
