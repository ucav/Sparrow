# Skill: Onboard Newbie

**Trigger:** onboarding, nouveau sur le projet, comment contribuer, getting started, guide du nouveau

**Description:** Guide un nouveau contributeur à travers la découverte du projet : structure, conventions, premier setup, première contribution.

## Body

1. **Vue d'ensemble** : explique le projet en 3 phrases. Quel problème il résout ? Pour qui ?
2. **Structure du code** : arborescence simplifiée avec les dossiers clés
3. **Setup local** :
   ```bash
   git clone <repo>
   cd <project>
   # Installer les dépendances
   # Lancer les tests
   ```
4. **Conventions** : style de code, format de commit, nommage des branches
5. **Première contribution** :
   - Cherche une issue taggée `good first issue`
   - Crée une branche `fix/description-courte`
   - Fais tes modifications
   - Lance les tests : `<commande de test>`
   - Ouvre une PR
6. **Où trouver de l'aide** : Discord, issues GitHub, docs

Adapte les commandes et les chemins au projet spécifique. Ne donne pas des instructions génériques — ils le `README.md`, le `CONTRIBUTING.md`, et le `Cargo.toml`/`package.json` pour trouver les vraies commandes.
