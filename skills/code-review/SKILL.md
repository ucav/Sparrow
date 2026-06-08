# Skill: Code Review

**Trigger:** review, code review, check this code, audit

**Description:** Revue de code adverse : sécurité, performance, edge cases, régressions. Checklist structurée, jamais "looks good" sans vérification concrète.

## Body

### Checklist (par priorité)
1. 🔒 **SÉCURITÉ** — secrets exposés ? injection ? path traversal ?
2. ✅ **CORRECTION** — le fix résout-il le problème ? edge cases ?
3. ⚡ **PERFORMANCE** — allocations inutiles ? N+1 queries ? boucles O(n²) ?
4. 🔄 **RÉGRESSIONS** — ça casse quoi ? `cargo test` passe ?
5. 📖 **LISIBILITÉ** — noms clairs ? fonctions <50 lignes ?

### Commandes
```bash
# Voir les diffs
git diff origin/main...HEAD

# Vérifier les tests
cargo test && cargo clippy -- -D warnings

# Vérifier la couverture des fichiers modifiés
cargo tarpaulin --out Html
```

### Patterns suspects
```rust
// ❌ unwrap() en production
let x = config.get("key").unwrap();

// ✅ Gérer l'erreur
let x = config.get("key").context("missing key")?;

// ❌ clone() dans une boucle
for item in items {
    process(item.clone()); // alloue à chaque itération
}

// ✅ Passer par référence
for item in &items {
    process(item);
}
```

### Format de rapport
```
## Review: PR #X — Titre
🔴 Bloquant: ...
🟡 Important: ...
🔵 Suggestion: ...
✅ Bien vu: ...
```
