# Keyboard shortcuts

## TUI cockpit (`sparrow tui`)

| Shortcut | Action |
|---|---|
| `Enter` | Submit current input |
| `Shift+Enter` | Insert newline in the input box |
| `Up` / `Down` | Walk through command history (oldest ↔ freshest) |
| `Tab` | Slash-command autocomplete (first match) |
| `Ctrl+I` | Inject text into the running run (no new turn) |
| `Ctrl+L` | Clear the cockpit pane |
| `Ctrl+O` | Fold / unfold tool activity for the focused task |
| `Ctrl+↑` / `Ctrl+↓` | Move focus between tasks/swarm lanes |
| `/collapse` | Fold all tasks |
| `/expand` | Unfold all tasks |
| `q` or `Esc` (in fallback view) | Quit the TUI |

## WebView console (`sparrow console`)

| Shortcut | Action |
|---|---|
| `Enter` | Submit input |
| `Shift+Enter` | Insert a newline in the composer |
| `Up` / `Down` | Walk through command history |
| `Cmd/Ctrl+K` | Open the slash command palette |
| `Shift+Enter` in palette | Insert selected command and submit |
| `@` after whitespace | Open the inline agent picker |
| `Esc` | Close picker, palette, diff panel, model dropdown, config, approval modal, or restore history draft |
| `D` (outside input) | Toggle the diff side panel |
| `P` (outside input) | Open the model picker dropdown |
| `Cmd/Ctrl+Shift+L` | Toggle Captain / Paper WebView theme |
| `Cmd/Ctrl+M` | Mute / unmute WebView chirps |
| Drag files onto page | Attach files to the current prompt |
| `/help` | List built-in and project slash commands |
| `/plan <task>` | Render a read-only plan |
| `/permissions` | Open the permissions panel |
| `/memory` | Inspect bounded `MEMORY.md` / `USER.md` |
| `/sessions` | List persisted sessions |
| `/tools` | Inspect toolset metadata |
| `/security` | Run a security audit and render findings |
| `/crew` | Open the persistent agents (Crew) panel |

## WebView panels (rail icons)

| Icon | Panel | Content |
|---|---|---|
| `◈` | Crew | Persistent `.agent.md` agents — name, role, live status |
| `◷` | Sessions | Recent session list with turn count and timestamp |
| `✦` | Memory | Bounded `MEMORY.md` / `USER.md` facts |
| `⌗` | Plugins | Installed plugin manifests |
| `⚒` | Tools | Tool metadata — toolset, risk, mutation, network flags |
| `◉` | Permissions | Current permission mode and allow/deny lists |
| `⛨` | Security | Live security audit score and findings |
| `◇` | Artifacts | Uploaded files and generated artifacts |

## Themes

The TUI cockpit picks its theme from the `SPARROW_THEME` environment variable.
Built-in names (case-insensitive):

| Name | Description |
|---|---|
| `captain` | Default warm amber on near-black |
| `midnight` | Cool blues for late-night work |
| `paper` | Cream paper background, dark fg, for bright environments |

Unknown names fall back to `captain`. Themes are listed by
`sparrow::tui::theme::THEME_NAMES`.
