# FAQ

### Is Sparrow a wrapper around Claude Code / Codex / OpenCode?

No. Sparrow is a self-contained Rust binary. It does **not** shell out to claude-code, codex, opencode, or any other coding CLI. It re-implements the agentic loop natively. The only external calls are model-provider HTTP endpoints and MCP servers.

### Can it run without cloud models?

Yes. Sparrow is **local-first**. With Ollama configured, you can run full tasks offline, `$0.00`, without any API key.

### How does autonomy stay safe?

Autonomy is a **continuous dial**, not binary. Every mutating action goes through the autonomy gate. Mutating batches are checkpointed. Destructive actions are never silently allowed. Budget caps and sandbox escape signals trigger hard stops.

### How does rollback work?

Before any mutating action, Sparrow snapshots the workspace via git. `sparrow rewind [id]` restores instantly. The checkpoint timeline is visible in the TUI.

### What providers are supported?

35 providers including Anthropic, OpenAI, NVIDIA, Groq, OpenRouter, DeepSeek, Gemini, Ollama, xAI, Mistral, and more. See the provider registry in `src/config/providers.rs`.

### Is the TUI required?

No. Use `sparrow run "task" --json` for headless/CI mode, `sparrow chat` for interactive line-based mode, or `sparrow --web` for the browser console.

### Can I use it through Telegram / Discord / Slack?

Yes. Start with `sparrow gateway start` and configure the messaging surfaces in `config.toml`.

### What is implemented today?

The Rust kernel (M0) compiles and passes 31 tests. The full architecture (M0-M5) is implemented. Polish (M6) is in progress. See [ROADMAP.md](../ROADMAP.md).
