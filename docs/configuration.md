# Configuration

Sparrow loads config from (in priority order):
1. CLI flags (`--autonomy`, `--model`, `--budget`, `--sandbox`, etc.)
2. Environment variables (`SPARROW_*`)
3. `~/.config/sparrow/config.toml`
4. Defaults

## config.toml Reference

```toml
[defaults]
autonomy = "trusted"          # supervised | trusted | autonomous
sandbox  = "local-hardened"   # local | local-hardened | docker | ssh:host
theme    = "captain"

[routing]
free_first = true
policy = { trivial = "local", small = "groq", medium = "nvidia", hard = "anthropic", vision = "anthropic" }
on_budget = "downgrade"       # downgrade | stop

[budget]
daily_usd = 5.0
session_usd = 1.0

[providers.<name>]
adapter = "openai-compatible"     # anthropic-messages | openai-compatible | ollama
base_url = "https://api.example.com/v1"
models = ["model-name"]
api_key_env = "PROVIDER_API_KEY"  # env var for the key

[surfaces.telegram]
enabled = true
token_env = "TELEGRAM_BOT_TOKEN"
allow_users = ["123456789"]

[surfaces.discord]
enabled = false

[surfaces.slack]
enabled = false

[skills]
dir = "~/.config/sparrow/skills"
curator_cron = "0 */6 * * *"
```

## Environment Variables

| Variable | Overrides |
|---|---|
| `SPARROW_DEFAULTS_AUTONOMY` | `defaults.autonomy` |
| `SPARROW_DEFAULTS_SANDBOX` | `defaults.sandbox` |
| `SPARROW_BUDGET_DAILY` | `budget.daily_usd` |
| `SPARROW_BUDGET_SESSION` | `budget.session_usd` |
| `SPARROW_THEME` | `theme` |
| `ANTHROPIC_API_KEY` | Anthropic credential |
| `OPENAI_API_KEY` | OpenAI credential |
| `NVIDIA_API_KEY` | NVIDIA credential |
| `OLLAMA_HOST` | Ollama base URL |

## Credentials

Credentials are resolved per provider, not all-or-nothing:

1. OS keychain when the optional keyring backend is available.
2. Local encrypted file fallback in the Sparrow config directory.
3. Environment variables such as `NVIDIA_API_KEY`, `OPENAI_API_KEY`, and
   `ANTHROPIC_API_KEY`.

The file fallback writes `auth.enc` as a ChaCha20-Poly1305 envelope and stores a
32-byte data key in `auth.key`. Both files use restrictive permissions where the
platform supports them. Older plain JSON and legacy XOR auth files are still
read for migration and are rewritten as encrypted envelopes on the next
credential save.
