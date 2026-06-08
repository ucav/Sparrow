# Skill: Database Design

**Trigger:** database, schema, migration, SQL design, table

**Description:** Conception de base de données : schéma, normalisation, index, migrations, requêtes optimisées.

## Body

### Schéma minimal (PostgreSQL)
```sql
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email TEXT UNIQUE NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    status TEXT NOT NULL DEFAULT 'pending',
    total_cents INT NOT NULL CHECK (total_cents >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_orders_user_id ON orders(user_id);
CREATE INDEX idx_orders_status ON orders(status) WHERE status = 'pending';
```

### Règles
1. **Toujours UUID** — pas de serial/auto-increment pour les IDs publics
2. **TIMESTAMPTZ** — pas de TIMESTAMP sans timezone
3. **TEXT > VARCHAR(n)** — PostgreSQL traite pareil, TEXT plus flexible
4. **Indexer les FK** — chaque foreign key doit avoir un index
5. **CHECK contraints** — valider les données côté DB, pas côté app
6. **Éviter NULL** — utiliser NOT NULL + valeur par défaut

### Migrations
```bash
# diesel (Rust)
diesel migration generate create_users
diesel migration run
diesel migration redo

# Prisma (Node)
npx prisma migrate dev --name init
npx prisma migrate deploy
```

### Pièges
- Pas de FK → intégrité des données compromise
- `SELECT *` en prod → largeur de table change, ton code casse
- Pas de pagination → `SELECT * FROM users` sur 1M lignes = crash
