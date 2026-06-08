# Skill: Git Workflow

**Trigger:** git, commit, branch, merge, PR, rebase

**Description:** Workflow Git complet : branches, commits conventionnels, PR, rebase, conflits, historique propre.

## Body

### Branches
```bash
git checkout -b feat/description    # Nouvelle fonctionnalité
git checkout -b fix/description     # Correction de bug
git checkout -b chore/description   # Maintenance
```

### Commits conventionnels
```
feat(auth): add OAuth2 login flow
fix(api): handle null response from /users endpoint
chore(deps): bump tokio to 1.43
refactor(parser): extract tokenizer module
test(user): add edge case for empty email
security(auth): fix token leak in logs
```

### PR workflow
```bash
# Créer la PR
gh pr create --title "feat: add OAuth2" --body "Closes #42"

# Mettre à jour après review
git add -p && git commit -m "fix: address review comments"
git push

# Squash avant merge
gh pr merge --squash --delete-branch
```

### Résoudre un conflit
```bash
git rebase origin/main
# CONFLIT dans src/main.rs
# Éditer le fichier, résoudre les conflits
git add src/main.rs
git rebase --continue
```

### Historique propre
```bash
git rebase -i HEAD~3   # squash/fixup/reword
git push --force-with-lease  # après rebase
```

### Pièges
- `git push --force` sur main = catastrophe
- Commit de fichiers sensibles (.env, tokens) → `git filter-branch`
- `git merge` crée un commit de merge → préférer `git rebase`
