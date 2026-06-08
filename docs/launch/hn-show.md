# Show HN — final post copy

**Title (≤ 80 chars):**

```
Show HN: Sparrow – tired of Claude Code forgetting my sessions, I built this
```

**Body (paste verbatim):**

```
Hi HN — I'm Abdou, solo dev, post-burnout, two kids.

I've been paying for Claude Code since the beta. Two weeks ago I checked the
billing dashboard: $847 in 4 days, on 11 modified files, and the assistant
kept losing context across crashes. I closed the laptop, opened a Rust
project, and started Sparrow.

What it is

  - One static binary. No Node, no Python, no Docker.
  - Routes each task to the cheapest model that can do it. 38 providers
    wired (Anthropic, OpenAI, Groq, NVIDIA, Gemini, OpenRouter, DeepSeek,
    Mistral, xAI, Cerebras, Ollama, …). Bring your own keys.
  - Auto-checkpoints to git before any mutating tool call.
    `sparrow rewind` undoes the whole run, files + conversation + counters.
  - Hard caps per run: --max-cost-usd, --max-wall-secs, --max-tokens. Hits
    the limit, stops at the last checkpoint, prints the receipt.
  - WebView cockpit on :9339, TUI cockpit in the terminal, or pure --json
    NDJSON stream. Same event bus.
  - Drop-in reader for ~/.claude/{CLAUDE.md, commands/, agents/, settings.json}
    so migration off Claude Code is zero effort.
  - Pre-commit secret scanner bundled.
  - PRIVACY.md is explicit: no telemetry by default, never sends file
    contents anywhere.

What it is NOT

  - A framework. Nothing to import.
  - A SaaS. There is no server I control.
  - A wrapper around one provider's SDK.

Install:   cargo install sparrow-cli
Repo:      https://github.com/ucav/Sparrow
30s demo:  https://asciinema.org/a/<UPLOAD_ID>

I'd love feedback on three concrete things:

  1. The routing policy in src/router/mod.rs — does the heuristic match
     how you'd pick a model?
  2. The drop-in reader in src/onboarding/claude_compat.rs — what did I
     miss in the .claude/ layout?
  3. The autonomy contract in src/autonomy/mod.rs — too restrictive, or
     too loose?

Roast it. I built this because the existing tools made me angry, and the
worst review I can get is "how is this different from X". So tell me.
```

---

## Post hygiene

- **Headline ≤ 80 chars.** Counted: 76. ✓
- **No emoji in title.** HN allergic. ✓
- **First word in body is "Hi"** — sets human tone immediately.
- **Personal stake in line 2** — the $847 number is the hook. Real or scaled-down to a real number you can defend.
- **Three explicit asks at the bottom** — gives commenters something concrete to react to instead of "looks cool".

## Timing

- Tuesday or Wednesday.
- **12:55 UTC** (= 5:55 PT, 8:55 ET, 14:55 CET). Front page peeks at 13:00 UTC.
- Do **not** post on Monday (Show HN backlog) or Friday (low traffic).
- Do **not** post on US public holidays.

## First 90 minutes after posting

- Stay at the keyboard. Reply to every top-level comment in under 15 minutes
  for the first 90 minutes. HN ranks heavily on engagement velocity.
- Replies must be specific: code-pointer, file path, line number. Generic
  thank-yous get downranked.
- The pre-loaded answers in `docs/launch/responses/` are starting points,
  not final replies — adapt to the actual question.

## Do NOT do

- Do NOT mention the post anywhere else (Twitter, Slack, Discord, friends'
  Telegram, etc.) until **after** it has either died or hit the front page
  on its own merits. HN detects orchestrated upvoting and flags posts.
- Do NOT edit the title after posting.
- Do NOT delete and repost — bans the URL.
