# Getting help with Sparrow

Sparrow is maintained by one developer. The fastest way to get a useful
answer is to pick the right channel.

## I think I found a bug

Open an issue: <https://github.com/ucav/Sparrow/issues/new?template=bug_report.yml>

Include:

- Sparrow version (`sparrow --version`)
- OS + architecture (`uname -a` on Linux/macOS, `winver` on Windows)
- Reproduction steps (commands you ran, in order)
- Expected behaviour vs what you saw
- `sparrow doctor` output if relevant

Bugs with a reproduction are usually fixed within 24 hours. Bugs without
a reproduction may sit for weeks because I genuinely can't act on them.

## I want a feature

Open a discussion first: <https://github.com/ucav/Sparrow/discussions>

A discussion is cheap and lets people upvote / push back before anyone
writes code. Once a feature has been validated in a discussion, open an
issue linking back to it.

## I have a question about how to use Sparrow

Two options, in this order:

1. Read [`docs/getting-started.md`](docs/getting-started.md) (60 seconds)
   and [`docs/faq.md`](docs/faq.md).
2. If your question isn't covered, open a discussion under the **Q&A**
   category.

Please don't open issues for usage questions — the issue tracker stays
focused on bugs and roadmap items.

## I want to report a security vulnerability

**Do not open a public issue.** Read [`SECURITY.md`](SECURITY.md) for
the responsible disclosure procedure.

## I want to contribute code

Read [`CONTRIBUTING.md`](CONTRIBUTING.md). Short version:

1. Open an issue or discussion first so we agree on the approach.
2. Fork, branch, code, test.
3. `cargo fmt --all && cargo clippy --no-deps && cargo test` — all green
   before opening the PR.
4. PR description includes a one-line summary + a "why" section.

## Response time expectations

- **Security reports**: under 24 hours.
- **Bugs with reproduction**: under 48 hours for first response.
- **Bugs without reproduction**: when I can reproduce them.
- **Feature discussions**: I read them all but I don't reply to every
  one — assume "no" unless I say "yes".
- **Usage questions**: community-first; I jump in if no one else does
  within 72 hours.

## Sponsoring the project

GitHub Sponsors is enabled. The maintainer is one person with two kids,
post-burnout, working on Sparrow nights and weekends. Sponsorship is
the cleanest way to make sure the project keeps shipping.

<https://github.com/sponsors/ucav>

Sponsorship is **never** required to get a bug fixed. Sparrow is MIT
and will stay free.
