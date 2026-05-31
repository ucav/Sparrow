# Routing & Model Freedom

## Provider Adapters

Sparrow supports **35 providers** via three adapter types:

- `anthropic-messages` — Anthropic Claude models
- `openai-compatible` — OpenAI, NVIDIA, Groq, OpenRouter, DeepSeek, Gemini, xAI, and 25+ more
- `ollama` — Self-hosted local models

## Fallback Chains

The router builds an ordered fallback chain:

1. **Primary:** Best model for the task tier (per config policy)
2. **Fallback 1:** Next best according to scoring
3. **Fallback N:** Local/Ollama (terminal fallback)

On 429, 5xx, timeout, or refusal → advance to next in chain. Emit `ModelSwitched` event.

## Budget-Aware Routing

- Models are scored by capability fit minus cost penalty
- Free models get a score boost
- When budget is exhausted, only free/local models remain
- Config: `routing.on_budget = "downgrade" | "stop"`

## Local-First / Offline

- `sparrow run "task" --local` forces local models only
- Config: `routing.free_first = true` prefers free models
- Ollama is the terminal fallback when configured

## `sparrow model`

Show/override routing live:
```bash
sparrow model --list          # show configured providers and models
sparrow model --set openai    # override routing to OpenAI
```
