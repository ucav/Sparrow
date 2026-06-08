# Skill: Refactor Safely

**Trigger:** refactor, clean up, restructure, simplifie

**Description:** Refactoring sans changer le comportement. Extraction de fonctions, simplification, early returns. Protégé par les tests existants.

## Body

### Règle d'or
> Le refactoring change la STRUCTURE, pas le COMPORTEMENT.
> Si un test casse, tu as fait une régression, pas un refactoring.

### Techniques
1. **Extraire une fonction** — bloc logique → fonction nommée
2. **Early return** — réduire la profondeur d'imbrication
3. **Remplacer magic number** — `86400` → `SECONDS_PER_DAY`
4. **Pattern matching** — `if/else` en cascade → `match`
5. **Type-driven** — `String` → `enum` ou `newtype`

### Process
```bash
# 1. S'assurer que tout passe AVANT
cargo test

# 2. Faire UNE transformation
# Extraire une fonction, renommer, simplifier

# 3. Vérifier que tout passe APRÈS
cargo test

# 4. Commit atomique
git add -p && git commit -m "refactor: extract validate_email()"
```

### Avant/Après
```rust
// AVANT : 40 lignes, 4 niveaux d'imbrication
fn process(order: Order) -> Result<Receipt> {
    if order.is_valid() {
        if order.has_items() {
            let total = order.items.iter().map(|i| i.price).sum();
            if total > 0.0 {
                // 20 lignes de logique...
            }
        }
    }
}

// APRÈS : 3 fonctions de 8 lignes
fn process(order: Order) -> Result<Receipt> {
    validate(&order)?;
    let total = calculate_total(&order.items);
    generate_receipt(&order, total)
}
```

### Pièges
- Ne pas "nettoyer" + "corriger un bug" dans le même commit
- Ne pas refactorer sans tests
- Le code "plus court" n'est pas toujours "plus lisible"
