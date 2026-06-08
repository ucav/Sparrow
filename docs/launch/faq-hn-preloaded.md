# HN/launch FAQ — 15 questions pre-answered

These are the questions you WILL get. Pre-loaded answers below. Adapt to
the exact phrasing of each comment — never copy-paste verbatim.

The existing user-facing FAQ is in [`docs/faq.md`](../faq.md); this file
is the **launch-day reply playbook**, not the public-facing FAQ.

---

## 1. "How is this different from Claude Code?"

Three calls Claude Code doesn't make:

  1. The event bus is the source of truth, not the TUI. Crash any
     surface, the run survives.
  2. Checkpoints sit at the tool boundary, not the message boundary.
     `sparrow rewind --last` atomically restores files, conversation,
     and counter as a unit.
  3. Hard caps (--max-cost-usd, --max-wall-secs, --max-tokens) enforce
     in the runtime, not a dashboard.

Plus: 38 providers, MIT, single binary, zero telemetry, drop-in reader
for `~/.claude/`. Comparison table in
[docs/comparison/vs-competitors.md](../comparison/vs-competitors.md).

## 2. "Why not just use Aider?"

Aider is excellent. I used it for six months. Reasons I rewrote:

  - Python dep tree breaks ~quarterly on my machines.
  - No WebView cockpit (the TUI is the whole product).
  - No autonomy contract / HardStop primitive — budget caps are
    advisory, not enforced.
  - No drop-in compat with the wider Claude Code ecosystem.

Aider's diff-application strategy is better than mine and I am stealing
it for v0.6.

## 3. "Why Rust?"

One static binary. 9 MB. Boots in 18 ms. Nobody breaks the install by
bumping a transitive Python or Node dep. `tokio::sync::broadcast` is the
right primitive for the event bus. Edition 2024. Clippy clean.

## 4. "Why not LangChain / LlamaIndex / Haystack?"

They are libraries you import into a Python app. Sparrow is a binary
you install. There is nothing in common to compare.

## 5. "How does it really cost less than Claude Code?"

Two mechanisms:

  - Router picks the cheapest model that can do the task. Ollama is the
    default first hop, so most simple edits cost $0.00.
  - Hard caps abort the run at the limit. No way to overspend by
    accident overnight.

For multi-day projects with frontier-model dependence, Sparrow won't
necessarily be cheaper per token — it'll just stop you from paying
$847 in 4 days without noticing.

## 6. "Is the $847 number real?"

Yes. Screenshot in my DMs if you want to see the redacted dashboard. I
have no incentive to inflate it; I would rather not have spent it.

## 7. "What about security? Has this been audited?"

No external audit yet. What I have:

  - `cargo audit` enforced in CI nightly.
  - `clippy -D warnings`, no `unsafe` outside the git2 FFI shim.
  - Credentials encrypted at rest with ChaCha20-Poly1305, key from OS
    keychain when available.
  - Pre-commit secret scanner bundled (`sparrow hook install`).
  - Sandbox on Linux uses bwrap; on macOS/Windows it reports
    "unsupported" rather than shipping a fake.
  - PRIVACY.md states explicitly: zero telemetry by default.

External audit is on the v0.7 roadmap — pointers to grants/programs
welcome.

## 8. "Does it really work on Windows?"

Yes. CI runs on `windows-latest`, full test suite green. The Windows
installer (`install.ps1`) and Scoop / winget manifests are in
`packaging/`. Sandboxing on Windows is reported as `unsupported` (see
question 7).

## 9. "What's the license? Can I use it commercially?"

MIT. Use it however you want. No CLA required to contribute.

## 10. "Is there a hosted version?"

No, and there won't be one I run. The whole point is that the binary
stays on your machine. If someone forks it and hosts a managed version,
that's their call — MIT lets them.

## 11. "How do you make money?"

I don't yet. GitHub Sponsors is enabled (`.github/FUNDING.yml`). If
Sparrow gets traction I will add an optional paid tier for things that
genuinely need a server (shared team memory, cross-machine session
sync). The CLI itself will stay MIT and free.

## 12. "Why not use MCP servers exclusively?"

Sparrow IS an MCP host + client. Load any existing MCP server with
`sparrow mcp add`. The reason it also has built-in tools is that
shipping a usable binary without bundled `read`/`edit`/`exec` would
mean every user installs N npm packages on first run, which I refuse
to subject anyone to.

## 13. "Doesn't this duplicate what Cursor / Continue / Cline do?"

Different surfaces. Cursor and Cline live inside an editor. Sparrow
lives in the terminal and as a WebView cockpit. The VS Code extension
(`ide/vscode/`) embeds the cockpit, but the source of truth is the CLI.
Sparrow is the right answer when you want the agent reachable from
ssh, CI, a Telegram bot, or a cron job. Cursor is the right answer
when you live full-time in an editor.

## 14. "What's the long-term roadmap?"

v0.6: better diff application (Aider-style), webview replay URL,
official MCP server discovery, signed releases.
v0.7: external security audit, plugin marketplace, deeper VS Code
extension, optional shared-memory backend.
v1.0: stability lock on the public API, formal MSRV policy.

`ROADMAP.md` in the repo for the full version.

## 15. "How can I help?"

Three things, in order of impact:

  1. Open issues for everything that breaks on your machine. I reply
     same day.
  2. Tell me which provider / model combinations work well for which
     task types — I want to tune the router.
  3. If you wrote a skill in `~/.claude/skills/` that you'd port to
     `skills/`, send a PR. The format is the same.
