# Skill: Deploy Check

**Trigger:** checklist déploiement, pre-deploy, before release, ready to ship, vérifie avant de déployer

**Description:** Checklist pré-déploiement complète : tests, sécurité, performance, configuration, rollback. Rien n'est oublié avant de pousser en production.

## Body

Exécute cette checklist point par point. Pour chaque item, répond OK / NOK / N/A.

### 1. Code
- [ ] Tous les tests passent en local
- [ ] CI/CD verte sur la branche à déployer
- [ ] Pas de TODO/FIXME non résolu
- [ ] Les warnings de compilation sont traités
- [ ] `cargo clippy` / `eslint` / linter : zéro erreur

### 2. Sécurité  
- [ ] Scan des secrets : pas de clé API, token, ou mot de passe commité
- [ ] Les dépendances sont à jour (`cargo update`, `npm audit`)
- [ ] Pas de vulnérabilité critique dans l'audit de sécu
- [ ] Les variables d'environnement sont documentées
- [ ] HTTPS forcé en production

### 3. Performance
- [ ] Pas de régression de perf visible
- [ ] Les requêtes lentes sont optimisées ou ticketées
- [ ] Les assets statiques sont minifiés/compressés

### 4. Configuration
- [ ] Les variables d'environnement de prod sont prêtes
- [ ] Les feature flags sont dans le bon état
- [ ] Les migrations de DB sont testées
- [ ] `.env.example` est à jour

### 5. Monitoring
- [ ] Les logs sont au bon niveau (pas de debug en prod)
- [ ] Les alertes sont configurées
- [ ] Le healthcheck endpoint répond

### 6. Rollback
- [ ] Le plan de rollback est documenté
- [ ] La version précédente est encore disponible (tag git, artifact)
- [ ] Les migrations de DB sont réversibles

### 7. Communication
- [ ] CHANGELOG mis à jour
- [ ] Version bumpée (Cargo.toml, package.json, etc.)
- [ ] Équipe notifiée du déploiement

Résultat final : **GO** (tous les blocs OK) ou **NO-GO** (bloquants restants listés).
