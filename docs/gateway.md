# Gateway

Sparrow's gateway turns the agent loop into a service that other surfaces can
drive. It is the **daemon** that owns the engine, the scheduler, and the
external transports.

## Daemon lifecycle

```bash
sparrow gateway start    # launches transports + WS API + abort poller
sparrow gateway status   # PID + WS port + DB path
sparrow gateway health   # deeper probe (stale PID warning)
sparrow gateway abort <run-id>
sparrow gateway stop
```

`gateway start` writes a PID file to `<state>/sparrow/gateway.pid`. `status`
and `health` are honest: a missing process or a closed port is reported
exactly as it is, never as "running."

## Transports

| Transport | Module | Status |
|---|---|---|
| Telegram | `gateway::telegram` | Alpha — needs `TELEGRAM_BOT_TOKEN` |
| Discord | `gateway::discord` | Alpha — needs `DISCORD_BOT_TOKEN` |
| Slack | `gateway::slack` | Alpha — Socket Mode, needs `SLACK_APP_TOKEN` + bot token |
| Email | `gateway::email` | Experimental (feature flag `email`) |
| WebSocket API | `gateway::ws` | Stable — port 9338, `/status` round-trip tested |
| Extras (WhatsApp/Signal/Teams/Feishu/WeCom/QQ) | `gateway::extra_transports` | Experimental |

A transport that has no token simply stays disabled and prints a clear
"no token (set FOO_BOT_TOKEN)" line at startup. No silent failures.

## Scoped sessions

Session keys are scoped per surface + channel + peer so a user can keep
parallel conversations across channels. The format is:

```
gateway:<surface>:channel:<chat>:peer:<user>
```

Sanitization strips empty/punctuated components so the key remains a valid
SQLite primary key.

CLI surface for the persisted store:

```bash
sparrow sessions list
sparrow sessions export <id> [path]
sparrow sessions cleanup --older-than-days <n>
```

## Allow-list

Every messaging surface has an `allow_users` (or `allowed_to` for email) list
in `config.toml`. When empty, the gateway answers nobody and `sparrow security
audit` flags the surface as a **Critical** wildcard-sender finding.

## Run registry & abort

The router owns a `RunRegistry` (`Arc<Mutex<HashMap<run_id, AbortHandle>>>`).
Every spawned gateway run is registered on spawn and de-registered on
completion. `sparrow gateway abort <run>` writes
`<state>/sparrow/gateway-abort/<run>.abort`; the gateway daemon polls that
directory every two seconds, calls `RunRegistry::abort`, and removes the
signal file. The mechanism crosses processes (CLI ↔ daemon) without needing
RPC.

## Deliverables

When a run produces files, the gateway exposes them through the WebView's
`GET /artifacts` endpoint (Phase 10) rather than re-uploading them through
every transport. Telegram, Discord, and Slack adapters fall back to a link.

## See also

- `docs/gateway-sessions.md` — deep dive on session keys and cleanup budgets.
- `docs/github-action.md` — `@sparrow review` PR comment flow (a gateway-shaped
  trigger living in GitHub instead of a chat surface).
- `docs/security.md` — wildcard-sender detection and other gateway audit
  rules.
