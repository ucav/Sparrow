# Plan d'implémentation — Écarts STATUS2.md

Généré le 3 juin 2026. Lecture seule — ne pas appliquer sans validation.

---

## Résumé des écarts

| # | Item | Priorité | Effort | Impact |
|---|------|----------|--------|--------|
| 1 | Auto-launch navigateur | 🔴 HIGH | 30 min | UX critique |
| 2 | Budget bar avec transition | 🟡 MEDIUM | 45 min | Visuel cockpit |
| 3 | Submit feedback animation | 🟢 LOW | 20 min | Polish |
| 4 | Ember tuning paper | 🟢 LOW | 15 min | Polish |
| 5 | Grain noise paper | 🟢 LOW | 5 min | Polish |
| 6 | Replay bouton branché recorder | 🟡 MEDIUM | 1h | Fonctionnel |
| 7 | Reduced-motion exhaustif | 🟡 MEDIUM | 30 min | Accessibilité |
| 8 | Boot animation char-by-char | 🟢 LOW | 20 min | Polish |

---

## 1. Auto-launch navigateur (Q1, décision Abdou)

**Fichier** : `src/main.rs` (handle_webview) + `src/cli/mod.rs`

**Plan** :
- Ajouter `#[arg(long, default_value = "true")] launch_browser: bool` à `Commands::Console`
- Dans `handle_webview`, après le `println!("WebView console: http://{}", addr)`, si `launch_browser` est true :
  - Windows : `std::process::Command::new("cmd").args(["/c", "start", &url]).spawn()`
  - Linux : `std::process::Command::new("xdg-open").arg(&url).spawn()`
  - macOS : `std::process::Command::new("open").arg(&url).spawn()`
- Ajouter `sparrow console --no-launch` pour désactiver
- Ne pas bloquer le serveur — le spawn est `fire-and-forget`

**Risque** : zéro. Le navigateur s'ouvre en parallèle, le serveur continue.

---

## 2. Budget bar cockpit (Q4)

**Fichier** : `console.html` (HTML cockpit + CSS + JS)

**Plan** :
- Ajouter dans la row cockpit une nouvelle stat : `<span class="k">budget</span>` + `<div class="budget-track"><span class="budget-fill"></span></div>` + `<span class="budget-pct">0%</span>`
- CSS :
  ```css
  .budget-track{width:60px;height:5px;border:1px solid var(--line);border-radius:999px;background:#0d0a07;overflow:hidden}
  .budget-fill{display:block;height:100%;width:0;background:var(--add);transition:width .4s ease}
  .budget-fill.warn{background:var(--coral)}
  .budget-fill.danger{background:var(--rem)}
  ```
- JS : fonction `updateBudgetBar()` appelée dans `updateBudget()`.
  - Calcule `pct = sessionCost / dailyLimit * 100`
  - `.budget-fill` width = `pct%`
  - `< 60%` : vert (`var(--add)`)
  - `60-80%` : orange (`var(--coral)`) + classe `warn`
  - `> 80%` : rouge (`var(--rem)`) + classe `danger`
  - `> 100%` : refus serveur (déjà géré par le backend)

**Risque** : la daily limit est chargée depuis `/config` dans `hydrateHero()`. S'assurer que `_budgetDailyUsd` est bien setté avant le premier appel.

---

## 3. Submit feedback animation (S2-6)

**Fichier** : `console.html` (JS uniquement)

**Plan** :
- Dans `runTask()`, avant le `fetch('/run',...)` :
  ```javascript
  const runBtn = $('runBtn');
  runBtn.style.transform = 'scale(0.92)';
  setTimeout(() => runBtn.style.transform = '', 120);
  ```
- La classe `.live-tag` a déjà le pulse — vérifier qu'elle passe bien à `live` après soumission
- Ajouter transition CSS sur le bouton run : `transition: transform .1s ease`

**Risque** : zéro. 3 lignes de CSS + 3 lignes de JS.

---

## 4. Ember tuning paper (S3-7)

**Fichier** : `console.html` (CSS uniquement)

**Plan** :
- Ajouter dans le bloc `[data-theme="paper"]` :
  ```css
  [data-theme="paper"] .ember{opacity:.15;filter:blur(.6px)}
  [data-theme="paper"] .ember{animation-duration:12s} /* plus lent */
  ```
- Réduire le nombre d'embers sur paper : dans la boucle `for(i=0;i<14;i++)`, check `getTheme()` et réduire à 6-7 sur paper

**Risque** : zéro. CSS only.

---

## 5. Grain noise paper (S3-8)

**Fichier** : `console.html` (CSS uniquement)

**Plan** :
- Modifier le bloc `[data-theme="paper"]` pour override le `body::after` :
  ```css
  [data-theme="paper"] body::after{opacity:.025}
  ```
- Valeur actuelle : `opacity:.05` pour captain → `opacity:.025` pour paper

**Risque** : zéro. 1 ligne CSS.

---

## 6. Replay bouton branché recorder (P7)

**Fichier** : `console.html` (JS) + `src/console.rs` (si endpoint manquant)

**Plan** :
- Vérifier si `GET /events/replay?run_id=...` existe dans `console.rs`. Si non :
  - Ajouter une route qui appelle `recorder.load(run_id)` et renvoie le JSON
- Dans `console.html`, le handler du `replayBtn` :
  ```javascript
  $('replayBtn').addEventListener('click', async () => {
    const runId = prompt('Run ID to replay:');
    if(!runId)return;
    const r = await fetch('/events/replay?run_id='+runId);
    const events = await r.json();
    term.innerHTML = '';
    events.forEach(ev => handleEvent(ev));
  });
  ```
- Si le recorder n'a pas de endpoint REST, passer par le WebSocket avec un message spécial `__replay__:<run_id>`

**Risque** : moyen. Le recorder doit être accessible depuis le contexte WebView (il l'est — `Arc<FsRecorder>` est déjà dans `AppState`).

---

## 7. Reduced-motion exhaustif (S5-1)

**Fichier** : `console.html` (CSS uniquement)

**Plan** :
- Auditer chaque `@keyframes` dans le fichier et vérifier qu'il a un fallback dans `@media (prefers-reduced-motion: reduce)`
- Keyframes actuelles à vérifier :
  - `drift` (embers) — ✅ déjà couvert
  - `rise` — ❌ pas de fallback → ajouter `animation:none;opacity:1;transform:none`
  - `shimmer` — ❌ pas de fallback → `animation:none`
  - `wordin` — ❌ → `animation:none;opacity:1;letter-spacing:11px;filter:none`
  - `logo-blink` — ❌ → `animation:none`
  - `logo-tap` — ❌ → `animation:none;transform:none`
  - `route-step` — ❌ → `animation:none`
  - `flash-cost` — ❌ → `animation:none`
  - `flash-tok` — ❌ → `animation:none`
  - `pulse-providers` — ❌ → `animation:none`
  - `lanein` — ❌ → `animation:none;opacity:1;transform:none`
  - `in` — ❌ → `animation:none;opacity:1`
  - `bl` (caret) — ❌ → `animation:none;opacity:1` (toujours visible)
  - `sp-dot` — ❌ → `animation:none` (spinner statique)
  - `tk-pulse` — ❌ → `animation:none`
  - `verb-in` — ❌ → `animation:none;opacity:1`
  - `fold-in` — ❌ → `animation:none;opacity:1`
  - `learnpop` — ❌ → `animation:none`
  - `drawer-in` — ❌ → `animation:none;opacity:1;transform:none`
  - `fade-in` — ❌ → `animation:none;opacity:1`
  - `logo-float` — ❌ → `animation:none`
  - `fin` — ❌ → `animation:none;opacity:1`
- Ajouter dans le bloc `@media (prefers-reduced-motion: reduce)` :
  ```css
  *,*::before,*::after{animation-duration:.001ms!important;animation-iteration-count:1!important;transition-duration:.001ms!important}
  .ember{display:none!important}
  .streaming::after{animation:none!important;opacity:1}
  .caret,.cur,.cur2{animation:none!important;opacity:1}
  .sp{animation:none!important}
  ```

**Risque** : zéro. CSS only, ~30 lignes.

---

## 8. Boot animation char-by-char (S1-6)

**Fichier** : `console.html` (JS uniquement)

**Plan** :
- Actuellement : `runBootAnimation()` affiche les lignes statiquement avec `fade-in`
- Ajouter le `typeCmd` sur la tagline "one cli · grows with you" :
  ```javascript
  async function runBootAnimation(){
    // ... existant ...
    // Après le logo SVG, taper la tagline caractère par caractère
    const tagEl = document.querySelector('.b-tag');
    const tagText = tagEl.textContent;
    tagEl.textContent = '';
    for(const ch of tagText){
      tagEl.textContent += ch;
      await sleep(40 + Math.random()*30);
    }
    // Puis lignes de boot
    BOOT_STATUS_LINES.forEach(...)
  }
  ```
- La fonction `typeCmd` existe déjà (ligne 1183) — la réutiliser ou adapter
- Budget : ~2.5s total (conforme au document)

**Risque** : zéro. JS only, fonction `sleep` déjà existante.

---

## Ordre d'exécution recommandé

1. **Auto-launch** (5 min, impact UX immédiat)
2. **Budget bar** (15 min, cockpit row)
3. **Submit feedback** (5 min, polish)
4. **Reduced-motion** (10 min, accessibilité)
5. **Replay bouton** (20 min, fonctionnel)
6. **Paper tuning** (embers + grain, 5 min)
7. **Boot char-by-char** (5 min)

**Total estimé** : ~1h15 pour les 8 items.

---

*Plan généré le 3 juin 2026 — en attente de validation Abdou*
