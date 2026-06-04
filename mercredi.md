# Session Mercredi 3 Juin 2026 — Sparrow Development Log

---

## 1. Tâches demandées

### Phase 1 — Provider + Routing
- [x] Ajouter `opencode-go` au registry des providers (URL: `https://opencode.ai/zen/go/v1`)
- [x] Option `auto_discover` pour scanner les modèles d'un provider après ajout de clé
- [x] Option `preferred_provider` pour choisir le provider du routing intelligent
- [x] Pouvoir scanner les modèles disponibles d'un provider depuis l'UI
- [x] Pouvoir sélectionner un provider par défaut depuis l'UI

### Phase 2 — OAuth
- [x] Intégrer l'OAuth device flow pour les providers qui le supportent
- [x] Rendre le système compatible registry-driven

### Phase 3 — Animations & Verbose
- [x] Appliquer le caret à tous les agents (planner, coder, verifier)
- [x] Ajouter des verbes de vol marrants pendant la réflexion (style Claude Code)
- [x] Afficher les appels d'outils avec icônes par type, dépliables
- [x] Compteurs de tokens ↑↓ (vert/rouge) par appel d'outil en temps réel
- [x] Moineau ASCII au démarrage
- [x] ResizeObserver pour redimensionnement dynamique
- [x] Mode jour/nuit préservé

### Phase 4 — Debug & Correctifs
- [x] Débugger pourquoi Sparrow ne se lance pas normalement
- [x] Ajouter `--port` à la commande `console`
- [x] Ajouter confirmation `[y/N]` sur `sparrow rewind`
- [x] Cache de classification LLM pour éviter les appels redondants
- [x] Fixer la race condition `loadRouting()` / `loadConfig()` (dropdown vide)
- [x] Scan models : injecter les résultats dans la liste de modèles
- [x] Auto-collapse de la carte RunFinished (se replie après 2.5s)
- [x] Code block renderer pour les sorties de code (collapsible)
- [x] Fixer l'affichage du texte coupé mot par mot
- [x] Fixer le texte corrompu/garbage de DeepSeek (XML/ANSI)
- [x] Fixer le `model_override` ignoré (fallback chain persistait)
- [x] Fixer le `recv 0 tok` et l'absence de réponse après tool call
- [x] Corriger le formatage des nombres (espace fine → virgule)
- [x] Augmenter la capacité du broadcast channel (256 → 1024)

---

## 2. Tout ce qui a été implémenté

### Backend (Rust)

| Fichier | Changement |
|---------|------------|
| `src/config/providers.rs` | Provider `opencode-go` (4 modèles seed), `AuthFlow` enum (ApiKey/DeviceOAuth), 34 injects `auth_flow: AuthFlow::default()`, `list_oauth_providers()` |
| `src/config/mod.rs` | `Routing.auto_discover: bool`, `Routing.preferred_provider: Option<String>` |
| `src/router/mod.rs` | `BasicRouter.preferred_provider`, `resolve_provider()` override |
| `src/cli/mod.rs` | `Commands::Console { port }`, `Commands::Route` + `RouteAction` enum, `Commands::Rewind { id }` |
| `src/main.rs` | `handle_webview(port)`, rewind confirmation `[y/N]`, `--web` 9339 port, `drop(active.take())` warning fix, `use std::io::Write`, `handle_auth_login` registry-driven |
| `src/extras.rs` | `OAuthFlow.start_device_flow` paramétrique (endpoints/scope) |
| `src/engine/mod.rs` | `role: "main"` → `"coder"`, `AgentStatus::Working/Done`, `TokenUsage` fallback, `classify_cache` (Mutex<HashMap>), planner/verifier notes verbeuses, broadcast channel 1024 |
| `src/capabilities/mod.rs` | Curator : retrait `.rs/.py/.js/.ts` markers, uppercase >12, +13 patterns `has_concrete_output` |
| `src/console.rs` | `POST /providers/scan`, `GET /routing` + `POST /routing`, `use secrecy::ExposeSecret`, strip provider prefix du `model_override` |
| `src/provider/openai_compat.rs` | Filtre `reasoning_content` : skip content si non vide, allow si null/empty |

### Frontend (console.html)

| Section | Changement |
|---------|------------|
| CSS | `.sp` spinner, `.tool-ico`, `.inline-tok` (↑↓), `.bird`, `.verb-think`, `.code-block`, `.run-summary` summary flex, `@keyframes sp-dot/tk-pulse/fold-in/verb-in` |
| JS Constants | `BIRD` ASCII, `TOOL_ICONS` (30+ mapping), `FLIGHT_VERBS` (+4 verbes) |
| JS Functions | `toolIcon()`, `finalizeStreamBlock()`, `scanProviderModels()`, `loadRouting()`, `saveRouting()` |
| Stream | `streamDelta` filter XML/ANSI → `⏳`, `STREAM_ENDERS` whitelist (ne plus casser sur AgentStatus) |
| AgentStatus | Caret pour tous les rôles, verbes de vol, spinner `◌`, `verbedNote` |
| Tool Cards | Icônes par type, baseline token → delta ↑↓ dans `closeToolCard` |
| RunStarted | BIRD ASCII + tagline SPARROW |
| RunFinished | `<details>` avec auto-collapse 2.5s, `setRunActive(false)` |
| Config | Scan button par provider, routing panel (dropdown + checkbox + save), `loadRouting()` via `j.all_providers` |
| Model Scan | Injection DOM directe avec metadata (`_modelsRegistry`), bouton "set default" |
| ResizeObserver | Font-size 11-14px dynamique |
| Locale | `toLocaleString('en-US')` partout (virgule au lieu d'espace fine) |

---

## 3. Ce qui n'est PAS opérationnel

### 🔴 Problème : réponse vide après tool call
**Symptôme** : DeepSeek appelle un outil (`fs_list {}`), le tool s'exécute, puis plus aucune réponse texte. Le run se termine avec `0↓ tok`.

**Cause suspectée** : Le filtre `reasoning_content` bloque le `content` quand DeepSeek inclut un champ `reasoning_content: null` dans la réponse post-outil. Le fix appliqué (check `!s.is_empty()`) devrait résoudre — **à re-tester**.

### 🟡 Problème : tool calls vides ou incorrects
**Symptôme** : `fs_list` appelé avec `{}` (arguments vides) — le modèle ne sait pas quel dossier lister.

**Cause** : Comportement du modèle deepseek-v4-pro, pas un bug Sparrow. Le modèle décide d'appeler un outil sans arguments valides.

### 🟡 Problème : texte corrompu résiduel
**Symptôme** : Texte avec caractères manquants ("B. Chang de modèle...")

**Cause** : Le broadcast channel (même à 1024) peut dropper des `ThinkingDelta` si le WebSocket est lent. Solution partielle appliquée.

### 🟢 Non testé
- OAuth device flow (copilot, qwen, google, microsoft)
- `sparrow route set/show/clear` depuis la CLI
- Mode jour/nuit survit au refresh
- Scan sur provider sans clé API (doit afficher une erreur claire)

---

## 4. Commandes utiles

```powershell
# Build + lancement
cd C:\sparrow
Get-Process -Name sparrow -ErrorAction SilentlyContinue | Stop-Process -Force
cargo build
.\target\debug\sparrow.exe console --port 9339

# Test endpoints
curl http://localhost:9339/config
curl http://localhost:9339/routing
curl -X POST http://localhost:9339/providers/scan -H "Content-Type: application/json" -d '{"provider":"nvidia"}'
curl -X POST http://localhost:9339/routing -H "Content-Type: application/json" -d '{"preferred_provider":"nvidia","auto_discover":true}'

# CLI
sparrow route set nvidia
sparrow route show
sparrow rewind <id>  # demande [y/N] maintenant
```

---

*Fichier généré le 3 juin 2026 — session deepseek-v4-pro via OpenCode Go*
