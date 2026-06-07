# Sparrow Privacy Policy

**Last updated: 2026-06-07**

Sparrow is a local-first command-line agent. It runs on your machine and
sends data only where you explicitly route it.

## What Sparrow stores locally

By default, Sparrow stores the following on your machine **only**:

- **Session transcripts** in `~/.sparrow/transcripts/` — full input/output of
  each run, for replay and rewind.
- **SQLite databases** in `~/.sparrow/state/` — memory facts, knowledge graph
  nodes/edges, session FTS5 index, scheduler state, run registry.
- **Credentials** in `~/.sparrow/auth.enc` (ChaCha20-Poly1305-encrypted at rest
  with a key derived from your OS keychain when available, otherwise from a
  local salt). Credentials are never written in plaintext to disk.
- **Configuration** in `~/.sparrow/config.toml`.

None of these are ever transmitted off-machine by Sparrow itself.

## What Sparrow sends to third parties

Sparrow makes outbound network calls **only** to the providers you have
configured:

- LLM provider APIs (Anthropic, OpenAI, NVIDIA, Groq, Gemini, OpenRouter, …)
  receive the prompts and tool-result context you send via `run`/`chat`.
- The local Ollama daemon (if used) is contacted on `127.0.0.1:11434`.
- Web-search and web-fetch tools contact the chosen search backend
  (DuckDuckGo Lite by default).
- Update checks contact `api.github.com` if you opt in via `sparrow update`.

When a request is routed, the cockpit and the `--json` stream show **which
provider was selected, why, and how many tokens were sent**. You can audit
every byte that left your machine.

## Telemetry

**Sparrow does not collect telemetry by default.**

There is no anonymous-usage ping, no error-reporting beacon, no analytics SDK.
The CLI works fully offline once installed.

If a future opt-in telemetry channel is added, it will:

1. Be **off by default**.
2. Require an explicit `sparrow telemetry enable` command.
3. Be documented here with the exact schema of every field collected.
4. Never include prompts, tool outputs, file contents, file paths, credentials,
   or any user-generated content.

## Gateway transports (Telegram / Discord / Slack / Email / …)

When you enable a gateway transport, Sparrow becomes a bot on that platform
using **your** bot token. The transport library sends messages to the
platform's servers (Telegram, Discord, Slack, …) according to their respective
terms. Sparrow does not proxy these messages through any Sparrow-controlled
server.

## Sharing a session (`sparrow share`)

`sparrow share` uploads the **current** session transcript to a GitHub Gist
under **your** GitHub account, using the `gh` CLI you have already
authenticated. The transcript is whatever you ran — Sparrow does not add or
remove anything before upload.

You are responsible for redacting credentials, customer data, or other
sensitive content before sharing. Sparrow's `redaction` pass best-effort
masks obvious secrets (API keys, tokens, private-key blocks); it is not a
guarantee.

## Children

Sparrow is a developer tool and is not directed at children under 13.

## Contact

Privacy questions or concerns: open an issue at
<https://github.com/ucav/Sparrow/issues> with the `privacy` label.
