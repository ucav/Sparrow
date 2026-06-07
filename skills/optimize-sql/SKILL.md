# Skill: Optimize SQL

**Trigger:** optimise cette requête, slow query, requête lente, explain this query, sql optimization

**Description:** Analyse et optimisation de requêtes SQL. Détection des problèmes courants (N+1, index manquants, jointures inefficaces) et propositions d'amélioration.

## Body

1. **Analyse** : ils la requête. Utilise `EXPLAIN` ou `EXPLAIN ANALYZE` si disponible.
2. **Problèmes courants** :
   - **N+1 queries** : une requête dans une boucle → utiliser `JOIN` ou `IN (ids...)`
   - **Index manquants** : `WHERE` ou `JOIN` sur des colonnes non indexées
   - **SELECT \*** : ne sélectionner que les colonnes nécessaires
   - **Fonctions dans WHERE** : `WHERE DATE(created_at) = '2024-01-01'` → utiliser une plage `WHERE created_at >= ... AND created_at < ...`
   - **Subqueries corrélées** : préférer `JOIN` ou `EXISTS` bien indexé
   - **LIMIT sans ORDER BY** : résultats non déterministes
3. **Avant/Après** : montre la requête originale et la version optimisée
4. **Impact** : estime le gain (temps d'exécution, lignes scannées)

Exemple :
```sql
-- AVANT (N+1)
SELECT * FROM users;
-- Pour chaque user :
SELECT * FROM orders WHERE user_id = ?;

-- APRÈS (JOIN)
SELECT u.*, o.*
FROM users u
LEFT JOIN orders o ON o.user_id = u.id;

-- Index recommandé :
CREATE INDEX idx_orders_user_id ON orders(user_id);
```
