# Phase 0 — Diagnostic Complet · Sparrow v0.7.1

> **Date :** 2026-06-10
> **Méthode :** Trace de chaque point de câblage à travers le code source.
> Aucun stub, aucune hypothèse. Chaque ligne est vérifiée.

---

## D1 · Skill Catalog Timing — PARTIELLEMENT FONCTIONNEL

**Fichier :** `src/engine/mod.rs:1286-1321`, `src/engine/mod.rs:248-378`

### Ce qui marche
- Le catalogue des skills (`skill_catalog`) est chargé AVANT `build_system_prompt()` à la ligne 1295
- Les skills pertinents (top 3) sont chargés à la ligne 1287-1291
- L'injection dans le system prompt couvre les deux : catalogue complet (lignes 341-368) + corps des skills pertinents (lignes 370-375)

### Ce qui est cassé
- **Seulement 3 skills sont pré-chargés** (`.relevant(&task.description, 3)` ligne 1290). Le nouveau main_soul.md exige "scan the available skills and LOAD every one that could apply" — mais le pré-chargement est limité à 3.
- **Le mécanisme d'invocation est `skill_invoke <name>`** (ligne 345 du prompt). Le nouveau soul demande un chargement automatique, pas une invocation manuelle.
- **Aucune règle dans l'engine** qui force le pré-chargement avant l'exécution d'un tool. Le soul le demande, mais l'engine ne l'applique pas.

### Fix nécessaire
1. Passer de `.relevant(3)` à `.relevant(5)` minimum
2. Ajouter dans l'engine un check : avant tout tool call, vérifier si des skills pertinents n'ont pas été chargés
3. Aligner le prompt d'injection avec le nouveau soul (remplacer `skill_invoke` par un chargement automatique)

---

## D2 · Tool Enforcement — PARTIELLEMENT FONCTIONNEL

**Fichier :** `src/engine/mod.rs:1749-1795`

### Ce qui marche
- Le parsing des `BrainEvent::ToolUseStart/Delta/End` est correct (lignes 1749-1795)
- L'engine émet `Event::ToolUseProposed` avec les vrais args (ligne 1795)
- Les hooks PreToolUse/PostToolUse sont exécutés (lignes 1951, 2070)

### Ce qui est cassé
- **Aucune détection de "tool narration".** Le modèle peut écrire "I'll run the tests" sans que l'engine ne détecte que c'est une narration et non un tool call. Le parsing ne vérifie pas si le texte contient des patterns de tool sans être suivi d'un vrai `ToolUse`.
- **Le nouveau main_soul.md** a la règle 4.2 "Tools — call them, don't narrate them" mais c'est une règle de prompt, pas une règle d'engine. Si le modèle ignore le prompt, l'engine ne fait rien.

### Fix nécessaire
1. Ajouter dans l'engine un détecteur de "tool mention without call" : si le texte contient "I'll use X" ou "Let me X" sans `ToolUse` associé → injecter un message système "CALL the tool, don't describe it"
2. Compteur : après 2 tool narrations sans call → le run échoue avec un message explicite

---

## D3 · Identity Propagation — CASSÉ

**Fichier :** `src/engine/mod.rs:1066-1074`, `console.html:1992-1993`

### Ce qui est cassé
- **L'engine émet `Event::AgentSpawned` avec `role: "coder"`** (ligne 1074). Le champ `role` contient le RÔLE FONCTIONNEL (planner/coder/verifier), pas le DISPLAY NAME.
- **Le frontend WebView utilise `role` comme label d'affichage** (console.html:1993) :
  ```javascript
  const label=role==='user'?'you':role==='assistant'?'sparrow':role;
  ```
  Quand `role === 'coder'`, le label affiché est `'coder'` au lieu de `'sparrow'`.
- **L'engine n'envoie jamais le `agent_name` comme champ séparé** dans les events de message.

### Fix nécessaire
1. Ajouter `agent_display_name: String` à `Event::AgentSpawned`
2. Dans l'engine, utiliser `identity.name` comme `agent_display_name` au lieu de `role`
3. Dans console.html, utiliser `agent_display_name` si présent, sinon fallback à `'sparrow'`
4. Pour les messages utilisateur : forcer `role === 'user'` → label `'you'`

---

## D4 · Permission Persistence — CASSÉ

**Fichier :** `src/permissions.rs`, `src/console.rs:1022-1058`, `src/console.rs:1825-1858`

### Ce qui est cassé
- **`PermissionMode` est global** (ReadOnly/Plan/Supervised/Trusted/Autonomous/EmergencyStop) — pas de per-tool persistence.
- **Le commentaire à console.rs:1033-1036 le confirme :** "The scope (once/session/always) is enforced client-side: the frontend remembers session-approved tool names". C'est du stockage navigateur, pas du stockage Sparrow.
- **`POST /permissions`** (console.rs:1825-1858) ne gère QUE le `PermissionMode` global. Pas de support pour des permissions par tool.
- **`PermissionVerdict`** n'a que `decision: Decision` et `reason: String` — pas de champ `scope` ou `persistent`.
- **`Decision` enum** n'a que `Allow`, `Deny`, `AskUser` — pas de `AllowAlways` ou `AllowSession`.

### Fix nécessaire
1. Ajouter `Decision::AllowSession` et `Decision::AllowAlways` à l'enum Decision
2. Créer un fichier `~/.config/sparrow/permissions.json` avec le format :
   ```json
   {"tool_name": {"decision": "allow", "scope": "session"|"global"}}
   ```
3. Dans `PermissionConfig`, charger ce fichier au démarrage
4. Dans `POST /permissions`, accepter un champ `tools: {"tool_name": "allow"|"deny"}`
5. Dans `PermissionVerdict`, ajouter `scope: PermissionScope`

---

## D5 · Memory Distillation — FONCTIONNEL MAIS FAIBLE

**Fichier :** `src/extras.rs:14-116`, `src/engine/mod.rs:2955-2961`

### Ce qui marche
- Le `Distiller::distill()` est appelé après chaque run réussi (engine/mod.rs:2961)
- Les faits extraits sont sauvegardés dans la mémoire
- Les faits sont réinjectés dans le system prompt (`all_facts()` ligne 1304)

### Ce qui est faible
- **Le Distiller n'extrait que 3 types de faits :** langages utilisés, frameworks, style (test-driven, refactoring). C'est très superficiel.
- **Aucune extraction de préférences utilisateur explicites :** "I prefer async/await", "use tabs not spaces", "targeting Python 3.11"
- **Aucune extraction de conventions projet :** nommage, structure de dossiers, CI config
- **Aucune extraction de patterns d'erreur récurrents :** "ce bug revient souvent"
- **Les faits ne sont pas priorisés ni dédoublonnés par pertinence**

### Fix nécessaire
1. Étendre le Distiller pour extraire : préférences explicites, conventions projet, erreurs récurrentes
2. Ajouter une détection de patterns : si le même outil est utilisé 3+ fois, c'est un fait
3. Ajouter un poids aux faits (fréquence d'utilisation = pertinence)
4. Afficher dans le TUI/WebView : "🧠 3 faits appris cette session"

---

## D6 · WebView Sender Attribution — CASSÉ

**Fichier :** `console.html:1992-1993`

### Ce qui est cassé
```javascript
const label=role==='user'?'you':role==='assistant'?'sparrow':role;
```
- `role === 'user'` → `'you'` ✅
- `role === 'assistant'` → `'sparrow'` ✅
- `role === 'coder'` → `'coder'` ❌ (devrait être `'sparrow'`)
- `role === 'planner'` → `'planner'` ❌ (devrait être `'sparrow'`)

### Fix nécessaire
```javascript
const label=role==='user'?'you':(role==='assistant'||!role)?'sparrow':('sparrow-'+role);
```
Ou utiliser le `agent_display_name` envoyé par l'engine.

---

## D7 · WebView Card CSS — FONCTIONNEL MAIS LOURD

**Fichier :** `console.html` (4486 lignes, 253 KB)

### État actuel
- **Monolithe :** Un seul fichier HTML avec CSS inline, JS inline, HTML inline. 4486 lignes.
- **Layout cockpit :** CSS Grid complexe avec 8+ colonnes (ligne 48). Cards dans des lanes avec max-width limité.
- **Pas de design system :** Pas de variables CSS pour les couleurs (utilise `var(--brand)` etc. mais pas de `:root` standardisé).
- **Cards :** Les lanes ont `max-width:240px` (idle) et `max-width:340px` (working). Le texte est en `font-size:11px`.
- **Pas de syntax highlighting dans les cards :** Le code est affiché en texte brut.
- **Pas de composants réutilisables :** Tout est inline, tout est couplé.

### Problèmes identifiés par l'utilisateur
- Cards trop grandes, prennent trop de place
- Textes trop écartés/espacés
- Absence de mise en forme et de couleur
- L'interface est fonctionnelle mais pas belle

### Fix nécessaire (déjà dans le plan de refonte)
1. Extraire le CSS dans des fichiers séparés avec un design system (tokens.css)
2. Réduire le padding des cards : `padding: 12px` au lieu de valeurs variables
3. Ajouter la syntax highlighting via l'ANSI bridge existant
4. Limiter la largeur des messages : `max-width: 720px`
5. Palette standardisée avec les couleurs brand Sparrow (amber #f2a93c, bg0 #0e0b08, etc.)

---

## D8 · TUI Layout — FONCTIONNEL MAIS BASIQUE

**Fichier :** `src/tui/mod.rs` (2164 lignes)

### État actuel
- **Fonctions de render :** `render_cockpit()` (ligne 1678), `render_scroll()` (1871), `render_input()` (2098), `render_swarm_lanes()` (1814), `render_toast()` (2065), `render_diff()` (1977), `render_boot()` (1613)
- **Layout :** Le cockpit utilise `ratatui::layout` avec `Direction::Vertical` + `Constraint::Length` simple
- **Pas de sidebar :** Aucun panel latéral pour les skills/tools/memory
- **Pas de status bar persistante :** Les infos de coût/tokens sont dans le scroll, pas en barre fixe
- **render_scroll** fait 100% de la surface principale — pas de layout multi-panel

### Problèmes identifiés
- Le TUI est un flux de texte scrollable, pas un cockpit structuré
- Pas de séparation visuelle entre messages, tools, et status
- La vision "Claude Code + Hermes Agent fusion" n'est pas réalisée

### Fix nécessaire (déjà dans le plan de refonte)
1. Layout 3-panels : scroll area (70%) | sidebar (30%) | status bar (fixe)
2. Status bar persistante avec spinner, wordmark, route, cost, tokens
3. Sidebar togglable avec Tab : Memory / Tools / Skills / Config
4. Input bar en bas avec autocomplete slash commands
5. Toast temporaires pour les notifications (update dispo, skill chargé)

---

## Synthèse

| Diagnostic | Statut | Sévérité | Lignes à changer |
|---|---|---|---|
| D1 · Skill catalog | Partiel | 🔴 Haute | ~10 |
| D2 · Tool enforcement | Partiel | 🟡 Moyenne | ~30 |
| D3 · Identity propagation | Cassé | 🟡 Moyenne | ~15 |
| D4 · Permission persistence | Cassé | 🔴 Haute | ~80 |
| D5 · Memory distillation | Faible | 🟢 Basse | ~40 |
| D6 · WebView sender | Cassé | 🟡 Moyenne | ~5 |
| D7 · WebView CSS | Lourd | 🟡 Moyenne | ~200 |
| D8 · TUI layout | Basique | 🟡 Moyenne | ~300 |

**Total estimé : ~680 lignes à modifier/créer.**
**Fichiers touchés :** engine/mod.rs, permissions.rs, extras.rs, console.rs, console.html, tui/mod.rs, event.rs
