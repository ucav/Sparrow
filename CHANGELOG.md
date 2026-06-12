# Changelog

All notable changes to Sparrow will be documented in this file.

## [0.9.2] — 2026-06-11 « The Ring »

> v0.9.2 closes the ring: measure, stabilize, accelerate.
> No public metric without proof in `./artifacts/`.

### Ajouté — Phase 0 audit et baselines
- **Audit v0.9.2 traçable** : `artifacts/audit-v092.md` capture l'état réel du
  monocrate, des surfaces CLI/WebView/TUI, des endpoints, de la CI, des dettes
  D1-D8 et des gaps v0.9.2.
- **Baseline perf reproductible** : `artifacts/perf-baseline.md` consigne build
  release propre, rebuild incrémental tool/engine, startup `hyperfine`,
  `/healthz`, first paint Playwright, taille binaire et `cargo bloat`.
- **Journal de décisions** : `artifacts/decisions-v092.md` documente les écarts
  d'exécution, notamment le target dir isolé `target/v092-release`.

### Ajouté — Phase 1 stabilisation
- **Garde CI anti-stub Rust** : CI et nightly échouent maintenant si
  `todo!()` ou `unimplemented!()` apparaît dans `src/**/*.rs`.
- **Rapport de stubs honnêtes** : `artifacts/stub-audit-v092.md` distingue les
  vrais stubs de production des mentions documentaires ou états expérimentaux
  assumés.

### Corrigé — Phase 1 stabilisation
- **Formatage v0.9.1 remis au vert** : `cargo fmt --all -- --check` repasse
  après formatage des changements existants dans `src/extras.rs`.
- **Rapports `./artifacts/*.md` suivables** : `.gitignore` garde les artefacts
  lourds ignorés mais autorise les rapports Markdown demandés par le plan.

### Corrigé — Phase 2 performance : démarrage CLI (suivi)
- **`--version`/`help` ne paient plus le runtime tokio** : `Cli::parse()` tourne
  désormais avant la construction du runtime multi-thread (parse sur le thread
  worker 16 MB pour éviter l'overflow Windows). Clap rend version/aide et sort
  sans spawner les threads du runtime ni initialiser le tracing.
  - Mesuré (release, 30 runs, même méthode avant/après) : `--version`
    **38 ms → 34 ms** en moyenne, mais surtout **max 98 ms → 40 ms** et
    **écart-type 11 ms → 2 ms**. La moyenne `hyperfine` de 361 ms précédemment
    rapportée était un artefact de variance (pics de spawn du runtime sous
    charge), pas une médiane — démarrage désormais **toujours < 50 ms**, sous la
    cible de 100 ms et prévisible.
- **Clarification dev-loop** : le rebuild **debug incrémental** (moteur touché)
  est de **~9 s** — le vrai cycle de développement est rapide. Les ~191 s du
  rapport concernent le rebuild **release** (`codegen-units = 1`,
  `opt-level = "z"`), un compromis taille/vitesse qui ne s'applique qu'au
  moment des releases/CI, pas au développement courant.

### Changé — Phase 2 performance
- **Workspace Cargo initial** : le contrat `event` vit maintenant dans
  `crates/sparrow-core` et reste réexporté comme `sparrow::event` pour préserver
  les imports existants.
- **Extraction `sparrow-providers`** : les adapters de modèles (trait `Brain`,
  Anthropic, OpenAI-compatible, Ollama, Responses, discovery, `sse_buffer`,
  `tool_markup`, types `ModelCaps`/`Msg`/`ContentBlock`…) — ~3050 lignes —
  vivent désormais dans `crates/sparrow-providers`, qui ne dépend que de
  `sparrow-core`. `crate::provider::*` reste réexporté (zéro import cassé) et
  `provider::detect` reste dans le binaire (il dépend du registre config +
  onboarding). Toucher l'engine ne recompile plus ce cluster d'adapters.
- **Extraction `sparrow-memory`** : la mémoire persistante (SQLite facts, FTS5,
  graphe de connaissances, symbol index) + la redaction de secrets — ~2350
  lignes — vivent dans `crates/sparrow-memory`, qui ne dépend que de
  `sparrow-core` + `sparrow-providers`. Le type `Identity` a migré vers
  `sparrow-core` pour casser le cycle `memory → engine`. `crate::memory::*` et
  `crate::redaction::*` restent réexportés (zéro import cassé) ; la feature
  `treesitter` est propagée à la crate.
- **Extraction `sparrow-config`** : la couche fondamentale config + registre de
  providers + store de credentials (`auth`) + `permissions` + `hooks` +
  `sandbox` + `humanize` — ~4810 lignes — vivent dans `crates/sparrow-config`
  (dépend de core + providers + intel). Cluster entièrement fermé (aucune dep
  arrière sur engine/memory/tools). Les 6 modules restent réexportés ; la feature
  `keyring` est propagée. Total modularisé : **~10 240 lignes** hors du monocrate
  sur 5 crates (core, intel, providers, memory, config).
- **Profil dev explicite** : `debug = "line-tables-only"`, incremental activé,
  dépendances optimisées à `opt-level = 2`.
- **Rapport perf après premier split** : `artifacts/perf-report.md` montre que
  ce split pose la structure mais ne réduit pas encore les rebuilds tool/engine ;
  l'extraction suivante devra déplacer des clusters d'implémentation plus gros.
- **CI perf hebdo/manuelle** : `.github/workflows/perf.yml` mesure
  `sparrow --version`, `sparrow help` et la taille du binaire release, avec
  seuils initiaux issus de la baseline + 15 %.
- **Console fast-start** : `sparrow console --fast` saute la découverte provider
  au boot, désactive l'animation via `?boot=0&fast=1`, évite l'ouverture
  automatique du navigateur et laisse les panneaux se charger à la demande.
- **Préchargements WebView en idle** : les caches de panneaux/commandes se
  préchargent via `requestIdleCallback` en mode normal et restent lazy en mode
  fast.
- **Profil release affiné** : le release profile utilise maintenant
  `lto = "thin"` avec `strip = true` et `codegen-units = 1`.
- **Plan B perf documenté** : les cibles startup/rebuild non atteintes restent
  marquées comme telles dans `artifacts/perf-report.md`, avec la prochaine
  extraction workspace et l'early-exit `--version/help` comme suites mesurables.

### Ajouté — Phase 3 cœur agentique
- **`sparrow audit`** : cartographie locale du repo, écrit
  `./artifacts/audit-<timestamp>.md`, détecte `todo!()`/`unimplemented!()`,
  TODO/FIXME et fichiers Rust suspects non reliés.
- **`sparrow test [--fix]`** : détecte `cargo`, `npm` ou `pytest`, exécute le
  runner et transmet l'échec au moteur de réparation si `--fix` est demandé.
- **`sparrow commit`** : commit uniquement les changements staged, génère un
  message prudent si absent et refuse les diff staged contenant des motifs de
  secrets.
- **`sparrow release prep`** : génère les notes de lancement et migration depuis
  les artefacts locaux, sans métriques inventées.
- **Flags de run prudents** : `--plan-first`, `--dry-run` et `--patch` sont
  branchés ; dry/patch basculent la run en permission `read-only`.
- **Garde anti-destruction S2** : les commandes `exec` contenant `rm -rf`,
  `del /s`, `rmdir /s`, `git clean -fdx` ou `DROP TABLE` sont surclassées en
  risque `Destructive` avant approbation.

### Ajouté — Phase 4 TUI
- **Profil `builder`** : `sparrow mode builder` est accepté, persistant, documenté
  et le splash TUI affiche les entrées Run, Test, Refactor, Git, Debug et Replay.

### Ajouté — Phase 5 WebView premium
- **Cinq panneaux cockpit** : la rightbar WebView expose Timeline, Costs,
  Roadmap, Watched releases et Autonomous tasks en plus des panneaux existants.
  Timeline/Costs se nourrissent du flux WebSocket réel ; Roadmap/Releases/Runs
  lisent des endpoints locaux.
- **Endpoints premium** : `GET /intel/backlog`, `GET /intel/digests` et
  `GET /runs` sont ajoutés. Les endpoints intel lisent uniquement le cache
  SQLite local et ne déclenchent jamais de réseau.

### Ajouté — Phase 6 compatibilité tools/skills
- **Manifest skill v2** : un `manifest.toml` ou `manifest.json` optionnel à côté
  du `SKILL.md` peut déclarer `version` et `allowed_tools`. Le registry sait
  produire une liste de tools restreinte pour une skill invoquée.
- **Manifest tools** : chaque tool expose un `ToolManifest` dérivé de son risque
  et de ses métadonnées (`read`, `files:write`, `network`, `exec`, `auth`), aussi
  disponible via `/tools`.

### Ajouté — Phase 7 auto-amélioration
- **Crate `sparrow-intel`** : scanner public opt-in pour GitHub Releases,
  changelog URL et docs URL, cache SQLite, digests et backlog scoré.
- **CLI `sparrow intel`** : `scan`, `report`, `backlog`, `watch`. Le réseau est
  refusé par défaut si `intel.enabled=false`, sauf source/config passée
  explicitement sur la commande.
- **Preuve réelle** : `artifacts/intel-sources-v092.toml` scanne deux sources
  publiques (`tokio-rs/tokio`, `rust-lang/rust`) et les commandes `report` /
  `backlog` produisent des digests et tickets scorés depuis le cache.

### Changé — Release
- Version crate portée à `0.9.2` pour la release “The Ring”.

## [Unreleased] — v0.9.1 « Le cerveau » (en cours)

> v0.9 a rendu Sparrow *compréhensible*. v0.9.1 le rend *compétent* : il utilise
> ses outils/skills, **apprend** vraiment, **range** ses livrables, et fait
> tourner en console tout ce qui tournait déjà en CLI. Plan : `PLAN_v0.9.1.md`.

### Corrigé — mémoire qui apprend enfin
- **Bug d'amnésie du Distiller** (`src/extras.rs`) : la déduplication se faisait
  sur la **clé seule** (`user:preference`, `user:language`…), génériques. Dès
  qu'un run avait enregistré un fait d'un type, **plus aucun autre** du même type
  n'était jamais sauvé → la mémoire saturait après le 1er run. Désormais la dédup
  porte sur la paire **(clé, valeur)** : chaque fait distinct est conservé.
- **Capture des consignes durables** : le Distiller détecte les formulations
  « retiens / souviens-toi / désormais / toujours / je m'appelle / je préfère »
  (FR + EN) dans les messages utilisateur et les enregistre **mot pour mot**
  comme faits `user:directive:<hash>` (clés sans collision).
- **Soul** : la section mémoire nomme désormais explicitement l'outil — « call
  `memory` with `action:"add"` » dès qu'une info durable est donnée, avec exemple
  ancré. Les modèles faibles ne devinaient pas qu'il fallait appeler l'outil.

### Corrigé — l'agent ne perd plus ses capacités sur les tâches « simples »
- **Catalogue de skills injecté à tous les tiers** : l'index (noms +
  descriptions) était caché en mode lean (Trivial/Small) → sur une tâche
  « simple » l'agent ne **voyait** aucun skill et n'en utilisait jamais.
  L'index léger est maintenant toujours présent ; seuls les corps complets
  (coûteux en tokens) restent réservés aux runs non-lean.
- **Invariants d'action en mode lean** : « appelle les outils sans les narrer,
  scanne les skills, vois le fichier avant d'éditer, sauve en mémoire, range dans
  ./artifacts » sont rappelés même sur les tâches triviales.

### Corrigé — la console se comporte enfin comme le CLI en production
- **Sous-agents activés depuis la WebView** : le moteur console n'instanciait pas
  `agent_store` → l'outil `subagent_spawn` ne pouvait rien lancer, alors que le
  soul ordonne d'en spawner. Câblé (`main.rs`).
- **Hooks de cycle de vie actifs en console** : `with_hooks_config` n'était jamais
  appelé → aucun hook PreToolUse/PostToolUse/OnError configuré ne se déclenchait
  depuis la WebView. Câblé.

### Ajouté — discipline d'artefacts
- **Convention `./artifacts/`** : le prompt système dit à l'agent de ranger ses
  livrables générés (rapports, exports, code généré, diagrammes) dans
  `./artifacts/`, sans polluer la racine ni déplacer les sources existantes.
- Le panneau **Artifacts** de la WebView liste désormais `./artifacts/` (section
  « deliverables ») en plus des fichiers édités et des uploads ; `GET /artifacts`
  scanne les deux répertoires avec un champ `source`.

## [0.9.0] — 2026-06-11 « Anyone »

> Accessibilité radicale : n'importe qui doit pouvoir comprendre et utiliser
> Sparrow. Couche humaine au-dessus du moteur — le mode pro ne perd rien.
> Plan : `PLAN_v0.9.0.md`.

### Ajouté — Pilier 1 « l'entrée par le problème »
- **`sparrow fix` / `sparrow repare`** — le réparateur universel : décris ton
  problème (ou colle une erreur), Sparrow lit les fichiers concernés,
  diagnostique la cause en une phrase simple, propose la correction et
  l'applique après ton accord. Sans argument, il inspecte le dossier courant.
- **`sparrow explique` / `sparrow explain`** — explique un fichier, une erreur
  ou un concept en langage simple, sans rien modifier.
- **Alias en langage humain** (additifs, les commandes d'origine ne bougent
  pas) : `sparrow montre` / `show` → `console`.

### Ajouté — Pilier 1 « l'entrée par le problème » (accueil)
- **`sparrow bonjour` / `hello` / `salut`** — l'accueil chaleureux : Sparrow
  regarde le dossier courant et propose l'action la plus pertinente (conflit
  git → `fix`, projet Node non installé → `fix`, dossier de photos → idée de
  tri), puis liste `fix` / `explique` / `idees` / `whatis` et rappelle
  `annule`. Détection de contexte hors-ligne, FR/EN.

### Ajouté — Pilier 6 « la galerie des possibles »
- **`sparrow idees` / `ideas`** — une galerie de recettes concrètes classées
  par profil (enseignant, grand-mère, artisan, enfant, créateur, développeur,
  expert…), avec le prompt prêt à coller. `sparrow idees enseignant` filtre par
  profil, `sparrow idees "factures"` cherche par mot-clé. La recette EST le
  tutoriel.

### Ajouté — Pilier 3 « installation en un geste »
- **`sparrow launch` zéro question** — le premier lancement prépare une config
  simple, free-first, avec secours local Ollama et détection des clés déjà en
  environnement. Message final : « Sparrow est prêt. Qu’est-ce qu’on règle
  aujourd’hui ? ». L'ancien setup détaillé reste disponible via
  `sparrow launch --pro`.
- **Config lisible par humain** — les fichiers `config.toml` écrits par Sparrow
  commencent par des commentaires clairs : mode simple, clés hors fichier,
  retour au mode expert.
- **`sparrow doctor` humain** — diagnostics en ✅/⚠️/❌ avec phrases et sortie
  de réparation, au lieu d'un dump interne.
- **Packaging v0.9** — scripts one-click avec raccourcis bureau opt-out
  (`--no-shortcut` / `-NoShortcut`) et gabarits `Sparrow-Setup.exe` NSIS +
  `.dmg` macOS.

### Ajouté — Pilier 5 « cockpit pour humains »
- **Panneau d'outils latéral droit** dans la console WebView : Preview, Diff,
  Terminal, Files, Background tasks et Plan, alimentés par le flux d'événements
  réel (diffs `DiffProposed/Applied`, commandes `exec`, tests, URLs locales
  détectées, liste `todo` via le nouvel endpoint `GET /todos`). Ouverture
  manuelle (bouton « ⧉ tools », `Ctrl+Shift+S`) ou automatique avec priorités :
  un échec rouvre le panneau même fermé à la main, un événement mineur jamais.
  Préférence `auto-open` persistée, overlay propre sous 980 px, états vides
  honnêtes (« No changes yet. », « No background tasks running. »).
- **Raccourcis panneau** : `Ctrl/⌘+P` preview, `Ctrl/⌘+Shift+D` diff,
  ``Ctrl+` `` terminal, `Ctrl/⌘+Shift+F` fichiers, `Ctrl/⌘+Shift+P` plan.
- **Preview réelle** : `GET /preview/scan` sonde les ports dev usuels
  (3000, 4200, 5173, 8080…) sur loopback et liste les serveurs vivants ;
  champ URL manuel + intégration iframe dans le panneau.
- **Échec de commande fiable** : l'événement `ToolOutput` porte désormais
  `is_error` (depuis `ToolResult::is_error`, champ additif rétro-compatible) —
  une commande échouée passe la tâche en `failed`, marque la ligne terminal en
  rouge et rouvre le panneau en priorité haute.
- **Demande en langage naturel** : « montre le diff », « ouvre le terminal »,
  « show files »… dans le chat ouvrent directement l'outil correspondant
  (verbe d'affichage + mot-clé, FR/EN).
- **Thème « white »** : interface claire et nette (fond blanc, contrastes
  lisibles, bordures visibles), sélectionnable dans config → appearance ; le
  bouton thème alterne désormais captain → paper → white.
- **Replay branché pour de vrai** : le bouton « ▸ replay » ouvre un sélecteur
  des runs enregistrés (`GET /replays`) et rejoue le transcript choisi
  (`GET /replay?run_id=`, ou le dernier run sans argument) — endpoints adossés
  à `FsRecorder`, garde anti-traversée de chemin. Fini le `prompt()` d'ID.
- **MCP & hooks visibles** : `GET /mcp/list` et `GET /hooks` exposent les
  connecteurs MCP (`mcp_servers.json`) et les hooks de cycle de vie configurés ;
  l'onglet config les rend proprement au lieu d'un « not exposed yet ».
- **Markdown riche dans le chat** : la prose streamée rend gras, italique,
  titres, listes, citations, tables, liens et code inline (échappé d'abord,
  rendu type Claude Code Desktop) au lieu de texte brut.
- **Ghost-text dans le composer** : suggestion grise inline tirée de
  l'historique réel (`GET /history`) pendant la frappe ; `Tab` accepte, `Échap`
  ignore — sensation de shell premium.
- **Cheat-sheet clavier** (`Ctrl/⌘+/`) : overlay listant tous les raccourcis,
  groupés par section, libellés ⌘ sur Mac.
- **Quick-actions d'accueil** : puces cliquables (Répare / Explique / Plan /
  Idées / Raccourcis) qui pré-remplissent le composer.
- **Mockup v0.9** : `sparrow-cockpit-v0.9.0-mockup.html`.
- **Focus/Cockpit** dans la console : Focus par défaut, une colonne lisible,
  toggle persistant, raccourci `Alt+F`; Cockpit garde les métriques avancées.
- **Actions persistantes Focus** : `OK`, `Undo`, `Explain`, câblées
  aux approbations, à `/annule` et à une demande d'explication simple.
- **Accessibilité** : A-/A+ conservés, contraste vérifié par script, ARIA sur
  les contrôles clés, raccourcis clavier Focus (`Alt+A/U/E`), onboarding 4
  bulles maximum stocké en `localStorage`.
- **Micro Focus** : dictée via Web Speech API quand disponible, fallback clair
  vers `sparrow voice transcribe`.

### Ajouté — Phase 7 « GO »
- **Fiche GO v0.9** : `docs/v0.9-go.md` avec métriques, protocole cinq
  personas et commandes de vérification.
- **Garde-fous automatisés** : `tests/v0_9_anyone.rs` verrouille launch
  `--pro`, config commentée, Focus UI, mockup et audit ; `npm run
  a11y:console` lance l'audit Playwright déterministe.

### Ajouté — Pilier 4 « le filet de sécurité »
- **`sparrow budget` / `sparrow budget 2€`** — voir ou changer le plafond de
  dépense par session en langage humain (accepte `2€`, `$0.50`, un nombre nu) ;
  « je m'arrête tout seul avant de dépasser ».
- **`sparrow annule` / `sparrow undo`** — l'annulation à un mot : sans
  argument, revient au dernier point de sauvegarde ; `--tout` revient au début
  de la session. Confirme avant d'agir et dit en clair ce qui a été restauré
  (« tes fichiers sont revenus comme ils étaient à 14h32 »). Réutilise le même
  rewind git-backed que `sparrow rewind`.

### Ajouté — Pilier 2 « zéro jargon » (fondation)
- **Table de traduction `humanize`** (`src/humanize.rs`) : chaque `Event` du
  moteur a une phrase en langage humain (FR/EN) pour le mode simple. Le `match`
  est **exhaustif sans wildcard** — ajouter un nouvel event ne compile plus
  tant qu'il n'a pas sa phrase humaine (verrou anti-régression au compilateur).
  La télémétrie (tokens, coût, route, reasoning) reste muette : elle appartient
  au HUD, pas à la conversation.
- **Réglage `[experience]`** dans la config : `mode` (simple / pro / auto) et
  `language` (fr / en / auto, suit la locale système). Rétro-compatible
  (section optionnelle, défauts auto/auto = simple).
- **`sparrow mode simple|pro|auto`** — choisis comment Sparrow te parle ; sans
  argument, affiche le mode courant. Persisté dans la config.
- **Renderer CLI branché** : en mode simple, `sparrow run`/`fix`/`explique`
  affichent les phrases humaines (« Je crée poeme.txt… », « Je change de
  modèle pour continuer. », « Rien n'a été modifié. ») au lieu des étiquettes
  techniques (`[Tool: …]`, `[Routing] … → …`, `Tokens: X in / Y out`), et un
  coût en centimes (« C'était gratuit. ») suivi du rappel « sparrow annule ».
  Le mode pro garde la sortie technique intégrale, à l'identique.
- **Console WebView branchée** : en mode simple, le serveur attache une phrase
  `human` (issue de la même table, côté serveur — zéro duplication) à chaque
  event ; la console affiche les lignes humaines (« Je change de modèle pour
  continuer. », « ✓ Rien n'a été modifié. », « C'était gratuit. ») au lieu du
  `X → Y (reason)` et de la ligne `$0.0000 · N tok`. Vérifié en live (frames WS
  porteuses de `human`/`simple`, zéro erreur console).
- **TUI branché** : le feed du TUI (`push_event`) affiche les lignes humaines
  en mode simple (`ModelSwitched`, `RunFinished` + coût en centimes) au lieu de
  `fallback: X → Y` et `status: … cost: $…` ; mode pro inchangé. Réglé depuis
  la config (`with_experience`). 3 tests. → CLI + console + TUI partagent
  désormais **la même table humanize**.
- **Accessibilité console (Pilier 5)** : boutons **A− / A+** (et `Ctrl/Cmd ±`)
  règlent la taille du transcript de 0.85× à 1.6×, persistée et restaurée au
  rechargement. La taille effective = base responsive × préférence, donc un
  redimensionnement n'écrase plus le choix. Vérifié live (resize + reload).
- **Erreurs qui rassurent** (Pilier 2.3) : en mode simple, une erreur API/400
  ou réseau ne recrache plus de blob JSON brut — elle devient une phrase calme
  avec une porte de sortie (« je réessaie autrement — si ça persiste, tape
  sparrow doctor »).
- **Glossaire vivant** (`src/glossary.rs`) : `sparrow whatis <mot>` (alias
  `c-est-quoi`, `glossaire`) donne une définition instantanée et hors-ligne,
  en deux phrases simples, du vocabulaire propre à Sparrow (checkpoint, token,
  swarm, MCP, provider, routing, autonomie, tier, agent, skill, budget…).
  Sans argument : liste les mots connus. Terme inconnu : renvoie vers
  `sparrow explique`. FR/EN selon `[experience] language`.

### Corrigé
- **Plafonds de run non appliqués** : `--max-wall-secs`, `--max-tokens` et
  `--max-cost-usd` étaient parsés par clap mais **jamais câblés** (comme
  `--bind` avant v0.8.1). `apply_cli_overrides` les pousse maintenant dans
  `config.budget`, et l'engine les fait respecter : arrêt net au plafond de
  temps (check par tour **et** `tokio::timeout` sur l'attente de chaque event
  d'un stream, pour qu'une complétion lente/bloquée soit interrompue), de
  tokens, et de coût (déjà géré). `--max-cost-usd` prime sur `--budget`. Avant,
  un `sparrow run --max-wall-secs 20` pouvait tourner > 9 minutes. Tests :
  `tests/budget_caps.rs`.
- **400 « Unsupported parameter prompt_cache_key » sur les providers
  OpenAI-compatibles** : `build_chat_body` (`src/provider/openai_compat.rs`)
  n'émet plus jamais `prompt_cache_key` / `prompt_cache_retention`. L'engine
  active le cache pour le run, mais cet adaptateur fronte des dizaines de
  proxys (opencode-go, NVIDIA, Groq, stepfun, Ollama…) dont beaucoup rejettent
  ces champs en HTTP 400 — ce qui faisait échouer le 1er modèle de chaque
  chaîne de routing et gâchait un tour. Le prompt-caching reste une
  fonctionnalité Anthropic (gérée dans `anthropic.rs`). Test :
  `openai_chat_body_never_sends_prompt_cache_params`.
- Deux lints clippy pré-existants sur master (router `for_kv_map`, route
  `unnecessary_unwrap`) qui cassaient `cargo clippy --all-targets -D warnings`.


## [0.8.1] — 2026-06-10 « Honesty »

> Patch release tracking the v0.8.0 audit. No new features.
> See `AUDIT_v0.8.0.md` and `PLAN_v0.8.1.md`.

### Security
- **La console se lie à `127.0.0.1` par défaut** (D1). Avant, elle écoutait
  `0.0.0.0` sans condition alors que `--bind` était parsé mais jamais lu —
  exposant run/agents/écriture de fichiers à tout le réseau local. `--bind`
  est désormais honoré et un avertissement s'affiche quand l'écoute est
  non-loopback.
- Refus propre de démarrer une 2e console sur un port déjà pris : message clair
  + sortie en erreur, via une sonde `/healthz` (D2, plus de « os error 10048 »).
- `--bind` est validé : une valeur contenant un port est rejetée explicitement
  au lieu d'être ignorée en silence (D3).

### Fixed — Tool execution (the "DeepSeek bug")
- **Un tour à plusieurs tool calls n'écrase plus le premier appel** (A1). Le
  moteur accumule chaque appel par `id` ; un test de régression
  (`tests/multi_tool_streaming.rs`) reproduit la séquence SSE réelle.
- Les tool calls natifs en attente sont drainés quand un provider termine en
  `finish_reason: "stop"` au lieu de `"tool_calls"` (A2) — ils s'exécutent au
  lieu d'être jetés.
- `ToolUseEnd` émis dans l'ordre des index, ids synthétiques uniques par
  session (B8).
- Ollama natif interroge désormais `/api/show`, lit les capacités live
  (`tools`) et le vrai contexte (`context_length`/`num_ctx`) ; si le modèle ne
  déclare pas le support tools, Sparrow n'envoie pas de bloc `tools` à
  `/api/chat` (B5/I5).

### Fixed — Markup recovery
- Le parser de secours respecte `string="true"` (plus de coercition de `"123"`
  en nombre) et ne `trim()` plus les valeurs (le `content` d'un fichier garde
  ses espaces/sauts de ligne) (B2).
- La détection de markup exige une structure ouverte ET fermée : une réponse
  qui *parle* de `<invoke …>` n'est plus avalée comme un faux tool call (B3).
- Récupération étendue aux formats d'outils cheap/local qui fuyaient en texte :
  fences JSON `{"name":…,"arguments":…}`, blocs `[TOOL_CALL]…[/TOOL_CALL]`,
  `function.arguments` encodé en string JSON, et format natif DeepSeek
  `<｜tool▁call▁begin｜>…<｜tool▁call▁end｜>` (I4).
- Les deltas de contenu qui commencent comme du markup DSML/DeepSeek sont
  tamponnés jusqu'à décision : plus de fuite de tags fragmentés dans le
  transcript avant qu'un tool call soit reconnu (B1).

### Fixed — Tool calling and learning
- Le garde anti-narration reconnaît le **français** (« je vais créer… »,
  « laisse-moi vérifier… ») : il était anglais-only et donc inerte pour les
  réponses françaises (I1).
- Les prompts système sont modulés par tier : les runs `trivial`/`small`
  utilisent un prompt lean sans protocole tribunal ni catalogue complet de
  skills ; `medium`/`hard` gardent le prompt complet (I3/E5).
- **Curator assaini** : ne crée plus de skill à partir d'un texte de statut UI
  (« ◌ consulting… », « completed · ↑↓ ») ou d'une plainte utilisateur ;
  purge automatiquement les skills empoisonnés déjà sur disque (G1).
- L'historique envoyé aux providers est filtré au point de sérialisation :
  aucune ligne de statut UI (`completed ·`, tokens, consulting/parsing) ne peut
  être réinjectée dans `BrainRequest.messages` (G3).

### Fixed — Approvals and numbers
- Les approbations sont honnêtes en mode non interactif : pas de blocage sur
  `stdin`, refus explicite si aucun handler d'approbation n'est disponible, et
  statut `WaitingForApproval` visible dans le cockpit (A3/F2).
- La carte d'approbation parle en langage humain via `humanize_tool_action`
  (« Sparrow veut créer/modifier/lire… ») au lieu d'exposer le vocabulaire
  interne ou le JSON brut comme message principal (F4).
- Le broker d'approbation web expire après délai dur et retourne `deny` au lieu
  de laisser une exécution “running” indéfiniment (F6).
- Pendant une approbation, la tool card passe à `en attente` et le résumé
  visible ne contient plus le vocabulaire interne `autonomy gate`/`permissions
  allow` (F2/F4).
- Le CLI n'imprime `Done.` et la comparaison de coûts que pour un run réellement
  `completed` ; les runs interrompus/refusés affichent leur statut réel (A3).
- Les usages provider réels remplacent les estimations au lieu de s'y ajouter ;
  les arguments d'outils streamés comptent dans l'estimation de sortie quand un
  tour est tool-only (E1/B6/E2).
- `OutcomeSummary` porte `duration_ms`, et le replay réutilise la durée stockée
  au lieu d'inventer une nouvelle durée au moment de la relecture (E3).
- Les modèles DeepSeek découverts reçoivent un prix d'entrée/sortie non nul
  dans les caps inférées, ce qui réactive les coûts et comparaisons quand la
  route est payante (E4).

### Fixed — End of theater and noise (pass 2)
- Le message du routeur est une ligne française claire (« tâche classée :
  trivial · outils : non… ») au lieu du doublon franglais « requete: requete
  trivial · tier: … » (F7).
- La lane « verifier » ne se marque plus « run closed · metrics captured » à la
  fin d'un run où aucune vérification n'a eu lieu — elle reste honnêtement au
  repos (F1). Le routage n'est plus présenté comme un « planner » qui délibère.
- Favicon inline : plus de 404 `/favicon.ico` à chaque chargement (F9).
- L'AudioContext n'est armé qu'après un premier geste utilisateur — fini
  l'avertissement Chromium au chargement ; champs API-key en `autocomplete=off`
  (F10).
- `reasoning_content` n'est plus capté à la fois en streaming (`delta`) et sur
  le chunk final (`message`) puis concaténé : la source est unique, ce qui
  évite de renvoyer au provider un raisonnement doublé (contexte/coût, risque
  de 400) (B4).
- Le body OpenAI-compatible peut désactiver l'écho de `reasoning_content` avec
  `echo_reasoning=false`, pour les familles de modèles qui refusent ce champ
  dans l'historique assistant (B4).
- Confirmé : la corruption de streaming (syllabes perdues) ne se reproduit plus
  sur la voie live (deepseek via opencode-go) — c'était un artefact d'avant le
  tampon de lignes SSE de v0.5.8 (G2).

### Fixed — Transcript density
- Le transcript normal n'affiche plus les lignes internes `RunStarted`,
  `RouteSelected`, messages routeur, `ApprovalResolved`, checkpoints et
  apprentissages de skills ; ces détails restent disponibles en verbose ou dans
  le cockpit (J1/J3).
- Les tool cards restent dédupliquées par id et repliées par défaut ; les mises
  à jour `proposed/started/output` modifient la même action au lieu d'empiler le
  flux (J2/B7).
- La fin de run est une ligne discrète `status · coût · tokens · durée · fichiers`
  au lieu d'une grande card récapitulative ouverte dans le transcript (J4).
- Un `DiffApplied` sans patch n'affiche plus une hunk vide `+0 −0` avec des
  contrôles accept/reject contradictoires ; il montre seulement le fichier
  appliqué et reste cliquable pour ouvrir le contenu (F3).

### Verification
- `cargo fmt --check`, `cargo test --all-targets` et
  `cargo clippy --all-targets -- -D warnings` sont verts après la fermeture des
  lots 2 à 11.
- `scripts/audit-webview.mjs hello`, `task`, `approve`, `ui` passent avec zéro
  erreur console. Les artefacts d'audit ne contiennent plus `autonomy gate`,
  `permissions allow`, `with args`, `running…`, ni faux `+0/−0`; le log serveur
  ne contient plus de panic `/update/check`.

## [0.8.0] — 2026-06-10

## [0.7.0] — 2026-06-10

### Added
- **Main agent soul: REFLEXION-MAX PROTOCOL V2** — the default agent now
  carries a rigorous reasoning protocol (`src/engine/main_soul.md`, baked in
  at compile time): tier triage, decomposition, a three-reviewer tribunal
  (skeptic / adversary / hurried user), verification by a different method,
  and absolute rules against simulated results, sycophancy, and fake
  certainty. Named agents (planner, coder, …) keep their own focused souls.
- **Verified escalation** — when a model exhausts its fix budget and the
  verify command still fails, the run escalates to the next model in the
  chain (bounded) instead of ending silently unverified; if the chain runs
  out, the run ends honestly as a failure. Cheap-first routing now has a
  quality floor.
- **Per-repo routing memory** — verified run outcomes are recorded per tier
  in `.sparrow/routing_memory.json`; when a tier mostly fails or escalates in
  a repo, the next run starts one tier higher. Self-correcting (stats are
  recorded under the classified tier, and counters decay), local-first, no
  telemetry.
- **Pre-run quote** — `sparrow run` shows tier, estimated token/cost range
  (at list price) and the selected route, then asks to proceed. Skipped with
  `--yes` or when stdin is not a TTY, so CI and pipes never block.
- **`sparrow --continue` / `--fresh`** — continue the most recently updated
  session from any surface, or start clean. Session continuity is now
  visible: runs print "continuing session (N prior messages)" instead of
  silently carrying context.
- **`sparrow skills install gh:user/repo[/path]`** — GitHub shorthand for
  installing skills, on top of the existing git URL and local-path sources.
- Engine: bounded transient-failure retry — a rate limit, timeout, or 5xx on
  the primary model retries the same model (with backoff, honouring
  Retry-After) before falling back, so one 429 no longer silently downgrades a
  long run to a weaker model.
- Engine: stuck-loop guard — a turn that repeats the exact same tool calls is
  nudged to change approach on the 3rd repeat and the run is stopped honestly
  on the 5th, instead of burning the remaining turn/budget allowance.
- Console: replay-on-connect — opening or refreshing the cockpit mid-run now
  replays the current run's events (bounded ring, cleared per run) instead of
  showing a blank feed.
- `sparrow import <claude-code|codex|opencode|openclaw|auto>` — one-command
  migration of agents, slash commands, settings, and MCP servers from other
  tools. `auto` detects every installed tool and imports each one.
- Cost comparison on run completion: every surface shows what the same token
  volume would have cost (estimated at list price) on Claude Code, Codex CLI,
  and OpenCode.

### Changed
- Budget caps (`--max-cost-usd`, `--max-wall-secs`, `--max-tokens`, `--budget`)
  and other per-run flags are now global: `sparrow run "task" --max-cost-usd
  0.50` works as documented (previously the flags were only accepted before the
  subcommand).
- `sparrow checkpoint list` shows timestamps and short ids instead of raw
  UUIDs, and points at `sparrow rewind`.
- Cost comparison output marks figures as estimates and says "comparable on
  this run" instead of "same cost" when Sparrow wasn't cheaper.
- `sparrow doctor` closes with a support pointer instead of internal milestone
  jargon.

### Fixed
- Build break: `sparrow import` settings merge used the wrong settings type.
- Build break: run summary moved `total_tokens` before the cost comparison
  could borrow it.
- Test suite updated for the new `OutcomeSummary.cost_comparison` field and the
  real `MigrationResult` shape.

## [0.6.2] — 2026-06-09

Launch-polish pass — cockpit HUD hardening and the first headless render tests.

### Fixed
- **TUI cockpit header no longer truncates on an 80-column terminal.** The
  status line is now split into a left zone (spinner · wordmark · verb · agent ·
  route) and a reserved, right-aligned HUD zone (cost · tokens · autonomy pill).
  Only the route truncates — with an ellipsis — when space is tight; the budget
  and autonomy readouts stay visible at standard width.

### Added
- Headless TUI render coverage (`tests/tui_render.rs`) driving the real `render`
  tree through `ratatui::TestBackend`: boot splash, cockpit HUD, swarm lanes,
  diff panel, checkpoint timeline, toast, replay, and a panic-free sweep from
  40 to 160 columns. The render path had no test coverage before.

## [0.6.1] — 2026-06-09

### Fixed
- Console: agent creator centered as a proper modal.

### Changed
- Engine: inject the full skill catalog into the system prompt so agents
  discover every available skill.

## [0.6.0] — 2026-06-09

Cockpit & agent launch — compact swarm lanes, tiered approvals, agent creator.

### Added
- Compact swarm cockpit: idle agents stay small, active triad expands
- Tiered approvals: once/session/always/deny with client-side rule memorization
- Agent creator: full form, writes `.soul.toml` + `.agent.md` instantly
- Config panel v2: 6 tabs (providers, routing, permissions, appearance, memory, MCP)
- Skills tab in left rail
- `GET /skills` endpoint
- `POST /agents`, `DELETE /agents/:name` endpoints
- TUI rich splash screen with formatted code/diff/JSON demo on boot
- Agent toggle in TUI: @agent + Tab switches full pipeline identity
- Auto-detect content formatting in TUI (code, diffs, JSON, markdown)

### Changed
- WebView stream prefix: "sparrow ›" instead of leaking agent role
- Skill library scan promoted to pub for API endpoint
- README updated to v0.6.0

## [0.5.8] — 2026-06-09

Audit-driven polish — every defect surfaced by `cargo test --all-targets`,
`cargo clippy --all-targets -- -D warnings`, and a live probe of the cockpit.

### Fixed
- `tests/docs_site.rs` referenced `docs/tutorials/first-launch.mp4`, which
  was removed from the repo to keep size down. The test now matches the
  set of tutorial videos that actually ship.
- `src/console.rs`: `GET /healthz` now returns `200 {"ok":true}`. The VS
  Code extension probes exactly this endpoint to detect a running
  cockpit; without it the extension spawned a duplicate `sparrow console`
  on every launch.
- `src/tui/ansi_bridge.rs`: dropped a no-op `let mut style = style;`
  shadow that tripped clippy under `-D warnings`.
- `src/tui/mod.rs`: replaced a manual `&line[1..]` slice with
  `strip_prefix('@')` so the `@agent` toggle slice is bounded by the
  same condition that proved the `@` was there.
- `src/main.rs`: dropped an orphaned `use sparrow::engine::Identity` and
  documented the `#[cfg(test)] mod webview_protocol_tests` placement so
  the strict-clippy run stays green.

## [0.5.7] — 2026-06-09

### Added
- `web_search` and `code_exec` tools wired into the engine; 8 skills
  enriched with their usage.

## [0.5.6] — 2026-06-08

### Added
- TUI `ansi_bridge` module: render ANSI-coloured agent output inside the
  TUI cockpit panels.

## [0.5.5] — 2026-06-08

Critical agentic-loop fix — multi-tool turns no longer abort against thinking-mode models.

### Fixed
- **Multi-file / multi-tool tasks completed only half-way** against DeepSeek /
  Qwen / Moonshot "thinking-mode" providers (e.g. the `opencode-go` route).
  Root cause: when a single model turn emitted N tool calls, the engine
  replayed them as N *separate* assistant messages, and only the first carried
  `reasoning_content`. The 2nd+ tool-call messages lacked it, so the provider
  rejected every subsequent turn with `HTTP 400 "reasoning_content must be
  passed back"`. The run aborted after the first tool round, leaving tasks
  half-done (e.g. a test file written but the module it imports missing) while
  burning tokens on ~60 rejected retries.
  Fix: a single model turn is now replayed as **one** assistant message
  carrying `reasoning_content` + the full `tool_calls` array, followed by one
  tool-result message per call — the correct OpenAI/Anthropic shape, accepted
  by thinking-mode providers.
  Verified end-to-end: a "create reverse.py + test_reverse.py" task now writes
  both files and the test passes; token use dropped from 212k in / 0 out to
  31k in / 148 out on the same task.
- **Output token accounting showed 0** on the affected runs. This was a
  downstream symptom of the 400-retry loop above (no completion ever
  succeeded); output tokens are now counted normally.

### Known issues
- Streaming tool-call labels can briefly render `[Tool: unknown]` when a
  provider sends the tool name in a later delta than the call id; the tool
  still executes correctly. Cosmetic, tracked for a follow-up.
- `sparrow plan` produces a deterministic template, not an LLM-authored plan.
  Tracked for a follow-up.

## [0.5.4] — 2026-06-07

Launch-ready pass — every public-facing asset needed for HN/X/Reddit day.

### Added
- `assets/sparrow-demo.cast` — real 30-second asciinema demo in English
  (was 3 lines of error output; now a working cast playable via
  `asciinema play` or uploadable to asciinema.org).
- `assets/launch/og-card.svg` — Open Graph card (1200×630).
- `assets/launch/x-hook-receipt.svg` — the `$847` visual for the X hook.
- `assets/launch/comparison-card.svg` — vs Claude Code / Codex / Aider table.
- `assets/launch/cli-demo-card.svg` — terminal still for threads without GIFs.
- `assets/launch/install-card.svg` — 6 install channels at a glance.
- `docs/launch/hn-show.md` — final Show HN copy + post hygiene + timing.
- `docs/launch/x-thread.md` — 8-tweet thread, media rotation, mention list.
- `docs/launch/x-hook-variants.md` — 5 A/B variants of the hook tweet.
- `docs/launch/reddit-rust.md` — r/rust post (factual, no marketing).
- `docs/launch/reddit-localllama.md` — r/LocalLLaMA post (offline-first).
- `docs/launch/reddit-programming.md` — r/programming essay form.
- `docs/launch/producthunt.md` — name, tagline, description, maker first
  comment, gallery slots.
- `docs/launch/devto-longform.md` — 1500-word essay for SEO/Dev.to.
- `docs/launch/press-kit.md` — one-liner, 30s, 2min, 5min pitches, bios.
- `docs/launch/faq-hn-preloaded.md` — 15 HN-typical questions pre-answered.
- `docs/launch/voice-guide.md` — voice rules + banned words list.
- `docs/launch/checklist.md` — J-7 → J+7 launch playbook.
- `docs/launch/responses/01..10.txt` — 10 pre-loaded reply files.
- `SUPPORT.md` — channel routing, response-time expectations.
- `.github/FUNDING.yml` — GitHub Sponsors button.

## [0.5.3] — 2026-06-07

Adoption pass — packaging, IDE, drop-in compat, opt-in telemetry, signed releases.

### Added
- **Homebrew tap** (`packaging/homebrew/sparrow.rb`) updated to v0.5.3 with
  per-arch SHA256 slots — installable via `brew install ucav/tap/sparrow`.
- **Scoop manifest** (`packaging/scoop/sparrow.json`) for Windows install via
  `scoop install sparrow`.
- **winget manifest** (`packaging/winget/ucav.Sparrow*.yaml`) for Windows 11
  install via `winget install ucav.Sparrow`.
- **VS Code extension scaffold** (`ide/vscode/`) — embeds the Sparrow cockpit
  in a side panel, exposes `Sparrow: Run`, `Plan`, `Rewind`, `Open Cockpit`.
- **Claude Code drop-in compat** (`src/onboarding/claude_compat.rs`) — reads
  `~/.claude/CLAUDE.md`, `~/.claude/commands/*.md`, `~/.claude/agents/*.md`,
  `~/.claude/settings.json`, plus the same in `.claude/` of the current project.
  Zero-effort migration from Claude Code to Sparrow.
- **Opt-in telemetry skeleton** (`src/telemetry.rs`) — off by default, closed
  enum of event kinds, never sends prompts/file content. Documented in
  `PRIVACY.md`.
- **`PRIVACY.md`** — explicit privacy policy covering local storage, third
  parties, gateways, sharing.
- **`docs/getting-started.md`** — 60-second quick start covering every install
  channel and budget caps.
- **Nightly CI workflow** (`.github/workflows/nightly.yml`) — fmt/clippy/test
  on 3 OS, `cargo audit`, budget-capped smoke test against Groq.
- **Release signing workflow** (`.github/workflows/release-sign.yml`) — cosign
  keyless signing of every release asset, signatures uploaded to the same
  release.

### Changed
- **CLI flags** added: `--max-cost-usd`, `--max-wall-secs`, `--max-tokens`,
  `--bind` — hard stop / network binding controls usable from every command.
- **README** — comparison table vs Claude Code / Codex / Aider at the top,
  multi-channel install block (cargo, brew, scoop, winget, curl), TOC link to
  the new `Why Sparrow` section.

## [0.5.2] — 2026-06-07

Unified release — upward merge of every feature shipped in parallel on the public master (v0.4.0 → v0.5.1) with the local hardening track (v0.3.5 → v0.3.6). No feature was dropped on either side.

### Added (from local hardening track, kept)
- **Tier2 autonomy hardening** — encrypted credential fallback store,
  honest sandbox reporting for unsupported hardened backends, knowledge graph
  tool persistence test.
- **Playwright computer-use e2e** — full browser sandbox regression suite.
- **Docs hub** — searchable static site + real tutorial videos.
- **Webview polish** — slash command palette runner, agent picker drawer,
  coder lane label updates when an agent is selected, base64-safe agent
  identity prefixing.
- **One-click installers** — `install.ps1` (Windows) and `install.sh`
  (Linux/macOS), `sparrow launch` first-run flow.

### Added (from public launch track, merged upward)
- **`cargo install sparrow-cli`** — crate published on crates.io.
- **Phase1** — CLI rich rendering engine (renderer + code/diff/json/markdown/table formatters).
- **Phase2** — streaming + chat composer (live events, progress, sessions).
- **Phase3** — TTS, STT, file_search tools + 30 community skills.
- **Phase4** — Memory CLI + Session FTS5 search.
- **Phase5** — Voice command (`sparrow voice {speak,transcribe,providers}`).
- **`sparrow demo`** — self-contained snake game demo.
- **`sparrow share`** — uploads latest session to a GitHub Gist.
- **`sparrow hook {install,scan}`** — pre-commit secret scanner.
- **`sparrow setup`** — first-run wizard with provider auto-detection.
- **Humanized French error messages** (`src/errors.rs`).

### Security
- **Nova agent files untracked** — `agents/nova.*` is now gitignored;
  PII (name, family, financial goals, business strategy) is no longer
  committed to the repo.

### Changed
- Crate renamed to **`sparrow-cli`** for crates.io with `[lib]` exposed for
  embedding. Binary name remains `sparrow`.
- Cargo deps unified: `dialoguer`, `walkdir`, `syntect`, `pulldown-cmark`,
  `indicatif`, `console` pulled in alongside the existing audio/lettre/imap
  feature set.

## [0.4.0] — 2026-06-05

Public launch readiness — crates.io publish, first-run wizard, live demo, community skills, and security hooks.

### Added
- **`cargo install sparrow-cli`** — published on crates.io. One-command install.
- **First-run wizard** (`sparrow setup`) — auto-detects 20+ API keys from env,
  validates them, ranks providers by cost tier (free first), one-click setup.
- **`sparrow demo`** — self-contained snake game coding demo in 30 seconds.
  Shows live Planner→Coder→Verifier pipeline with colored terminal output.
- **`sparrow share`** — reads latest session transcript, formats as Markdown,
  uploads to GitHub Gist via `gh` CLI or API.
- **`sparrow hook install`** — installs pre-commit security scanner that blocks
  commits containing secrets, tokens, API keys, or private keys.
- **`sparrow hook scan`** — one-off security scan of staged or all files.
- **Provider auto-detection** (`src/provider/detect.rs`) — scans environment,
  validates keys with lightweight API calls, ranks by cost tier.
- **Humanized French error messages** (`src/errors.rs`) — translates HTTP 401,
  429, connection errors, config errors into actionable French messages.
- **10 community skills** bundled in `skills/`: explain-error, generate-commit,
  write-unit-tests, review-my-pr, refactor-function, onboard-newbie, fix-bug,
  optimize-sql, api-docs, deploy-check.
- **Pre-commit hook script** (`hooks/pre-commit`) — bash script scanning for
  GitHub tokens, OpenAI keys, Anthropic keys, AWS keys, private keys, passwords.

### Changed
- **Cargo.toml metadata** — improved description, added authors, keywords,
  categories, repository link.
- **`rust-toolchain.toml`** — pins Rust 1.96.0 with rustfmt and clippy.
- **SECURITY.md** — updated supported versions to 0.3.x (current).
- **README** — crates.io and docs.rs badges, `cargo install` as primary method.

### Fixed
- **Dependencies** — added `dialoguer` (interactive prompts) and `walkdir`
  (recursive file scanning) for wizard and hook features.

## [0.3.6] — 2026-06-05

Public distribution pass for one-click installation and simplified launch.

### Added
- `sparrow launch`: runs first-launch setup when needed, then opens the
  WebView cockpit on port 9339. `sparrow launch --tui` keeps the same
  onboarding path but starts the terminal cockpit.
- Windows one-click installer (`install.ps1`) that installs to the user-local
  Sparrow bin directory, adds it to the user PATH, and launches Sparrow.
- Linux/macOS one-click installer (`install.sh`) that downloads the latest
  release artifact from `ucav/Sparrow`, falls back to a source build when an
  artifact is unavailable, and launches Sparrow.
- Release/distribution regression tests that pin installer repository targets,
  expected artifact names, and docs coverage for `sparrow launch`.

### Fixed
- The public installer no longer points at the old placeholder
  `sparrow-dev/sparrow` repository.
- The Release workflow now publishes `sparrow-windows-x86_64.exe` without
  accidentally appending a second `.exe`.
- The Release workflow explicitly grants `contents: write` for GitHub release
  publishing.

### Validation
- `cargo fmt --all -- --check`
- `cargo check --all-targets`
- `cargo clippy --all-targets -- -D warnings`
- `cargo build --release`
- `cargo test`
- `bash -n install.sh`
- PowerShell parser validation for `install.ps1`

## [0.3.5] — 2026-06-04

Public polish pass for the WebView cockpit and repository surface.

### Added
- Prompt caching request controls for Anthropic Messages and
  OpenAI-compatible/Responses providers. Anthropic stable system prompts receive
  `cache_control`; OpenAI-compatible requests receive `prompt_cache_key` and
  `prompt_cache_retention`.
- WebView plan mode now has explicit accept, edit, and reject actions.
- Diff cards and the side diff panel now render patches as per-hunk review
  blocks with accept/reject states.
- Local syntax highlighting for streamed code cards, covering common languages
  without a CDN dependency.
- Regression tests that lock code-card styling, metric-spam behavior, and
  WebView typography, plan actions, provider cache payloads, and hunked diffs.

### Fixed
- `CostUpdate` events no longer print one transcript line per streamed metric
  tick; they update the live meters only.
- Empty prose fragments between adjacent fenced code blocks are removed, so
  collapsed cards no longer create large blank gaps.
- Code cards are denser, easier to scan, and preserve copy/line-count behavior.

### Polished
- WebView typography now uses a system UI stack with a modern monospace stack
  for code.
- Root-level scratch logs, local handoffs, and presentation-only artifacts were
  removed from the public tree.
- README status and release metadata now reflect the v0.3.5 build.

## [0.3.3] — 2026-06-03

Public-release hardening pass. Fixes every issue from hands-on testing and
stabilizes the WebView cockpit for local use.

### Fixed (critical)
- **Context truly persists now.** The engine emits the assistant response as
  `ThinkingDelta`, not `Message`, so the previous capture never fired. The
  WebView command loop now accumulates streamed deltas and flushes one
  assistant turn into a persistent history on `RunFinished`. Context survives
  model switches AND separate prompts.
- **Sessions persist across restarts.** The conversation is saved to the
  SQLite `SessionStore` under id `webview` and re-hydrated on launch — it
  appears in the `/sessions` panel and reloads next time.
- **Counters animate live and never stick at zero** (direct `textContent`
  write + easing layer; the rAF-only path failed in background tabs).
- **All models are visible.** `/models` merges live-discovered models with the
  curated registry — NVIDIA jumps from 6 to 82 models in the picker/config.

### Added
- **Stop button** next to Run aborts the active task (`POST /stop`).
- **Mid-run injection**: typing while a task runs injects the text into the
  live run's context instead of starting a new run.
- **Collapsible code cards**: assistant code output is split out of the stream
  into `<details>` blocks with language label, line count, and copy button.
- **Auto-collapsing run summary** that re-expands on click.

### Polished UI
- Fixed vertical char-by-char text wrapping (`overflow-wrap` instead of the
  deprecated `word-break:break-word`).
- Drawer shows only the clicked panel, not all metrics stacked.
- Context bar fills with a green → amber → coral → red gradient.
- Animated pulsing green dot on the active session.
- Removed the duplicated dollar figure; budget shows `unlimited` when unset.
- Removed the misaligned fixed "live" tag (the chrome bar already has one).

## [0.3.2] — 2026-06-03

### Fixed (critical bugs from user report)
- **Conversation context was dropped on every new run/model switch.**
  `main.rs` now keeps a persistent `Arc<Mutex<Vec<Msg>>>` of the dialog
  shared across runs; every command pulls prior turns into `Task.context`
  (previously hardcoded to `vec![]`) and pushes user+assistant messages
  back, capped at the last 40 turns.
- **Cockpit counters never updated** because the rAF-based `countUp`
  didn't fire in background tabs / headless contexts. `refreshTokens()`
  now writes `textContent` directly first, then runs `countUp` as an
  optional easing layer. Tokens, cost, and budget all animate live.
- **No way to see which providers / models are configured.** The full
  35-provider · 60+-model registry is now visible in the config panel
  as expandable cards (was a single dropdown showing one provider at a
  time). Search, default-route picker, and per-provider key input added.

### Added
- **`POST /conversation/reset`** endpoint + UI button (`⟲ new conversation`)
  that clears retained context when the user wants a fresh start.
- **Live "context retained · turn #N"** indicator on every `RunStarted` from
  turn 2 onward; **"● context preserved"** marker on every `ModelSwitched`.
- **BUDGET pill** in cockpit row: live `<pct>% / $<daily> day` with color
  gradient (green → gold → coral → red) as the cap is approached.
- **`/conversation/reset`**, **`/status`**, **`/file`**, **`/models`**
  endpoints fully wired.
- **Config panel** rebuilt with 3 tabs (providers&models / routing&autonomy /
  permissions). Each provider card shows LED health · adapter · notes ·
  per-model row with ★ recommended, ctx window, $/Mtok cost, "set default"
  button, and API key input.

### Polished
- Welcome panel: shows `<N> providers · <M> models · <K> with key` and
  the boot timestamp.
- Route bar: 3-hop fallback chain with animated arrows.
- Tool cards: summary like `2 files · 6 KB` for fs_read, `<N> hits` for
  search, matching the validated mockup.

## [0.3.1] — 2026-06-03

### Added — Rust
- `GET /models` endpoint: returns the full provider registry (35 providers,
  60+ models) with `context_window`, `cost_in`, `cost_out`, and `recommended`
  fields — used by the WebView model picker and provider health dots.
- `RunRequest.model_override`: optional field; when set the backend injects
  `__model:X__` into the task string.
- Engine parses and strips `__model:X__` prefix; filters the routing chain to
  the requested brain ID (falls back to full chain if not found).
- `list_agents` scans three directories (user config, `agents/`, `.sparrow/agents/`)
  and deduplicates by name — the 5 repo soul files appear in the Crew panel
  without any install step.
- `RouteSelected` event now carries `context_window: u64` from the primary
  brain's `caps()`, fixing the hardcoded 32k display in the WebView.

### Added — WebView (`console.html`)
- **Model picker dropdown**: the `⬡ auto` pill expands to a full popover
  grouped by provider; each model shows context window and cost; provider rows
  show a green/dim LED based on `has_credential` from `/config`; selection
  stored in `localStorage` and injected into `POST /run`.
- **Side diff viewer**: `.diff-panel` slides in from the right on every
  `DiffApplied` event; shows file path, `+N −M` stats, and colour-coded diff
  lines; `D` key and `✕` button toggle it; `Esc` closes.
- **Run summary card**: replaces the bare `✓ done` log line with a structured
  card showing cost, token count, files edited, checkpoint count, and duration.
- **Dynamic context bar**: `contextLimit` updates from `RouteSelected.context_window`
  and shows the real model limit (200k for Claude, 131k for NVIDIA/Llama4, etc.).
- **Provider health dots**: model picker dropdown renders a green LED next to
  providers that have a credential configured; dim dot for unconfigured.
- **Crew panel live status**: `AgentSpawned` and `AgentStatus` events update
  each agent row's status pill and note in real time via `updateCrewLiveStatus()`.
- **Checkpoint scrubber**: `addCheckpointNode()` stores checkpoint IDs; each
  node is clickable — confirms and issues `/rewind <id>` via the composer.
- **Dynamic home screen**: `bootIntro()` is now async; fetches `/models` and
  `/config` at startup to show real provider counts and active-provider count.
- **Hero stats**: `hero-active` shows count of providers with credentials;
  `hydrateHero()` caches configured provider IDs for the model picker dots.
- **Keyboard shortcuts**: `D` toggles diff panel; `P` opens model picker;
  both disabled when focus is inside an input/textarea.
- **Escape**: closes diff panel and model picker dropdown in addition to
  existing modals (approval, config, palette, history draft restore).

### Added — Agents / Docs
- Agent SOUL files enriched: `description`, `color`, `max_turns` added to all
  five repo agents (planner, coder, verifier, debugger, researcher); prompts
  tightened with step-by-step rules and better model assignments.
- `docs/keyboard.md`: updated with new shortcuts (`D`, `P`, `Esc` additions)
  and a WebView panels reference table with rail icons and descriptions.

## [0.3.0] — 2026-06-02

WebView Cockpit v0.3.0 is shipped. The console now matches the presentation
direction as a real local cockpit rather than a static mockup: every panel is
fed by Sparrow endpoints, the composer is usable as a primary command surface,
and both Captain/Paper themes are covered by tests.

### Added
- Three-column WebView cockpit: icon rail, persistent drawer, main event
  stream, live cockpit metrics, boot animation, route chain, token/cost/context
  counters, and dynamic swarm row from `GET /agents`.
- Drawer panels wired to real endpoints: `/sessions`, `/memory`, `/plugins`,
  `/tools`, `/permissions`, `/security`, and `/artifacts`.
- Typed event renderers for tool cards, diff cards, compaction banners,
  checkpoint timeline nodes, streaming text caret, skill notifications, route
  updates, autonomy changes, token/cost updates, and model fallback labels.
- Command composer upgrades: `Cmd/Ctrl+K` slash palette, inline `@<agent>`
  picker, server-backed `/history`, multiline `Shift+Enter`, persisted draft,
  paste attachment handling, real `POST /upload`, drag-and-drop overlay, and
  live context meter.
- Approval modal wired to `POST /approval`, replacing inline-only approval
  controls for new approval events.
- Paper theme coverage, theme query override for screenshots/tests, persisted
  theme toggle, and reduced-motion fallback that disables animations, embers,
  transitions, and the boot overlay.
- WebView keyboard shortcuts documented in `docs/keyboard.md`.

### Validation
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all-targets`
- `cargo build --release`
- HTTP smoke: `GET /`, `/history`, `/agents`, `POST /upload`, and
  `GET /artifacts`.

## [0.2.0] — 2026-06-02

Autonomy mega-prompt phases 1–13 are landed and the deferred items have been
closed. Every WebView/CLI surface advertised in the README is wired and tested.

### Added — phases 1–11
- Read-only plan mode + slash command loader (`sparrow plan`, `/plan`).
- Permission modes (`read-only` / `plan` / `supervised` / `trusted` /
  `autonomous` / `emergency-stop`) and lifecycle hooks (`PreRun`, `PreToolUse`,
  `PostToolUse`, `PreCheckpoint`, `PostCheckpoint`, …).
- Declarative agents with `.agent.md` frontmatter and `agent mention`.
- Bounded `MEMORY.md` / `USER.md`, anti-injection memory guards, SQLite FTS
  session search, `memory` tool.
- Progressive skills (references + templates + scripts + assets loaded on
  invoke), plugin manifests, scanner, namespaced commands.
- Tool metadata (toolset / risk / auth / mutation / network / exec) with
  surface-aware filtering.
- Scoped gateway sessions, `gateway health`, `gateway abort`, sessions
  list/export/cleanup.
- `sparrow security audit [--json]` and WebView `/security`.
- Sandbox policy: protected paths, env allowlist, Docker/SSH/Worktree backends,
  honest errors when vendor CLIs (Modal/Daytona/Vercel/Singularity) are missing.
- `vision`, `image_generate`, `text_to_speech`, `transcribe` tools and WebView
  `POST /upload` (10 MB cap, classified) + `GET /artifacts`.
- `sparrow github review|status|logs` CLI plus composite `action.yml` and a
  sample PR-review workflow.

### Added — phase 12 (context & compaction)
- `ContextMeter` over prompt/memory/tools/attachments/transcript.
- `HookEvent::PreCompact` and `PostCompact`, `Event::Compacted`.
- `sparrow compact` writes a durable `HandoffDoc` Markdown.
- Engine auto-compaction: when the transcript exceeds 120k chars the loop
  collapses earlier messages, writes the handoff to `.sparrow/handoff/`, and
  emits `Event::Compacted` so the UI can render the pass.

### Added — phase 13 (UI / TUI)
- WebView `GET /sessions` endpoint.
- TUI `@<name>` inline agent picker with autocomplete.
- Theme variants `captain`, `midnight`, `paper` selected via `$SPARROW_THEME`.
- `docs/keyboard.md` listing every shortcut.

### Added — v0.2.0 stability pass
- `gateway::RunRegistry`: gateway daemon now registers spawned runs and
  honours `sparrow gateway abort <run>` by actually cancelling the matching
  task (previously only a signal file was written).
- Engine reflects auto-compaction in the event stream; the threshold and the
  `keep_last` window are constants so the behaviour is reproducible.
- Docs updated: `docs/keyboard.md`, `docs/security.md`, `docs/sandboxing.md`,
  `docs/media.md`, `docs/github-action.md`, `docs/compaction.md`,
  `docs/cli-reference.md`.
- README status table promoted from Alpha → Stable for everything covered by
  unit + integration tests.
- New regression suite `tests/v0_2_stability.rs` pinning the run-registry
  abort path, the TUI agent picker, and `CARGO_PKG_VERSION = 0.2.0`.

### Validation
All of the following pass on `master` at the v0.2.0 cut:
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all-targets` (26 test binaries, 188 assertions)
- `cargo build --release`

## [0.1.0] — 2026-05-31

Initial kernel and feature surface.

### Added
- Complete Rust kernel: config, auth, provider, tools, sandbox, router, engine, autonomy.
- 35-provider registry with model tags and recommended defaults.
- Agentic engine loop with budget-aware routing and fallback chains.
- Multi-agent swarm orchestrator (Planner → Coder → Verifier).
- 4-tier persistent memory (SQLite): repo, identity, task, shared.
- Self-improving skill system with Curator.
- Cron scheduler with job persistence.
- Run recorder/replayer with transcript format (`inputs.json` + `events.jsonl`).
- Terminal TUI (ratatui) with cockpit, scroll, ASCII mascot.
- WebView console (HTTP + WebSocket + JS event client).
- Gateway transports: Telegram, Discord, Slack, WebSocket API, plus experimental extra adapters.
- CLI grammar (clap) with 20+ subcommands.
- `--json` NDJSON output for CI/hooks.
- Install script (`install.sh`).
- 84 local tests across unit/integration/bench harnesses.
- Branding assets: SVG mascot, cockpit mark, ASCII variant, presentation HTML.
- Honest module audit at `docs/AUDIT.md`.
- README status model based on Stable/Alpha/Partial/Experimental/Planned evidence.
