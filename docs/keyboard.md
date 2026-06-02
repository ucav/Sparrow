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
| `/help` | List built-in and project slash commands |
| `/plan <task>` | Render a read-only plan |
| `/permissions` | Open the permissions panel |
| `/memory` | Inspect bounded `MEMORY.md` / `USER.md` |
| `/sessions` | List persisted sessions |
| `/tools` | Inspect toolset metadata |
| `/security` | Run a security audit and render findings |

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
