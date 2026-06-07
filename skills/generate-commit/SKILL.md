# Skill: Generate Commit

**Trigger:** génère un commit, write commit message, commit this

**Description:** Analyse les modifications stagées et génère un message de commit conventionnel (Conventional Commits) en français ou en anglais.

## Body

1. **Analyse** : `git diff --cached` pour comprendre ce qui a changé
2. **Type** : choisis le bon préfixe :
   - `feat` : nouvelle fonctionnalité
   - `fix` : correction de bug
   - `chore` : maintenance, dépendances
   - `docs` : documentation
   - `refactor` : restructuration sans changement fonctionnel
   - `test` : ajout/modification de tests
   - `security` : correction de sécurité
3. **Scope** : précise le module concerné (ex: `auth`, `cli`, `api`)
4. **Message** : verbe à l'impératif, première ligne ≤ 72 caractères
5. **Body** (optionnel) : explique POURQUOI, pas juste QUOI

Format :
```
type(scope): message court

- Changement 1
- Changement 2

Closes #123
```

Exemple :
```
security(auth): encrypt credential store with ChaCha20-Poly1305

- Replace plaintext storage with AEAD encryption
- Add OS keychain integration as primary store
- Fallback to encrypted file when keychain unavailable
```
