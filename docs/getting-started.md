# Getting started in 60 seconds

```bash
# 1. Install (any one of)
cargo install sparrow-cli                                                  # everywhere
brew install ucav/tap/sparrow                                              # macOS
scoop bucket add ucav https://github.com/ucav/scoop-bucket && scoop install sparrow   # Windows
winget install ucav.Sparrow                                                # Windows 11
curl -fsSL https://raw.githubusercontent.com/ucav/Sparrow/master/install.sh | sh      # Linux/macOS one-liner

# 2. Launch — first run picks a free provider automatically
sparrow launch

# 3. Try it
sparrow run "explain what this repo does and write a TODO.md"
```

That's it. Three commands, under a minute.

## What just happened

- `sparrow launch` ran the first-run wizard, which scanned your environment
  for **20+ provider keys** (Anthropic, OpenAI, Groq, NVIDIA, Gemini,
  OpenRouter, Cerebras, …). If none were found, it offered to use **Ollama
  locally** (auto-installed if missing) or set up **Groq** (free tier) in one
  prompt.
- The WebView cockpit opened on <http://127.0.0.1:9339/>.
- `sparrow run` routed your task to the cheapest capable model, opened a Git
  checkpoint before any file change, and streamed every event to the cockpit.

## Common next steps

```bash
sparrow plan "refactor the auth module"      # read-only execution plan
sparrow chat                                 # interactive multi-turn
sparrow agent run coder "fix the login bug"  # named SOUL agent
sparrow rewind --last                        # undo last checkpoint
sparrow share                                # upload session to a Gist
sparrow doctor                               # diagnose your install
```

## Run safely from anywhere

Hard budget caps work on every command:

```bash
sparrow run "scrape this site"                  \
  --max-cost-usd 0.50                           \
  --max-wall-secs 300                           \
  --max-tokens 50000                            \
  --autonomy supervised
```

When any limit is reached, Sparrow stops at the current checkpoint and shows
you exactly what was used. No surprise bills.

## Where to go next

- [`docs/architecture.md`](architecture.md) — how routing, checkpoints, and the
  event bus fit together
- [`docs/comparison/vs-competitors.md`](comparison/vs-competitors.md) — honest
  comparison vs Claude Code, Codex, Aider, OpenCode
- [`docs/cookbook.md`](cookbook.md) — recipes (CI helper, RAG over a repo,
  Telegram bot, scheduled jobs)
- [`docs/security.md`](security.md) — sandbox, permissions, secret scanner
- [PRIVACY.md](../PRIVACY.md) — what data Sparrow keeps and where
