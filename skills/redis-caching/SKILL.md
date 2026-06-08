# Skill: Redis Caching

**Trigger:** redis, cache, caching, session store, rate limit

**Description:** Redis : caching patterns, session store, rate limiting, pub/sub, commandes.

## Body

```bash
redis-cli                     # Shell interactif
redis-cli PING                # Test connexion
redis-cli KEYS "user:*"       # Lister les clés
redis-cli GET user:42         # Lire
redis-cli SET user:42 "data" EX 3600  # Écrire avec TTL
redis-cli --scan --pattern "*" | head
redis-cli INFO memory         # Stats mémoire
```

### Patterns
```
# Cache-aside (le plus courant)
1. Essayer Redis → si hit, retourner
2. Si miss → charger depuis DB → stocker dans Redis → retourner
3. TTL = 5min pour données fréquentes, 1h pour stables

# Rate limiting
MULTI
  INCR rate:user:42
  EXPIRE rate:user:42 60
EXEC
→ Si > 100, bloquer

# Session store
SET session:abc123 "{\"user\":42}" EX 1800
GET session:abc123
```

### Rust (redis-rs)
```rust
let client = redis::Client::open("redis://127.0.0.1/")?;
let mut conn = client.get_connection()?;

conn.set("key", "value")?;
let val: String = conn.get("key")?;
conn.expire("key", 3600)?;
```

### Pièges
- Pas de TTL → mémoire infinie → OOM
- `KEYS *` en prod → bloque Redis (single-threaded) → utiliser `SCAN`
- Cache stampede → des centaines de requêtes simultanées sur un miss → utiliser un lock distribué
- Redis sans persistance → perte de données au restart → configurer AOF ou RDB
