# First-Launch Setup

On first run (no config detected), Sparrow runs a **conversational onboarding** instead of dumping a TOML file.

## Flow

1. No config → launch **Setup Agent**: minimal AgentRun, ReadOnly tools only, on any available/free model.
2. User states intent in natural language: *"Use my NVIDIA and Anthropic keys, keep daily cost under €5, default to trusted autonomy, reach me on Telegram."*
3. Setup Agent maps utterance to a `Config` via structured output (LLM → JSON, validated against config schema).
4. Asks for missing secrets (keys/OAuth), **validates each provider with a live ping**.
5. Writes `config.toml`, prints plain-language summary, offers confirm/edit.

## Constraints

- Ambiguous/unknown fields trigger a **single** clarifying question at a time (max one).
- **No secret is ever echoed**; keys via masked input or OAuth.
- **Fully offline-capable**: with no provider key, configures Ollama/local + interactive fallback.
- **Idempotent**: a later utterance ("raise the budget to €10") updates only affected fields.

## Quick Start Alternative

For users who prefer manual setup:

```bash
sparrow config --edit              # open config.toml in editor
sparrow auth add anthropic         # add credentials
sparrow model --list               # verify providers
sparrow doctor                     # run diagnostics
```
