# Audit v0.8.0 — « Pourquoi c'est mauvais avec DeepSeek » (2026-06-10)

> Question posée : le mauvais comportement avec les modèles type DeepSeek
> vient-il du modèle ou de Sparrow ?
> **Réponse : majoritairement de Sparrow.** Trois bugs critiques détruisent ou
> bloquent des tool calls parfaitement valides émis par le modèle. Les modèles
> "thinking" multi-tools (DeepSeek, Qwen, Kimi) déclenchent ces bugs presque à
> chaque tour — d'où l'impression que « DeepSeek est mauvais ».

État de départ : `cargo test --all-targets` **entièrement vert** sur master.
Les bugs ci-dessous ne sont pas couverts par la suite — c'est un trou de
couverture, pas une régression.

---

## A. Critiques (prouvés)

### A1. Un tour à N tool calls détruit le 1er appel — PROUVÉ PAR TEST
- **Repro** : `tests/multi_tool_streaming.rs` (marqué `#[ignore]`, à réactiver
  une fois corrigé). Résultat observé : `a.txt` jamais modifié, l'appel
  re-proposé comme `name: "unknown", args: {}`.
- **Cause** : deux défauts qui se combinent.
  1. `src/engine/mod.rs:1780-1862` — le moteur garde UN SEUL
     `current_tool_name` / `current_tool_json` ; l'`id` de `ToolUseDelta` est
     explicitement ignoré (`let _ = id;` ligne 1859). Le `ToolUseStart` du
     2e appel **écrase le nom et vide le buffer d'arguments du 1er**.
  2. `src/provider/openai_compat.rs:504-515` — les `ToolUseEnd` ne sont émis
     qu'à `finish_reason: "tool_calls"`, par `tool_state.drain()` → **ordre
     aléatoire** (HashMap). La séquence réelle livrée au moteur est donc
     `Start(0)·Δ(0)·Start(1)·Δ(1)·End(?)·End(?)`, jamais le
     `Start·Δ·End` séquentiel que suppose le moteur.
- **Symptôme utilisateur** : sur une tâche multi-fichiers, le 1er outil
  s'exécute en `unknown`/`{}`, l'erreur repart au modèle, qui réessaie,
  re-déclenche le bug, brûle le budget et finit en échec ou en boucle.
  Le fix v0.5.5 a corrigé la **sérialisation** (replay) des tours
  multi-tools, pas leur **exécution live**.
- **Fix proposé** : dans le moteur, accumuler par id
  (`HashMap<String, (String /*name*/, String /*json*/)>`) ; dans l'adaptateur,
  émettre `End` dans l'ordre des index (`BTreeMap` ou tri avant drain).

### A2. `finish_reason: "stop"` avec des tool calls natifs en attente → appels jetés
- `src/provider/openai_compat.rs:455-502` : la branche `"stop"` ne fait que la
  récupération de markup inline ; elle **ne draine jamais `tool_state`**. Un
  provider qui streame des `tool_calls` natifs puis termine en `"stop"`
  (comportement observé chez certains proxys OpenAI-compat et Ollama) →
  aucun `ToolUseEnd` → les appels n'exécutent jamais, le run finit en
  `EndTurn` muet.
- **Fix proposé** : dans la branche `"stop"`, si `tool_state` est non vide,
  drainer comme la branche `"tool_calls"` et terminer en `StopReason::ToolUse`.

### A3. Mode non-interactif : l'approbation bloque et le résumé ment — VU EN LIVE
- Run réel (`sparrow run "create hello.txt…" --yes`, stdin non-TTY, modèle
  Ollama local) : l'approbation `fs_write` ne peut pas être répondue, et la
  sortie affiche **à la fois** « Done. », la comparaison de coûts
  (« save 100% »), `Status: waiting_for_approval` et un exit code 1 —
  **alors qu'aucun fichier n'a été créé**.
- Trois incohérences distinctes :
  1. `--yes` saute le devis pré-run mais **pas** les approbations → un run
     scripté/CI se bloque puis meurt. Il faut une politique explicite
     non-TTY (deny par défaut + message clair, ou flag d'autonomie).
  2. « Done. » + cost-comparison s'affichent pour un run **non terminé**.
     La comparaison ne devrait s'afficher que sur un run `completed`.
  3. Le prompt d'approbation s'affiche en 3 exemplaires de formats
     différents (`Approve fs_write? [y/N]` + 2 lignes `[APPROVAL NEEDED…]`).
- C'est exactement le contraire du pilier « confiance » de la v0.9.

---

## B. Sérieux (lecture de code, à corriger avec A)

### B1. Fuite du markup DSML en début de stream
`openai_compat.rs:314-338` — `suppress_text` ne devient vrai qu'une fois que
`looks_like_tool_markup()` matche le buffer accumulé. Les premiers deltas
(`<｜｜DSML｜｜tool_calls>` partiel…) sont **déjà partis** comme `TextDelta`
visibles. L'utilisateur voit du markup brut en tête de réponse. Fix : retenir
les deltas dans un petit buffer de garde tant que l'ambiguïté n'est pas levée
(ou rétracter via un event de correction).

### B2. `tool_markup::coerce()` corrompt les arguments récupérés
`src/provider/tool_markup.rs:50-68` :
- l'attribut `string="true"` (DSML dit explicitement « c'est une chaîne »)
  est **ignoré** : `"123"` devient le nombre `123`, `"true"` devient un
  booléen → schémas d'outils violés ;
- `trim()` systématique : un paramètre `content` d'écriture de fichier perd
  ses espaces/sauts de ligne de tête et de queue → fichiers corrompus ;
- regex non-greedy `(.*?)</invoke>` : une valeur contenant `</invoke>` ou
  `</parameter>` (écrire ce parser, par exemple !) tronque l'extraction.

### B3. Faux positifs de suppression
Un texte légitime qui *discute* de `<invoke name="…">` (réponse de doc, revue
de code) déclenche `suppress_text` → la réponse visible est avalée et
re-parsée en tool calls fantômes. La détection devrait exiger une structure
complète (balise ouvrante + fermante) et non deux sous-chaînes.

### B4. `reasoning_content` dupliqué et contrat provider divergent
- `openai_compat.rs:347-386` capture le reasoning depuis `delta.*` ET
  `message.*` avec le commentaire « duplicate captures are harmless because
  the engine joins them » — faux : `engine/mod.rs:1832` **concatène**, donc
  le reasoning peut repartir en double au provider (contexte gonflé, coût,
  et risque de 400).
- Le commentaire de `build_chat_body` affirme que DeepSeek **exige** l'écho
  de `reasoning_content` ; la doc officielle de `deepseek-reasoner` dit
  historiquement l'inverse (400 si on le renvoie). Le contrat varie par
  provider/modèle (Kimi le veut, DeepSeek R1 le refuse selon version). Il
  faut un **flag par provider** (`echo_reasoning: bool`) + un test live par
  provider, pas un comportement global. **À vérifier en réel avec une clé
  DeepSeek — candidat n°1 des 400 constatés.**

### B5. Caps Ollama codées en dur
`openai_compat.rs:37-48` : tout modèle Ollama reçoit `tools: true`,
`context_window: 32k`. Les modèles locaux sans support outils reçoivent quand
même le bloc `tools` → certains émettent du pseudo-markup ou ignorent la
tâche. Croiser avec les capacités réelles (`/api/show` expose
`capabilities: ["tools", …]`).

### B6. Comptabilité tokens incohérente sur tours tool-only
Vu en live : `Tokens: 6796 in / 0 out` alors que le modèle a produit des tool
calls. L'estimation de sortie ne compte que les `TextDelta`
(`engine/mod.rs:1798-1820`) — les arguments d'outils streamés ne sont jamais
comptés si le provider n'envoie pas d'`usage`.

### B7. Double affichage des tool cards
`ToolUseProposed` est émis deux fois par design (placeholder `{}` puis args
réels — `engine/mod.rs:1850, 1884`). La console gère la mise à jour de card,
mais le **CLI affiche deux fois `[Tool: fs_write]`** (vu en live). Le renderer
CLI doit dédupliquer par id.

### B8. Ids de markup-calls réutilisés
`markup-call-0`, `-1`… régénérés à chaque tour (`openai_compat.rs:478`) —
collision possible dans toute structure indexée par id au niveau session
(replay, approbations mémorisées par id).

---

## C. Trou de couverture de test

- Le SEUL test moteur de tool call (`tests/engine_loop.rs:174-188`) utilise un
  appel unique au pattern séquentiel `Start·Δ·End` — précisément la forme que
  l'adaptateur réel ne produit jamais pour les tours multi-tools.
- Aucun test : multi-tools entrelacés (→ A1), `"stop"` avec tools natifs
  (→ A2), fuite de markup (→ B1), run non-interactif avec approbation (→ A3),
  round-trip `reasoning_content` (→ B4).
- **Règle proposée pour la v0.9 (à ajouter au plan, phase 7)** : tout fix de
  bug provider doit être accompagné d'un test qui simule la séquence SSE
  *réelle* du provider, pas une séquence idéalisée.

---

## Verdict

| Question | Réponse |
|---|---|
| C'est le modèle ou Sparrow ? | **Sparrow** pour l'essentiel : A1+A2 détruisent des appels valides ; A3 rend tout run scripté incohérent. Un modèle moyen aggrave (markup non standard → B1-B3), mais même un modèle parfait échoue sur A1. |
| La suite de tests le voyait-elle ? | Non — tout est vert. La suite teste des séquences idéalisées. |
| Priorité avant la v0.9 | **A1 → A2 → A3** (un correctif v0.8.1), puis B1-B7. Inutile de construire la couche « confiance » v0.9 sur un moteur qui ment (« Done. » sans rien faire). |

Repro disponible : `cargo test --test multi_tool_streaming -- --ignored`

---

# Partie 2 — Session interactive WebView (2026-06-10)

> Méthode : `sparrow console --port 9876` piloté par un vrai navigateur
> (Edge headless via playwright-core, script `scripts/audit-webview.mjs`).
> Conversation, tâche avec écriture de fichier + approbation, rechargement en
> cours d'approbation, tour des panneaux et de la palette ⌘K.
> Artefacts (screenshots, transcripts, frames WebSocket) : `C:/tmp/sparrow-audit/`.

## D. Serveur & process

| # | Constat | Détail |
|---|---|---|
| D1 | **Sécurité : la console écoute sur `0.0.0.0`** | `netstat` : `TCP 0.0.0.0:9876 LISTENING`, alors que `--help` annonce « default 127.0.0.1 ». Toute machine du LAN peut POST `/run`, créer/supprimer des agents, faire écrire des fichiers. Le commit e07d7e39 n'a corrigé que l'URL affichée, pas le bind. **À corriger en priorité absolue.** |
| D2 | Port occupé → erreur OS brute + exit 0 | « Une seule utilisation de chaque adresse de socket… (os error 10048) », aucun message utile (« une console tourne déjà sur 9339 »), et le process sort en **code 0** malgré l'échec. Le endpoint `/healthz` existe précisément pour détecter une console existante — le CLI ne s'en sert pas. |
| D3 | Flag invalide accepté silencieusement | `--bind 127.0.0.1:9876` (port dans l'adresse = invalide) est accepté sans erreur et ignoré ; le serveur retente 9339. Aucune validation de la valeur. |

## E. Chiffres incohérents (confiance brisée)

| # | Constat | Preuve |
|---|---|---|
| E1 | **Tokens comptés en double dans le HUD** | HUD « sent 9,584 tok » vs `RunFinished.tokens.input: 4792` — exactement ×2. Jauge contexte « 32,446 / 131k » vs run « 16,223 tok » — ×2 aussi. L'événement `TokenUsageEstimated` ET l'usage réel sont additionnés. |
| E2 | « recv 0 tok » sur les tours à outils | Confirme B6 (la sortie outillée n'est jamais comptée). |
| E3 | La durée d'un run change au replay | Premier affichage « 8.7s » ; après rechargement le même run affiche « 550ms ». Le replay recalcule au lieu de stocker. |
| E4 | Coût $0.0000 sur un provider payant | Route primaire `deepseek-v4-flash` (tier Cheap, payant) → coût affiché $0.0000 et `cost_comparison: ""` vide. Soit les prix du modèle sont absents de la table, soit le coût n'est pas câblé sur cette route — dans les deux cas le « moat » coût affiche du faux. |
| E5 | 9 584 tokens envoyés pour un « bonjour » classé trivial | Le routeur classe « tier: trivial » mais le prompt système intégral (soul + skills + protocole) part quand même. Le tier devrait moduler le prompt. |

## F. Théâtre d'affichage & incohérences UX

| # | Constat | Détail |
|---|---|---|
| F1 | **Swarm lanes fictives** | Pour un simple message de chat : « planner: analyzing request · 6 candidates », « coder: consulting deepseek… », « verifier: run closed · metrics captured ». Aucun de ces trois agents n'a tourné — c'est le routeur + une complétion. Contradiction frontale avec le protocole anti-simulation du soul. |
| F2 | Tool card « running… » pendant un blocage | Le run attend l'approbation (rien ne tourne) mais la card affiche « running… ». |
| F3 | Diff card cassée sur création de fichier | `poeme.txt` créé avec 3 lignes → diff « +0 −0 », hunk `@@` vide, et boutons accept/reject affichés alors que la card dit « applied ». |
| F4 | Carte d'approbation en jargon brut | Texte interne exposé : « permissions allow autonomy gate to decide. Approve fs_write with args: {…JSON brut…} ». |
| F5 | Historique visible perdu au rechargement | Après F5 le transcript ne montre plus que le run courant, mais le serveur garde le contexte (« context retained · turn #2 », 8k tok). L'utilisateur ne voit plus ce que Sparrow « sait ». |
| F6 | Aucun timeout d'approbation | Un run en attente d'approbation reste « running… » indéfiniment (constaté > 3 min, survit aux rechargements). Aucun rappel, aucune expiration honnête. |
| F7 | Message routeur dupliqué/franglais | « requete: requete trivial · tier: trivial · tools: false ». |
| F8 | Bilinguisme incohérent | Chrome anglais (« Welcome back, Captain »), réponses françaises, routeur franglais. |
| F9 | 404 sur `/facts` et `/route` | Le drawer a des panneaux facts et route ; leurs endpoints n'existent pas (404). Favicon 404 aussi. |
| F10 | Bruit console navigateur | AudioContext démarré sans geste utilisateur (3 warnings), champs API-key « password » hors `<form>`. |

## G. Curator / skills auto-appris — le plus grave côté produit

| # | Constat | Détail |
|---|---|---|
| G1 | **Un skill auto-appris contient une engueulade utilisateur corrompue** | `C:/Users/abdou/AppData/Roaming/sparrow/skills/code-review` : description = « Reusable pattern learned from: non tu as vraiment un problème regarde ce que tu m'as écris… », corps = texte mutilé d'une session d'échec. Trigger « review, pr, diff » → **ce contenu est réinjecté dans le prompt système de toute tâche future mentionnant review/pr/diff**, et il s'affiche dans la palette ⌘K. L'auto-learn doit filtrer (jamais apprendre d'un run en échec/d'une plainte), et ne jamais squatter un nom de skill existant. Marqué `auto_generated: false` à tort. |
| G2 | **Preuve d'une corruption de streaming récente** | Le texte appris documente des syllabes perdues en plein mot (« mesponses précédentes motsqués », « suisagent Sparrow je te réponds act ») sur deepseek-v4-pro — le pattern exact du bug SSE « à rebours → àours » censé être corrigé par le LineBuffer. Une voie de perte de chunks subsiste (UTF-8 multi-octets ? renderer ?). À reproduire en priorité. |
| G3 | **Fuite du protocole UI dans la conversation modèle** | Le skill appris contient « ✓ coder completed · 4487↑ 150↓ tok » — des lignes de statut d'interface ont fui dans le transcript que voit le modèle (l'isolation v0.5.8 a un trou). |

## I. Pourquoi l'appel d'outils reste fragile (synthèse des causes)

Le « mal à appeler les outils » n'a pas UNE cause — c'est l'empilement de cinq,
qui se renforcent :

| # | Cause | Détail |
|---|---|---|
| I1 | **Le garde anti-narration est anglais-only** | `tool_narration_detected()` (`src/engine/mod.rs:3121-3163`) ne matche que « i'll use », « let me check »… Sparrow répond en **français** : « je vais créer le fichier », « laisse-moi vérifier » ne déclenchent JAMAIS le retry. Le mécanisme central anti-« je joue l'outil au lieu de l'appeler » est mort dans la langue de l'utilisateur. Fix : patterns fr (« je vais », « laisse-moi », « je m'occupe de », …) + idéalement détection structurelle (verbe d'action + nom d'outil + zéro ToolUse). |
| I2 | **Quand le modèle appelle bien, Sparrow casse l'appel** (A1/A2) | Tour multi-tools → 1er appel détruit (`unknown`/`{}`) ; `finish_reason:"stop"` → appels jetés. Le modèle reçoit des erreurs d'outils qu'il a pourtant bien formés, « apprend » que les outils échouent, et bascule en narration. Les causes moteur fabriquent le symptôme comportemental. |
| I3 | **Prompt système obèse et contradictoire pour les petits modèles** | ~9,5k tokens pour un « bonjour » : base + main_soul (12 Ko, tribunal/triage) + git + facts + memory + catalogue de skills complet. Le routing cheap-first envoie ça aux modèles les PLUS faibles. En plus le catalogue ordonne « scan this catalog and load every skill **before** writing any code/running any tool » → détour `skill_invoke` imposé avant toute action = un mode d'échec de plus. Et le catalogue contient le skill poubelle (G1). Fix : prompt modulé par tier (trivial = base courte, pas de protocole tribunal), et « load skills » en suggestion, pas en prérequis. |
| I4 | **Récupération de markup limitée à 2 formats** | `tool_markup.rs` ne récupère que DSML et `<invoke>`. Les modèles locaux/cheap émettent aussi ```json {"name":…,"arguments":…}```, `[TOOL_CALL]…`, ou le format natif DeepSeek `<｜tool▁call▁begin｜>` — tous fuient en texte brut et ne s'exécutent pas. |
| I5 | **`tools: true` codé en dur pour tout Ollama** (B5) | Un modèle local sans entraînement function-calling reçoit quand même le bloc tools → il narre ou imite. Lire les `capabilities` réelles via `/api/show`. |

Ordre de réparation : I2 (moteur) → I1 (garde fr) → I3 (prompt par tier) →
I5 → I4. Mesure de succès : taux de tours « narration sans appel » et taux
d'appels d'outils malformés, comptés par run et affichés dans `sparrow doctor`.

## J. « Tout est trop étalé » — diagnostic densité du transcript

Inventaire réel d'UNE tâche d'une ligne (le haïku) dans le transcript :
ligne « ▸ started » + 3 lignes sent/recv/live + ligne route (chaîne complète de
6 modèles) + message routeur + tool card (placeholder puis update) + carte
d'approbation (résumé jargon + 4 boutons) + diff card (cassée) + carte réponse
+ carte « completed » (coût/tokens/durée) — **≈ 11 blocs verticaux, ~2 écrans,
pour une tâche d'une phrase.** Causes :

| # | Cause | Fix |
|---|---|---|
| J1 | La télémétrie vit DANS le flux | sent/recv/live/route/router appartiennent au HUD (chrome bar), pas au transcript. Le transcript ne devrait contenir que : ce que dit l'utilisateur, ce que fait Sparrow, ce qu'il répond. |
| J2 | Chaque event = une card pleine largeur avec marges | Tool propose → running → result = UNE card qui se met à jour, repliée par défaut sur une ligne (`✏️ fs_write poeme.txt ✓`). |
| J3 | La chaîne de route complète s'affiche à chaque run | Une pastille `⬡ deepseek-v4-flash` suffit ; la chaîne au survol. |
| J4 | Métadonnées de fin éclatées | coût + tokens + durée = une seule ligne discrète sous la réponse, pas une card. |
| J5 | Pas de regroupement par run | Chaque run devrait être une section repliable ; les runs passés se replient automatiquement sur « tâche → résultat » en 2 lignes. |

→ Spécifié en détail dans `PLAN_v0.9.0.md` Annexe A.7 (ajoutée ce jour).
Cible : une tâche simple = **3 blocs visibles** (message, action repliée,
réponse), tout le reste à un clic.

## H. Ce qui marche bien (à préserver)

- Conversation fluide, streaming visible, réponse française naturelle de qualité.
- L'approbation **survit au rechargement** de la page et reste cliquable (replay-on-connect) ; le fichier est ensuite créé avec exactement le bon contenu.
- Continuité de session réelle entre les runs (turn #2 avec contexte).
- Palette ⌘K riche, slash-commands documentées, esthétique Captain/Paper soignée.
- `/healthz` répond correctement.

## Priorités consolidées (parties 1 + 2)

1. **D1** bind 0.0.0.0 (sécurité) — une ligne à corriger, risque réel.
2. **A1/A2** tool calls multi-tours détruits (le « bug DeepSeek »).
3. **G1/G2/G3** Curator toxique + corruption streaming + fuite protocole.
4. **E1/E4** chiffres faux (tokens ×2, coût $0) — le « moat » coût n'est pas crédible tant que les chiffres mentent.
5. **A3/F2/F6** approbations : politique non-TTY, statut honnête, timeout.
6. F1 (théâtre swarm), F3 (diff création), F5 (historique au reload), F9 (404).
