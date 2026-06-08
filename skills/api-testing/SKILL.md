# Skill: API Testing

**Trigger:** api test, endpoint test, test API, curl test

**Description:** Test d'API : requêtes HTTP, validation de réponses, scénarios, authentification, erreurs.

## Body

```bash
# GET
curl -s http://localhost:9339/status | jq '.'

# POST avec JSON
curl -X POST http://localhost:9339/api/run \
  -H "Content-Type: application/json" \
  -d '{"task":"hello"}'

# Avec authentification
curl -H "Authorization: Bearer $TOKEN" https://api.example.com/users

# Voir les headers
curl -I https://api.example.com

# Mesurer le temps
curl -w "\n%{time_total}s\n" https://api.example.com
```

### Scénarios à tester
1. **200 OK** — requête valide → réponse correcte
2. **400** — requête invalide → message d'erreur clair
3. **401** — sans token → refusé
4. **404** — ressource inexistante → 404
5. **429** — rate limit → retry-after header
6. **500** — erreur serveur → pas de crash

### Rust (reqwest)
```rust
let client = reqwest::Client::new();
let resp = client.get("http://localhost:9339/status")
    .send().await?;
assert_eq!(resp.status(), 200);
let body: serde_json::Value = resp.json().await?;
assert!(body.get("ok").is_some());
```

### Pièges
- Tester en local ≠ tester en prod (CORS, HTTPS, latence)
- Oublier les headers CORS → bloqué par le navigateur
- Token expiré → renouveler avant le test
