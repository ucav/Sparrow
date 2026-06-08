# X / Twitter launch thread

Post Tuesday at 14:00 CET (after the HN post has settled at 13:00 UTC).
Always quote-tweet from your own oldest mention of Sparrow if you have one,
otherwise standalone.

---

## Tweet 1 — the hook (MUST have media)

> $847 in 4 days on Claude Code.
> 11 files modified.
> Closed the laptop. Opened Rust. Built Sparrow.
>
> [attach: assets/launch/x-hook-receipt.svg exported as PNG]

## Tweet 2

> Sparrow is one static binary.
> No Node. No Python. No Docker.
> No subscription. No cloud.
>
> `cargo install sparrow-cli`

## Tweet 3

> It routes each task to the cheapest model that can do it.
> 38 providers wired. Ollama is the default first hop.
> Your keys. Your choices.

## Tweet 4

> Every mutating step gets a git checkpoint, automatically.
> `sparrow rewind --last` undoes everything — files, conversation, counters.
>
> No more "shit, I left it running overnight".

## Tweet 5

> Hard caps per run:
>   `--max-cost-usd 0.50`
>   `--max-wall-secs 300`
>   `--max-tokens 50000`
>
> Hits the limit, stops at the last checkpoint, prints the receipt.

## Tweet 6

> Coming from Claude Code?
> Sparrow reads `~/.claude/CLAUDE.md`, `commands/`, `agents/`, `settings.json`
> on first run.
> Migration: zero effort.

## Tweet 7

> No telemetry by default.
> `PRIVACY.md` in the repo, written in plain English.
> MIT, Rust, 9 MB binary.
>
> github.com/ucav/Sparrow

## Tweet 8

> I have 61 followers and 2 kids.
> If Sparrow saves you a dollar, repost this.
> If it breaks, open an issue and I fix it tonight.
> 🐦

---

## Media rotation

- Tweet 1 → `x-hook-receipt` (the $847 visual)
- Tweet 3 → `comparison-card` (vs Claude Code / Codex / Aider)
- Tweet 4 → 5-second GIF of `sparrow rewind` succeeding
- Tweet 5 → `cli-demo-card` (the `sparrow run …` still)
- Tweet 7 → repo screenshot (file tree) with PRIVACY.md highlighted
- Tweet 8 → the sparrow mascot SVG

Tweet 1 with NO media = stillborn. Use the SVG even if PNG export looks rough.

## Mention list (max 2)

Pick exactly two of these in tweet 7, never more:

- @simonw — writes about every interesting devtool, very fast turnaround
- @swyx — covers devtool launches in his newsletter
- @karpathy — long-shot but he RTs lean Rust craft

Do not mention Anthropic / OpenAI / Cursor / Devin. Looks petty.

## Reply protocol

When someone replies "how does this compare to X?" → drop a one-liner +
`docs/comparison/vs-competitors.md` link. No paraphrase.

When someone replies "is the $847 real?" → "yes, screenshot in my DMs if
you want it. or run `sparrow doctor` and see for yourself why I built it."

When someone replies "why Rust?" → 13 words max: "9 MB binary, 18 ms boot,
nobody breaks it by bumping a dep."
