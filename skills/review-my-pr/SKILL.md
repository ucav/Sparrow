# Skill: Review My PR

**Trigger:** review ma PR, check this PR, code review, relis mon code

**Description:** Review complète d'une pull request : sécurité, performance, lisibilité, edge cases, régressions. Rapport structuré avec priorités.

## Body

1. **Contexte** : ils le titre et la description de la PR. Quel problème est censé être résolu ?
2. **Diff** : `git diff origin/main...HEAD` pour voir tous les changements
3. **Checklist** :
   - 🔒 **Sécurité** : secrets exposés ? injections ? permissions ?
   - ✅ **Correction** : le fix résout-il vraiment le problème ? Y a-t-il des edge cases non traitées ?
   - ⚡ **Performance** : allocations inutiles ? boucles inefficaces ? N+1 queries ?
   - 🔄 **Régressions** : ce changement peut-il casser quelque chose d'existant ?
   - 📖 **Lisibilité** : noms de variables clairs ? fonctions pas trop longues ? commentaires utiles ?
   - 🧪 **Tests** : les tests couvrent-ils les changements ? les tests existants passent-ils ?
4. **Rapport** : classe les problèmes par sévérité (🔴 Bloquant, 🟡 Important, 🔵 Suggestion)

Format de sortie :
```
## Review : PR #X — Titre

### 🔴 Bloquant
- [ ] **Problème** : description → suggestion

### 🟡 Important  
- [ ] **Problème** : description → suggestion

### 🔵 Suggestions
- [ ] **Amélioration** : description → suggestion

### ✅ Points positifs
- Bien vu sur...
```
