# Skill: GitHub API

**Trigger:** github api, gh, GitHub REST, GraphQL

**Description:** Utilisation de l'API GitHub : REST, GraphQL, gh CLI, webhooks, actions.

## Body

### gh CLI (le plus simple)
```bash
gh api /repos/ucav/Sparrow                     # GET
gh api /repos/ucav/Sparrow/issues -f title="Bug"  # POST
gh api graphql -f query='{ viewer { login } }'    # GraphQL

# Créer une PR
gh pr create --title "feat: something" --body "Closes #42"

# Lister les PRs ouvertes
gh pr list --state open --limit 10
```

### REST API (curl)
```bash
# Avec token
curl -H "Authorization: Bearer $GITHUB_TOKEN" \
     -H "Accept: application/vnd.github+json" \
     https://api.github.com/repos/ucav/Sparrow/issues

# Créer une issue
curl -X POST \
     -H "Authorization: Bearer $GITHUB_TOKEN" \
     -d '{"title":"Bug report","body":"Details..."}' \
     https://api.github.com/repos/ucav/Sparrow/issues
```

### GraphQL (une requête, données précises)
```graphql
query {
  repository(owner: "ucav", name: "Sparrow") {
    issues(first: 5, states: OPEN) {
      nodes {
        title
        url
        createdAt
      }
    }
    stargazers { totalCount }
  }
}
```

### Rate Limits
- Authentifié : 5000 req/h
- Non authentifié : 60 req/h
- GraphQL : 5000 points/h (calculé par complexité)
- Vérifier : `curl -H "Authorization: Bearer $TOKEN" https://api.github.com/rate_limit`

### Pièges
- Token expiré → 401, regénérer sur github.com/settings/tokens
- `gh api` sans `--paginate` → seulement la première page
- Webhook secret non vérifié → toujours valider `X-Hub-Signature-256`
