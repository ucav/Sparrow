# Skill: API Docs

**Trigger:** génère la doc API, documente cette API, api documentation, document endpoints

**Description:** Génère une documentation d'API claire et complète à partir du code source. Détecte les endpoints, leurs paramètres, et produit une documentation au format standard.

## Body

1. **Scanner** : ils le code source pour trouver les définitions d'endpoints :
   - Rust : `#[get("/...")]`, `#[post("/...")]`, fonctions handlers
   - Python : `@app.route(...)`, `@router.get(...)`, fonctions FastAPI/Flask
   - TypeScript : `app.get(...)`, `router.post(...)`
   - Go : `r.GET(...)`, `mux.HandleFunc(...)`

2. **Pour chaque endpoint** :
   - **Méthode** : GET, POST, PUT, DELETE, PATCH
   - **Path** : URL complète avec paramètres
   - **Description** : une phrase qui dit ce que fait l'endpoint
   - **Paramètres** :
     - Path params : `/users/:id`
     - Query params : `?page=1&limit=10`
     - Body : schéma JSON
     - Headers : `Authorization: Bearer <token>`
   - **Réponse** : code HTTP + schéma du body
   - **Erreurs** : codes d'erreur possibles
   - **Exemple** : curl ou requête exemple

3. **Format** : OpenAPI 3.0 (Swagger) ou Markdown selon le contexte

Exemple :
```markdown
## GET /api/users/:id

Récupère un utilisateur par son ID.

**Paramètres**
| Nom | Type | Description |
|-----|------|-------------|
| id  | UUID | ID de l'utilisateur |

**Réponse** `200 OK`
```json
{
  "id": "uuid",
  "name": "string",
  "email": "string"
}
```

**Erreurs**
- `404` : Utilisateur non trouvé
- `401` : Non authentifié
```
