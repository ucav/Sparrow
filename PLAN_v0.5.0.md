# Plan: Sparrow → surpasser Hermes (v0.5.0+)

## Vision
Sparrow doit être PLUS complet, PLUS rapide, et PLUS agréable que Hermes sur tous les axes : outils, skills, CLI rendering, streaming, gateway, memory, sécurité.

## Gap Analysis Sparrow vs Hermes (v0.4.0)

| Dimension       | Sparrow v0.4.0 | Hermes | Gap | Priorité |
|-----------------|----------------|--------|-----|----------|
| Outils          | ~15            | 30+    | +15 | P0 |
| Skills          | 21             | 200+   | +180 | P1 |
| CLI rendering   | Texte brut     | TUI + MDV2 | Total | P0 |
| Streaming       | Non            | Oui    | Total | P0 |
| Memory visible  | Non            | Oui    | Total | P1 |
| Gateway E2E     | Non testé      | Oui    | Total | P1 |
| Voice TTS/STT   | Rien           | Oui    | Total | P2 |
| Cron scheduler  | Code présent   | Actif  | Partiel | P2 |
| Session search  | Basique        | FTS5   | Total | P1 |
| Dashboard       | WebView        | Web UI | OK | P2 |

## Plan d'attaque — 5 phases

### Phase 1: CLI Rich Rendering Engine (FONDATION)
- Terminal renderer: Ratatui + syntect pour syntax highlighting
- Output formatters: Markdown→Terminal, JSON→Pretty, Code→Highlighted, Diff→Colored
- Progress/Spinner system: streaming live avec étapes
- Tool output renderers: un renderer par type de tool (file, terminal, search, etc.)
- § src/tui/renderer.rs, src/tui/formatters/, src/tui/tool_renderers/

### Phase 2: Streaming + Chat Interactif
- Infrastructure streaming: channels tokio pour events live
- `sparrow chat` v2: session persistante, historique, reprise, streaming
- Progress bars, ETA, lane display pour multi-agent
- § src/streaming/, src/chat/

### Phase 3: Tools + Skills expansion
- Browser/vision tools: Playwright déjà là → wrapper CLI-friendly
- TTS/STT tools: via API (OpenAI TTS, Whisper)
- File search: rg/grep wrapper avec output formaté
- 50+ skills communautaires portés depuis Hermes
- § src/tools/new/, skills/

### Phase 4: Gateway + Memory + Sessions
- Gateway E2E: Telegram/Discord avec healthcheck et monitoring
- Memory visible: `sparrow memory show/edit/search`
- Session search: FTS5 + cross-profile + UI terminal
- § src/memory/, src/gateway/

### Phase 5: Polish + Performance
- Cron scheduler: activation + commandes de gestion
- Voice TTS/STT: intégration native
- Optimisation startup <500ms
- Documentation exhaustive

## Architecture cible

```
src/
├── main.rs              # CLI entry (à splitter)
├── lib.rs               # Module declarations
├── streaming/           # NOUVEAU — infrastructure streaming live
│   ├── mod.rs
│   ├── channel.rs       # Tokio channels pour events
│   ├── progress.rs      # Spinners, bars, ETA
│   └── lane.rs          # Multi-agent lane display
├── chat/                # NOUVEAU — chat interactif v2
│   ├── mod.rs
│   ├── session.rs       # Session persistante
│   ├── history.rs       # Historique + reprise
│   └── composer.rs      # Input composer avec autocomplete
├── tui/                 # EXISTANT — enrichi
│   ├── renderer.rs      # NOUVEAU — moteur de rendu terminal
│   ├── formatters/      # NOUVEAU — formatters par type
│   │   ├── mod.rs
│   │   ├── markdown.rs  # Markdown → terminal
│   │   ├── code.rs      # Code + syntax highlighting
│   │   ├── diff.rs      # Diff coloré
│   │   ├── json.rs      # JSON pretty print
│   │   └── table.rs     # Tableaux formatés
│   └── tool_renderers/  # NOUVEAU — renderers par outil
│       ├── mod.rs
│       ├── file.rs
│       ├── terminal.rs
│       ├── search.rs
│       ├── browser.rs
│       └── web.rs
├── tools/               # EXISTANT — étendu
│   ├── tts.rs           # NOUVEAU — Text-to-Speech
│   ├── stt.rs           # NOUVEAU — Speech-to-Text
│   ├── vision.rs        # EXISTANT
│   └── file_search.rs   # NOUVEAU — ripgrep wrapper
├── memory/              # EXISTANT — enrichi
│   ├── cli.rs           # NOUVEAU — CLI memory commands
│   └── fts.rs           # NOUVEAU — FTS5 session search
├── skills/              # Racine — 200+ skills
│   ├── [50+ new].md
└── gateway/             # EXISTANT — testé E2E
    ├── telegram.rs      # Healthcheck + E2E
    └── discord.rs       # Healthcheck + E2E
```

## Implémentation immédiate — Phase 1

On commence par le CLI Rich Rendering Engine parce que TOUT le reste en dépend.
Une fois que le terminal peut afficher du markdown, du code coloré, des diffs,
des spinners, et du streaming, tout le reste devient naturel.
