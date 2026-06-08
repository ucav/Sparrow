# Skill: Security Audit

**Trigger:** security, audit, vulnerability, secret scan, pentest

**Description:** Audit de sécurité : scan de secrets, dépendances, permissions, surfaces d'attaque.

## Body

```bash
# Scan de secrets
gitleaks detect --source . --verbose
trufflehog filesystem .

# Dépendances
cargo audit                    # Rust
npm audit                      # Node.js
pip-audit                      # Python

# Secrets dans l'historique git
git log -p | grep -E 'sk-|ghp_|AKIA|-----BEGIN'

# Permissions de fichiers
find . -type f -perm /111 -not -path './.git/*'  # Exécutables inattendus
find . -type f -name "*.pem" -o -name "*.key"     # Clés privées
```

### Checklist
1. **Secrets** — pas de token, clé API, mot de passe dans le code
2. **Dépendances** — pas de vulnérabilité connue (CVE)
3. **Permissions** — principe du moindre privilège
4. **Input validation** — tout input utilisateur est sanitizé
5. **HTTPS** — pas de HTTP en clair, TLS ≥1.2
6. **Headers** — CSP, HSTS, X-Frame-Options
7. **Logs** — pas de donnée sensible dans les logs
8. **Rate limiting** — protection contre brute force

### .gitignore minimal
```
.env
*.pem
*.key
credentials.json
service-account.json
```

### Pièges
- `.env` commité = toutes les clés exposées
- `git filter-branch` oublié après un leak → l'historique contient encore les secrets
- Dépendance non épinglée → supply chain attack
