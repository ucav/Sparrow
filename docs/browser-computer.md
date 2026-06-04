# Browser And Computer-Use Tools

Sparrow exposes two Playwright-backed UI automation tools:

- `browser`: headless browser navigation, screenshots, text extraction, click,
  type, and JavaScript evaluation.
- `computer`: focused computer-use primitive for `screenshot`, `click`, `type`,
  and `press`. It is classified as `Exec` so the autonomy gate treats
  UI-driving actions more strictly than passive web reads.

Both tools use the same embedded Playwright driver. The driver is compiled into
the Sparrow binary and written to a temporary file at runtime, so installed
binaries do not need a repository checkout.

## Runtime Setup

Install Node.js, then from the Sparrow checkout:

```bash
npm install
npm run browser:install
```

The tool returns an honest error if Node.js, the `playwright` package, or the
Chromium browser is missing.

## Tool Schemas

`browser` accepts:

```json
{
  "action": "navigate | screenshot | get_text | extract | click | type | evaluate",
  "url": "https://example.com",
  "selector": "main",
  "x": 120,
  "y": 240,
  "text": "optional text for type",
  "key": "Enter",
  "js": "document.title",
  "timeout_ms": 30000,
  "session_id": "optional-persistent-browser-session",
  "full_page": true,
  "viewport": { "width": 1365, "height": 768, "deviceScaleFactor": 1 }
}
```

`computer` accepts:

```json
{
  "action": "screenshot | click | type | press",
  "url": "https://example.com",
  "selector": "input[name=q]",
  "x": 120,
  "y": 240,
  "text": "search query",
  "key": "Enter",
  "session_id": "sparrow-local",
  "timeout_ms": 30000
}
```

`selector` is preferred when the target has a stable CSS selector. Coordinate
actions use viewport coordinates (`x`, `y`) and are useful after a screenshot
when the model needs to click or type into a visible area. `session_id` keeps a
Chromium user-data directory under the OS temp directory so multi-step browser
and computer-use calls can share cookies, page state, and focus.

## Sandbox Behavior

On Linux, `computer`/`browser` launch through `bwrap` when it is available. Set
`SPARROW_BROWSER_BWRAP=0` to disable this wrapper for troubleshooting.

On Windows and macOS, Sparrow still applies the tool risk metadata and autonomy
gate. The browser process runs from the workspace root and does not receive API
keys unless your shell environment provides them.

## Examples

Ask Sparrow:

```text
open the local webview, take a screenshot, and tell me if the composer is visible
```

or use the tool shape directly in tests:

```json
{"action":"screenshot","url":"http://127.0.0.1:9339/","full_page":true}
```

The screenshot result is returned as a Sparrow `Block::Image` with MIME
`image/png`, so vision-capable models and WebView artifacts can consume it
without an extra file conversion step.
