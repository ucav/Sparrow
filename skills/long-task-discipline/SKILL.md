# Skill: Long-Task Discipline

**Trigger:** refactor, multi-file, migrate, audit, implement feature, walk through, plan and implement

**Description:** Discipline pour tâches longues (>5 étapes / multi-fichier). Externalise l'état dans le tool `todo`, vérifie après chaque étape, et fait des points de contrôle visibles. Sans ça, l'agent oublie une étape, déclare "done" trop tôt, ou répète la même action en boucle.

## Body

### Quand cette skill s'applique
- ≥3 étapes distinctes ET séparables
- Modifications dans ≥2 fichiers
- Refactor / migration / audit
- Tâches où "tout réussir" et "tout vérifier" ne tiennent pas en mémoire

Pour une demande triviale (1 read, 1 edit, 1 réponse), n'utilise PAS cette discipline — c'est de l'overhead.

### Le pipeline

**1. Décompose AVANT d'éditer.** Appelle `todo list_add` une fois par étape concrète et vérifiable. Ne décris pas "améliorer le code" — décris "extraire `parse_header` dans `parser.rs` ligne 42, tests passent".

**2. Une étape à la fois.** Au début de chaque étape, marque-la `in_progress` via `todo mark`. À la fin, `done`. Si tu en sautes une, c'est un signal que le plan était mauvais — rédige un nouveau plan, ne mens pas sur l'état.

**3. Vérifie à chaque étape, pas à la fin.** Une étape ne passe à `done` que si :
   - le fichier compile (`cargo check` ou équivalent)
   - les tests touchés passent
   - la modification est visible dans `git diff`

   Pas de "ça devrait marcher" — exécute, lis la sortie, colle-la dans le todo si elle est utile.

**4. Le rapport final lit le tableau, pas la mémoire.** Quand tu finis, fais `todo list` une dernière fois et résume ce que le tableau dit. Pas ce que tu te souviens avoir fait.

### Anti-patterns à éviter
- ❌ Marquer plusieurs étapes `done` d'un coup à la fin
- ❌ Faire une étape sans entrée dans le todo (drift silencieux)
- ❌ "J'ai aussi corrigé X au passage" → si X n'était pas dans le todo, soit ajoute-le rétroactivement, soit annule X
- ❌ Refaire le même tool call avec les mêmes args sur 3+ tours — c'est le signe que tu es bloqué·e, change d'approche

### Mini-exemple

```
todo list_add "Lire src/parser.rs et identifier les 4 fonctions à extraire"
todo list_add "Créer src/parser/headers.rs avec parse_header"
todo list_add "cargo check passe"
todo list_add "Mettre à jour les imports dans 3 sites appelants"
todo list_add "cargo test parser:: passe (12 tests attendus)"

# Puis par étape :
todo mark <id> in_progress
[... travail ...]
todo mark <id> done
todo list   # voir le tableau avant l'étape suivante
```

### Lien avec le protocole REFLEXION-MAX
Ce skill est la matérialisation concrète des étapes 3.1 (Décomposition) et 3.5 (Verification) du protocole : un tableau lisible et persistant remplace la mémoire interne, qui ment sur les longs runs.
