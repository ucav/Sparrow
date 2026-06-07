# Sparrow for VS Code

Open the Sparrow cockpit inside VS Code. Run, plan, and rewind tasks without
leaving the editor.

## Install (dev)

```bash
cd ide/vscode
npm install
npx vsce package
code --install-extension sparrow-vscode-*.vsix
```

## Commands

- **Sparrow: Open Cockpit** — opens the WebView cockpit in a side panel
- **Sparrow: Run Task** — prompts for a task and runs `sparrow run`
- **Sparrow: Plan Task** — read-only plan via `sparrow plan`
- **Sparrow: Rewind Last Checkpoint** — `sparrow rewind --last`

## Settings

- `sparrow.binaryPath` — path to the `sparrow` binary (default `sparrow`)
- `sparrow.cockpitPort` — cockpit HTTP port (default `9339`)
- `sparrow.autoLaunchCockpit` — start cockpit on VS Code startup (default `false`)

## How it works

The extension spawns `sparrow console --port <port>` if no cockpit is already
running, then embeds it via a WebView pointing at `http://127.0.0.1:<port>/`.
There is no remote endpoint, no telemetry, no API key collection in the
extension itself — everything lives in your local Sparrow binary.
