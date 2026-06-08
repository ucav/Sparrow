# Launch-day checklist

Print this. Tick boxes physically. Do not skip steps.

---

## J-7 (one week before)

- [ ] All install channels work end-to-end on a clean VM (Linux, macOS, Windows).
- [ ] `cargo publish --dry-run` passes.
- [ ] Homebrew tap repo exists at `github.com/ucav/homebrew-tap` with the formula.
- [ ] Scoop bucket repo exists at `github.com/ucav/scoop-bucket` with the manifest.
- [ ] winget PR submitted to `microsoft/winget-pkgs`.
- [ ] Asciinema cast uploaded to asciinema.org. Note the URL.
- [ ] Screenshots in `docs/screenshots/` are current with the latest UI.
- [ ] SVG cards in `assets/launch/` exported to PNG (1200×630 / 1200×675) and tested in Twitter preview, Slack preview, Discord preview.
- [ ] README "What's new" section names the right version.
- [ ] CI green on master.
- [ ] `cargo audit` clean.
- [ ] PRIVACY.md and SUPPORT.md visible from the repo home.

## J-3

- [ ] Pre-write 8 reply variants for HN (see `docs/launch/responses/`).
- [ ] Pre-write 5 reply variants for X (one-liners + link to issue/file).
- [ ] DM 3 people you know who would honestly try Sparrow and tell you what breaks.
- [ ] Update GitHub repo description and topics: `agent`, `ai`, `cli`, `coding-assistant`, `llm`, `rust`, `local-first`, `mcp`.
- [ ] Pin the repo on your GitHub profile.
- [ ] Update Twitter/X bio with `building github.com/ucav/Sparrow`.
- [ ] Schedule the ProductHunt slot for Tuesday 00:01 PT (use the PH dashboard, not third-party tooling).

## J-1 (Monday)

- [ ] Tag `v0.5.x-launch-ready` in git. Push.
- [ ] Confirm release CI built all binaries on the tag.
- [ ] Compute SHA256 for every release artifact. Update brew/scoop/winget. Push.
- [ ] One quiet tweet with the asciinema link, **no launch wording**. Mostly to warm up the preview cache and check the embed renders.
- [ ] Sleep 8 hours. Don't post tonight.
- [ ] Block 4 hours on Tuesday calendar starting 12:50 UTC. No meetings.

## J-0 — launch Tuesday

### 12:50 UTC

- [ ] Coffee. Real coffee. Not the test one.
- [ ] Close Slack, Discord, email. Phone in airplane mode except for one notification channel (the HN reply notifier of your choice).
- [ ] Open: HN submit page, Twitter compose, the response folder, your repo, the asciinema page.

### 13:00 UTC — POST HN

- [ ] Paste title from `docs/launch/hn-show.md`.
- [ ] Paste body verbatim. Do NOT preview tweak.
- [ ] Submit.
- [ ] Note the HN URL.

### 13:01 → 16:00 UTC — 3-hour reply window

- [ ] Reply to every top-level comment within 15 minutes.
- [ ] If a reply needs a file pointer, link to `github.com/ucav/Sparrow/blob/master/...#Lxxx` with the exact line range.
- [ ] If a reply names a competitor, link to `docs/comparison/vs-competitors.md`. Don't paraphrase.
- [ ] Do not delete any of your replies. Edit only to fix typos in the first 60 seconds.
- [ ] Do NOT tweet the HN link from your own account yet.

### 14:00 UTC — POST X (only if HN is alive)

If HN has ≥ 20 points at 14:00 UTC, post the X thread. Use
`docs/launch/x-thread.md` tweet by tweet, one minute apart so the thread
shows as a thread (not as separate tweets).

If HN died (< 5 points by 14:00 UTC), skip X today. Try again next
Tuesday with a different hook from `docs/launch/x-hook-variants.md`.

### 14:30 UTC — POST r/rust

If both HN and X are live, post `docs/launch/reddit-rust.md`. Wait
30 min after X to avoid all three notifications hitting the same
inbox.

### 15:00 UTC — POST r/LocalLLaMA

Same window logic. `docs/launch/reddit-localllama.md`.

### 16:00 UTC — break

- [ ] Eat. Hydrate. Read no comments for 30 min.
- [ ] When back: triage the open issues that came in. Reply to all.

### 17:00 — 22:00 UTC — sustain

- [ ] Check HN every 30 minutes. Reply to every new top-level comment.
- [ ] Check X every 30 minutes. Reply to every reply and every quote.
- [ ] Check Reddit every hour. Reply to every comment.
- [ ] Track: ⭐ count, install count (from cargo / brew bandwidth), HN ranking, twitter impressions.

### 22:00 UTC — close out

- [ ] Note in a private file: peak HN ranking, peak ⭐/hr, top 3 reactions, top 3 criticisms.
- [ ] Pick the single biggest criticism. Open an issue for it tonight (not tomorrow, tonight).
- [ ] Sleep.

## J+1 — Wednesday

- [ ] Post `docs/launch/reddit-programming.md` (essay-format, no Show).
- [ ] Ship the fix for the top criticism. Commit with `Co-Authored-By` to whoever surfaced it.
- [ ] Reply once on the HN thread linking the fix commit.
- [ ] Tweet: "spent yesterday's launch reading every reply. Here's the first thing I shipped because of you: <commit>".

## J+2 — Thursday

- [ ] Submit to ProductHunt for Friday slot (or skip — PH conversion is
  weak relative to HN/X).
- [ ] Cross-post the X thread to LinkedIn (one-shot, no thread, single
  image, do not link). LinkedIn has a different audience.

## J+5 — Sunday

- [ ] Write the post-mortem thread for X: real numbers, top 3 surprises,
  next milestone. Pin it.
- [ ] Publish `docs/launch/devto-longform.md` to Dev.to with canonical
  unset. Cross-post to Hashnode 24h later with canonical pointing to
  Dev.to.

## J+7 — next Monday

- [ ] Reset: backups, branch hygiene, tag `v0.6.0-dev` to start the
  next cycle.
- [ ] Triage every issue that came in. Close, tag, or schedule each one.
- [ ] Block 4 hours next Tuesday for the next launch wave.

---

## Things to NEVER do during launch week

- Never delete a tweet.
- Never edit the HN title after posting.
- Never reply with "thanks!" alone. Add a code pointer or fact.
- Never engage with bad-faith trolls beyond one factual reply.
- Never DM a tech journalist asking them to cover the launch. They notice and they ignore you.
- Never lower the price. Sparrow is MIT.
