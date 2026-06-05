# Skill: Fix Bug

**Trigger:** fix ce bug, répare ça, debug this, corrige cette erreur

**Description:** Diagnostic et correction de bug en 5 phases. Approche systématique : reproduire, isoler, comprendre, corriger, vérifier.

## Body

1. **Reproduire** : le bug doit être reproductible. Si ce n'est pas le cas, identifie les conditions exactes.
   - Quelles entrées ? Quel état ? Quel environnement ?
   - Écris un test qui échoue (RED)

2. **Isoler** : réduis le problème à sa plus simple expression.
   - Binary search dans le code (commenter des blocs)
   - `git bisect` si c'est une régression
   - Logs + points d'arrêt

3. **Comprendre** : POURQUOI le bug se produit. Pas juste OÙ.
   - Quelle hypothèse du code est violée ?
   - Trace l'exécution pas à pas

4. **Corriger** : la correction la plus simple possible.
   - Une seule chose à la fois
   - Le test doit passer (GREEN)
   - ⚠️ Ne casse rien d'autre

5. **Vérifier** :
   - Tous les tests passent
   - Le scénario original est résolu
   - Pas de régression sur les cas connexes
   - Optionnel : ajoute un test de régression

Format de réponse :
```
🐛 **Bug** : [Description en une phrase]
🔍 **Cause racine** : [Explication technique]
✅ **Fix** : [Code ou description du changement]
🧪 **Vérification** : [Comment tester que c'est résolu]
```
