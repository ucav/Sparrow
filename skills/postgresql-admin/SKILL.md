# Skill: PostgreSQL Admin

**Trigger:** postgres, psql, database, migration, slow query

**Description:** Administration PostgreSQL : requêtes, index, migrations, sauvegarde, optimisation.

## Body

```bash
# Connexion
psql -h localhost -U user -d database

# Commandes psql
\\dt              # Liste les tables
\\d+ users         # Structure détaillée
\\di              # Liste les index
\\x               # Mode expanded (vertical)
\\timing          # Affiche le temps d'exécution
```

### Requêtes utiles
```sql
-- Top 10 requêtes lentes
SELECT query, mean_exec_time, calls
FROM pg_stat_statements
ORDER BY mean_exec_time DESC LIMIT 10;

-- Index manquants
SELECT schemaname, tablename, seq_scan, seq_tup_read,
       idx_scan, seq_tup_read / seq_scan AS avg
FROM pg_stat_user_tables
WHERE seq_scan > 0 ORDER BY seq_tup_read DESC;

-- Tailles des tables
SELECT relname, pg_size_pretty(pg_total_relation_size(relid))
FROM pg_cached_subscription WHERE relkind='r'
ORDER BY pg_total_relation_size(relid) DESC;
```

### Index
```sql
-- Index simple
CREATE INDEX idx_users_email ON users(email);

-- Index pour recherche texte
CREATE INDEX idx_articles_fts ON articles
USING GIN(to_tsvector('english', title || ' ' || body));

-- Index partiel
CREATE INDEX idx_orders_pending ON orders(status)
WHERE status = 'pending';
```

### Sauvegarde/Restauration
```bash
pg_dump -Fc database > backup.dump     # Compressé
pg_restore -d database backup.dump     # Restaurer
pg_dumpall > full_backup.sql           # Tout le cluster
```

### Pièges
- `SELECT *` en prod → lister les colonnes
- Pas d'index sur les foreign keys → chaque JOIN scanne
- `VACUUM` oublié → bloat, performances dégradées
- Connexions non fermées → `pool_max = 100` saturé
