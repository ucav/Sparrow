# Skill: Refactor Function

**Trigger:** refactore cette fonction, clean this up, simplifie ce code

**Description:** Refactoring ciblé d'une fonction : extraction de sous-fonctions, simplification de la logique, amélioration de la lisibilité sans changer le comportement.

## Body

1. **Analyse** : ils la fonction. Qu'est-ce qu'elle fait ? Est-ce qu'elle fait TROP de choses ?
2. **Principe de responsabilité unique** : une fonction = une tâche
3. **Techniques** :
   - Extraire les blocs de code en fonctions nommées
   - Remplacer les commentaires par des noms de fonctions explicites
   - Réduire la profondeur d'imbrication (early returns)
   - Remplacer les magic numbers par des constantes nommées
   - Utiliser des `match` ou pattern matching au lieu de `if/else` en cascade
4. **Avant/Après** : montre le code original et le code refactoré côte à côte
5. **Vérification** : les tests existants doivent toujours passer

⚠️ RÈGLE D'OR : ne change JAMAIS le comportement. Le refactoring est une transformation structurelle, pas fonctionnelle.

Exemple avant/après :
```
// AVANT (40 lignes, 4 niveaux d'imbrication)
fn process_order(order: Order) -> Result<Receipt> {
    if order.is_valid() {
        if order.has_items() {
            let total = order.items.iter().map(|i| i.price).sum();
            if total > 0.0 {
                // ...
            }
        }
    }
}

// APRÈS (3 fonctions de 10 lignes)
fn process_order(order: Order) -> Result<Receipt> {
    validate_order(&order)?;
    let total = calculate_total(&order);
    generate_receipt(&order, total)
}
```
