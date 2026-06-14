# Audit UI/UX Sparrow WebView + TUI

Date: 2026-06-14  
Workspace: `C:\Sparrow`  
Port WebView audité: `http://127.0.0.1:9888`

## Statut court

- La proposition validée pour la zone de texte WebView a été intégrée dans `console.html`.
- Les tests existants passent après cette intégration.
- Le dépôt est déjà sur `master`, avec `master` en avance sur `origin/master`. Aucun merge Git supplémentaire n'a été lancé, car le workspace contient aussi des changements non liés, notamment `package.json`.
- Cet audit couvre le rendu graphique, les espacements, les chevauchements, les panneaux, boutons, onglets, états vides, interactions et incohérences WebView + TUI.

## Preuves collectées

### Tests et audits passés

- `cargo test --test ui_finalisation`: 19 tests passés.
- `cargo test --test theme_paper`: 2 tests passés.
- `cargo test --test tui_render`: 12 tests passés.
- `cargo test --test rightbar_panel`: 5 tests passés.
- `node scripts/audit-a11y.mjs`: passé.

### Audit partiellement bloqué

- `node scripts/audit-webview.mjs ui` a expiré après environ 124 secondes.
- Le script semble trop fragile ou trop long pour servir de garde de validation principal. Il doit être modernisé ou remplacé par des tests Playwright plus ciblés.

### Captures générées

- `C:\Sparrow\artifacts\text-zone-implemented-preview.png`
- `C:\Sparrow\artifacts\ui-ux-live-audit\desktop-focus.png`
- `C:\Sparrow\artifacts\ui-ux-live-audit\desktop-cockpit.png`
- `C:\Sparrow\artifacts\ui-ux-live-audit\mobile-focus.png`
- `C:\Sparrow\artifacts\ui-ux-live-audit\mobile-cockpit.png`
- `C:\Sparrow\artifacts\ui-ux-live-audit\rightbar-preview.png`
- `C:\Sparrow\artifacts\ui-ux-live-audit\palette-desktop.png`

### Données d'audit

- `C:\Sparrow\artifacts\ui-ux-live-audit\live-audit.json`
- `C:\Sparrow\artifacts\ui-ux-live-audit\rightbar-tabs-click-audit.json`
- `C:\Sparrow\artifacts\ui-ux-live-audit\rightbar-palette-audit.json`

## Corrections déjà faites

### WebView, zone de texte

Fichier: `C:\Sparrow\console.html`

- Rythme typographique plus calme dans la zone de conversation.
- Corps de texte autour de 15 px avec interligne autour de 1.62.
- Marges verticales réduites mais plus régulières.
- Largeur de lecture plus contrôlée en mode Focus.
- Markdown plus homogène: titres, paragraphes, listes, tableaux, inline code.
- Cartes outil/code/diff moins envahissantes.
- Lignes de reasoning et statuts `planner/coder` compactés.
- Textes de fin de type `coder completed` moins disproportionnés.

Réponse à la question précédente: oui, cette passe réduit aussi le problème de place prise par les blocs de reasoning et les lignes de statut finales. Ce n'est pas encore une vraie refonte des panneaux d'activité, mais le rendu devient nettement moins haut, moins criard et moins espacé.

## Bugs WebView critiques

### P0. Command palette vide malgré une API fonctionnelle

Symptôme:
- Le bouton palette ouvre bien la fenêtre.
- La palette affiche `No matches.`
- Taper `/` affiche encore `No matches.`
- L'endpoint `/commands` renvoie pourtant une liste de commandes.

Impact:
- Fonction centrale inutilisable.
- L'utilisateur pense que les commandes n'existent pas.

Cause probable:
- La palette filtre avant que le cache de commandes soit chargé, ou le cache n'est pas chargé au bon moment.

À faire:
- Charger les commandes avant ouverture, ou afficher un état `loading`.
- Refactorer `paletteOpen()` pour attendre `loadCommandsCache()`.
- Ajouter un test Playwright: ouverture palette, présence de `/help`, `/plan`, `/clear`.
- Ajouter un état erreur si `/commands` échoue.

### P0. Onboarding tour chevauche le composer et des contrôles

Symptôme:
- Sur desktop Focus, la bulle `YOUR STARTING POINT` couvre la zone basse.
- Sur mobile, elle couvre aussi le composer.
- En Cockpit, elle peut masquer des panneaux ou boutons.

Impact:
- Premier lancement confus.
- L'utilisateur peut croire que la zone de saisie ou les boutons sont cassés.

À faire:
- Recalculer l'ancrage de la tour selon viewport.
- Interdire à la tour de couvrir le composer.
- Ajouter collision detection avec topbar, rightbar, drawer et composer.
- Ajouter une option visible `skip` ou `dismiss`.
- Persister la fermeture.
- Ajouter snapshots desktop, laptop, tablet, mobile.

### P0. Cockpit mobile non utilisable

Symptôme:
- En viewport mobile, rail, drawer, topbar, contenu et composer se superposent.
- Les boutons de topbar débordent horizontalement.
- Le drawer garde une logique desktop.

Impact:
- Cockpit inutilisable sur petit écran.

À faire:
- Transformer le drawer Cockpit en sheet/modal mobile.
- Masquer ou condenser le rail sur mobile.
- Faire passer les actions secondaires dans un menu.
- Garantir un composer toujours visible et non recouvert.
- Ajouter breakpoint dédié inférieur à 768 px.
- Ajouter tests visuels `375x812`, `768x1024`, `1366x768`, `1600x1000`.

### P0. Historique des runs trop brut et potentiellement sensible

Symptôme:
- L'endpoint `/runs` renvoie des textes de tâches historiques en clair.
- Le panneau Autonomous tasks peut exposer des prompts longs ou sensibles.

Impact:
- Risque de fuite de secrets ou de contenu privé dans l'UI.
- Risque de panneau très lourd et illisible.

À faire:
- Redacter les secrets côté serveur avant renvoi.
- Tronquer les prompts longs côté API et côté UI.
- Afficher seulement un résumé court par défaut.
- Ajouter un bouton explicite pour révéler le détail.
- Ajouter tests de redaction sur tokens, clés, URLs privées et variables sensibles.

## Bugs WebView importants

### P1. Bouton ou entrée clavier d'aide non branché

Symptôme:
- Le code contient une logique de keyboard help.
- L'audit n'a pas trouvé de bouton visible `#kbdBtn`.
- Les raccourcis existent dans la documentation, mais l'accès in-app n'est pas clair.

Impact:
- Fonctionnalité présente mais difficile ou impossible à découvrir.

À faire:
- Ajouter un bouton clavier visible dans la topbar ou la zone composer.
- Ou supprimer le code mort si la fonctionnalité n'est plus voulue.
- Ajouter `aria-label`, tooltip et test click.

### P1. Boutons topbar trop petits

Symptôme:
- Plusieurs boutons mesurent entre 19 px et 23 px de haut.
- Exemples observés: Focus, Cockpit, replay, sound, verbose, font, theme, rightbar, config.

Impact:
- Mauvaise accessibilité.
- Clic difficile sur laptop tactile et mobile.
- Perception d'interface bricolée.

À faire:
- Passer les contrôles interactifs à 32 px minimum sur desktop.
- Passer à 40 px minimum sur tactile/mobile.
- Grouper les actions secondaires dans un menu si la place manque.
- Ajouter un audit automatique des tailles minimales.

### P1. Panneau Autonomous tasks peut rester en loading

Symptôme:
- L'onglet existe et l'endpoint `/runs` répond.
- L'audit de clic voit parfois `loading runs...` après ouverture.

Impact:
- L'utilisateur ne sait pas si c'est lent, vide ou cassé.

À faire:
- Ajouter état `loading`, `empty`, `error`, `loaded`.
- Ajouter timeout UI.
- Ajouter bouton refresh.
- Ajouter test qui attend réellement le rendu ou l'erreur.

### P1. Boutons pin/close rightbar trop petits et ambigus

Symptôme:
- `rbPin` et `rbClose` sont autour de 20 px.
- Après fermeture, le pin n'est plus actionnable, ce qui est logique, mais doit être rendu inert/accessibilité proprement.

Impact:
- Panneau difficile à contrôler.
- Navigation clavier potentiellement confuse.

À faire:
- Agrandir les boutons.
- Ajouter `aria-expanded`, `aria-controls`, `aria-hidden` selon état.
- Rendre le panneau inert quand il est fermé.
- Tester ouverture, pin, fermeture, réouverture.

### P1. Panneau Files trop bruyant

Symptôme:
- Le panneau liste les artefacts générés, captures, logs et fichiers attachés ensemble.
- Les fichiers d'audit encombrent rapidement l'affichage.

Impact:
- Difficile de trouver le fichier utile.

À faire:
- Grouper par source: attachments, artifacts, screenshots, logs, generated.
- Ajouter recherche/filtre.
- Ajouter tri par date/type.
- Masquer les logs techniques par défaut.

### P1. Rightbar: libellés et compteurs peu lisibles

Symptôme:
- Les items du menu peuvent apparaître comme `Preview0`, `Timeline0`, sans séparation visuelle claire.

Impact:
- Impression de finition faible.
- Lecture moins rapide.

À faire:
- Utiliser badges séparés.
- Masquer les compteurs à zéro.
- Ajouter largeur stable pour éviter les sauts.

### P1. Roadmap et Watched releases peuvent afficher trop de texte brut

Symptôme:
- Certaines entrées de roadmap/releases sont textuellement longues.
- Risque de panneaux très hauts.

Impact:
- Le rightbar devient un mur de texte.

À faire:
- Limiter à 2 ou 3 lignes par item.
- Ajouter expansion par item.
- Séparer titre, état, date, source.
- Eviter les blocs Markdown massifs dans une ligne de liste.

### P1. Focus actions strip trop présent

Symptôme:
- La bande noire d'actions Focus attire beaucoup l'oeil.
- Sur desktop et surtout mobile, elle occupe l'espace du bas près du composer.

Impact:
- L'attention quitte la conversation.
- Le composer semble serré.

À faire:
- Rendre cette bande contextuelle.
- La masquer quand aucune action n'est disponible.
- Ou la transformer en barre compacte intégrée au composer.

## Bugs WebView moyens

### P2. Topbar mobile déborde

Symptôme:
- Sur mobile, les boutons de topbar se chevauchent ou sortent du viewport.

À faire:
- Introduire menu overflow.
- Réduire le libellé de mode.
- Garder seulement les deux actions principales visibles.

### P2. Config button incohérent

Symptôme:
- Le bouton config est plus petit que les autres.
- Sa hauteur observée est autour de 19 px.

À faire:
- L'aligner sur le système de boutons standard.
- Ajouter un état hover/focus cohérent.

### P2. Etats vides trop faibles

Symptôme:
- Plusieurs panneaux affichent un état vide minimal.
- Exemple: Preview, Timeline, Costs, Diff, Terminal.

À faire:
- Ajouter un titre court, une phrase utile, une action éventuelle.
- Eviter les zones complètement blanches qui ressemblent à des bugs.

### P2. Hiérarchie des icônes du rail à clarifier

Symptôme:
- Le rail utilise surtout des symboles courts.
- La compréhension dépend des tooltips et de la mémoire utilisateur.

À faire:
- Vérifier tous les tooltips.
- Ajouter état actif plus lisible.
- Ajouter `aria-label` sur chaque bouton.

## Bugs TUI critiques/importants

### P1. Raccourcis affichés faux ou non implémentés

Symptôme:
- La ligne d'aide TUI affiche `Esc:quit  Tab:agents  /:search  @:skills  Ctrl+R:run  Ctrl+C:stop  F1:help`.
- Le code observé ne montre pas de gestion normale de `F1`.
- `Ctrl+R` est incohérent: ailleurs il est présenté comme rewind/checkpoint, ici comme run.
- `Ctrl+C` quitte la TUI, mais le hint dit `stop`.
- `@` est présenté comme skills, alors que le TUI travaille surtout avec des agents.

Impact:
- L'utilisateur apprend de mauvais raccourcis.
- Perte de confiance dans la TUI.

À faire:
- Aligner `render_keyboard_hints()` avec les handlers réels.
- Implémenter ou retirer `F1`.
- Clarifier `Ctrl+C`: stop ou quit, pas les deux sans état.
- Clarifier `Ctrl+R`: run, replay ou rewind.
- Remplacer `@:skills` par le comportement réel.

### P1. `Ctrl+L` nettoie seulement les lignes, pas l'état de groupes

Symptôme:
- `/clear` réinitialise les lignes, groupes, groupe courant et focus.
- `Ctrl+L` vide seulement `self.lines`.

Impact:
- Etat interne possiblement incohérent après nettoyage.
- Des groupes ou focus peuvent rester actifs sans lignes visibles.

À faire:
- Faire appeler la même routine par `/clear` et `Ctrl+L`.
- Ajouter test TUI: groupes actifs, `Ctrl+L`, état propre.

### P1. Inline code Markdown non stylé correctement

Symptôme:
- Le renderer Markdown traite `Event::Code` comme du texte normal dans certains chemins.

Impact:
- Les commandes, fichiers et variables ressortent moins clairement.
- Incohérence avec le WebView.

À faire:
- Appliquer `styles.code_inline` pour `Event::Code`.
- Ajouter snapshot TUI markdown avec inline code.

### P1. Liens Markdown possiblement rendus sans URL

Symptôme:
- Le renderer garde une variable `current_link_url`.
- Le début de lien ne semble pas affecter `dest_url` à cette variable.

Impact:
- Les liens peuvent perdre leur destination dans le rendu texte.

À faire:
- Stocker `dest_url` au début du lien.
- Afficher `label (url)` ou une convention claire.
- Ajouter test Markdown avec lien.

### P1. Layout TUI trop rigide sur petits terminaux

Symptôme:
- Le layout réserve des hauteurs fixes pour cockpit, swarm, diff, checkpoints, hints et input.
- Les tests prouvent l'absence d'overflow ligne par ligne, mais pas le confort d'usage quand tous les panneaux sont actifs.

Impact:
- Sur 40x12 ou 60x20, la zone de logs peut devenir trop petite.

À faire:
- Prioriser les panneaux selon hauteur disponible.
- Collapser automatiquement swarm/diff/checkpoints sous un seuil.
- Ajouter test de stress avec swarm + diff + checkpoints + input multiligne.

## Bugs TUI moyens

### P2. Documentation clavier incohérente

Symptôme:
- `docs/keyboard.md` indique certains raccourcis qui ne correspondent pas exactement au comportement TUI.
- `q` est décrit comme quit dans certains contextes, mais le code le réserve surtout au replay.

À faire:
- Séparer clairement WebView, TUI normal, TUI replay.
- Générer la doc depuis une table de raccourcis partagée si possible.

### P2. Aide `/help` mélange commandes locales et commandes moteur

Symptôme:
- Le TUI liste des commandes comme si elles étaient toutes locales.
- Certaines sont probablement envoyées au moteur.

Impact:
- L'utilisateur ne sait pas ce qui est instantané, local, distant, ou dépendant du backend.

À faire:
- Séparer `Local commands` et `Engine commands`.
- Ajouter une phrase courte par commande.

### P2. Autocomplete ignore le contexte multiligne

Symptôme:
- L'autocomplete travaille sur `input_lines[0]`.
- En saisie multiligne, la ligne courante du curseur n'est pas forcément prise en compte.

Impact:
- Suggestions incorrectes quand l'utilisateur écrit une tâche longue.

À faire:
- Utiliser la ligne courante.
- Ajouter tests pour curseur sur ligne 2 ou 3.

### P2. Table Markdown visuellement incohérente

Symptôme:
- Le formatter utilise des bordures qui ressemblent à des séparateurs de milieu pour haut/bas.

Impact:
- Les tableaux ont une finition visuelle moins propre.

À faire:
- Utiliser `┌ ┬ ┐`, `├ ┼ ┤`, `└ ┴ ┘`.
- Ajouter snapshot de table.

### P2. Barre de raccourcis TUI tronquée sur petite largeur

Symptôme:
- La ligne d'aide est longue.
- Sur narrow terminal, certains raccourcis disparaissent.

À faire:
- Adapter les hints selon largeur.
- Afficher seulement les 3 ou 4 raccourcis essentiels.
- Déplacer le reste vers `F1` ou `/help`.

## Liste de correction recommandée

### Phase 1, stabilisation visible

1. Corriger la palette vide.
2. Corriger l'onboarding tour qui chevauche composer et panneaux.
3. Rendre Cockpit utilisable sur mobile.
4. Redacter et tronquer les données du panneau runs/autonomous tasks.
5. Aligner les raccourcis TUI affichés avec le code réel.
6. Unifier `/clear` et `Ctrl+L`.

### Phase 2, lisibilité et accessibilité

1. Agrandir tous les boutons topbar/rightbar sous-dimensionnés.
2. Ajouter ou supprimer proprement le bouton keyboard help.
3. Améliorer les états loading/empty/error des panneaux rightbar.
4. Réorganiser Files avec filtres et groupes.
5. Compacter Roadmap, Releases et Autonomous tasks.
6. Ajouter `aria-label`, `aria-expanded`, `aria-hidden`, `inert` selon état.

### Phase 3, cohérence de rendu

1. Finaliser la typo WebView sur conversations réelles longues.
2. Harmoniser Markdown WebView/TUI: inline code, liens, tables, listes.
3. Corriger les bordures de tables TUI.
4. Adapter les hints TUI aux petites largeurs.
5. Clarifier la doc clavier par surface: WebView, TUI normal, TUI replay.

### Phase 4, garde-fous automatisés

1. Remplacer ou réparer `scripts/audit-webview.mjs`.
2. Ajouter snapshots Playwright pour Focus et Cockpit à 4 viewports.
3. Ajouter test palette avec `/commands`.
4. Ajouter test rightbar pour chaque onglet.
5. Ajouter test de taille minimale des boutons interactifs.
6. Ajouter tests TUI de raccourcis, clear state, markdown links/code, stress layout.

## Priorité finale

Ordre conseillé:

1. Palette vide.
2. Mobile Cockpit.
3. Onboarding overlay.
4. Redaction/troncature des runs.
5. Raccourcis TUI faux.
6. Boutons trop petits.
7. Rightbar loading/empty/error.
8. Markdown TUI inline code/liens.
9. Files/Roadmap/Releases trop bruyants.
10. Tests visuels automatisés.

