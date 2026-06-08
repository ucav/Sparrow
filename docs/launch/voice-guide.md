# Sparrow voice & tone guide

For Abdou and any future contributor writing public copy (README, blog,
launch posts, release notes, replies on HN/Reddit/X).

---

## The one rule

**Sparrow sounds like a tired builder who shipped a real tool, not a
SaaS pitching a series A.**

If you can read a sentence aloud without rolling your eyes, ship it. If
you read it aloud and it sounds like a LinkedIn post, delete it and
start over.

---

## Voice cues

| Use | Avoid |
|---|---|
| "I built this because…" | "We're excited to announce…" |
| "It does X. Here's how." | "Sparrow empowers developers to…" |
| "9 MB binary. 18 ms boot." | "Lightning-fast, blazing performance." |
| "One developer. Two kids." | "Trusted by thousands of teams." |
| "If it breaks, I fix it tonight." | "Enterprise-grade SLA support." |
| First person singular. | Royal "we" (you are one person). |
| Specific numbers. | Vague superlatives. |
| Short sentences. Fragments are fine. | Run-on sentences with three clauses. |
| Real frustration as the hook. | Hype as the hook. |

## Words that are banned

These words are auto-fail. Find-and-replace them out of any draft
before publishing:

- `unleash`, `unleashes`
- `supercharge`, `supercharged`
- `revolutionize`, `revolutionary`
- `10x`, `100x`
- `next-gen`, `next-generation`
- `cutting-edge`
- `game-chang*` (any variant)
- `AI-powered`, `AI-driven`
- `empower`
- `seamlessly`
- `effortlessly`
- `take your X to the next level`
- `world-class`
- `best-in-class`
- `paradigm shift`

If you need to convey one of these ideas, **show** the thing instead.
"9 MB binary, 18 ms boot, zero deps" replaces every adjective above.

## Words that earn trust

These are pre-approved:

- `binary` (not "executable")
- `local-first` (specific, technical)
- `git-backed` (specific, verifiable)
- `routes`, `routing` (Sparrow's verb)
- `rewind` (Sparrow's other verb)
- `cap` / `hard cap` (when describing budget enforcement)
- `MIT` (state it; it's a signal)
- `one developer`, `solo`, `post-burnout` (honesty)
- `nightly CI`, `signed releases`, `clippy clean` (trust through detail)
- `your keys, your machine` (privacy positioning)

## Emoji policy

- 🐦 is fine. It's the mascot. Use sparingly: maximum one per post.
- ✅ ❌ in tables is fine.
- No 🚀 ever. Banned.
- No 🔥 either.
- No 💯, 🙌, 🤝, ✨, 🎉 in launch copy. Save them for casual replies.

## Capitalization & punctuation

- Lowercase headers in tweets and tagline. Looks builder, not corporate.
- Em-dash with spaces around it: `like — this`.
- Oxford comma: yes.
- French apostrophes only in French copy.
- No double exclamation points anywhere, ever.

## Length cues

- Tweet hook: ≤ 240 chars.
- HN title: ≤ 80 chars.
- PH tagline: ≤ 60 chars.
- README intro paragraph: ≤ 4 sentences.
- Release note item: one line, verb first.

## The "would I roast this on r/programming?" test

Read your draft. If a r/programming top commenter would write a snarky
reply in 30 seconds, rewrite it. Examples that pass the test:

> ✅ "Sparrow is a CLI agent. It routes between 38 providers."
> ✅ "I quit Claude Code after a $847 spend and built this."
> ✅ "Edition 2024. Clippy clean. 9 MB. Boots in 18 ms."

Examples that fail:

> ❌ "Sparrow unlocks the next generation of AI development."
> ❌ "Trusted by builders worldwide."
> ❌ "Built with love and ☕."

## Release note voice

```
v0.5.3 — Drop-in compat with ~/.claude/. Privacy policy. Nightly CI.

- New: `sparrow` reads ~/.claude/{CLAUDE.md, commands/, agents/, settings.json}.
- New: PRIVACY.md, written in plain English. No telemetry by default.
- New: nightly CI on Linux/macOS/Windows + cargo audit + budget-capped smoke.
- New: hard cap flags: --max-cost-usd, --max-wall-secs, --max-tokens, --bind.
- Fixed: chat composer history navigation semantics.

Install: cargo install sparrow-cli
```

Notice: no emoji headers, no "✨ Highlights", no "thank you to our
amazing community". Verb-first bullets. Done.

## When in doubt

Sentence to keep on your desk:

> "Real builder, real tool, real frustration."

Read it before posting anything that calls itself "launch copy".
