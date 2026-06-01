//! Conversational onboarding via a minimal Setup Agent (§16).
//!
//! Reads natural-language intent from the user, sends it to a Brain with a
//! structured-output system prompt, parses the resulting JSON into Config
//! updates, validates against the registry, prompts for missing secrets,
//! and writes config.toml.

use std::io::{self, Write};
use std::sync::Arc;

use crate::auth::{AuthStore, Credential};
use crate::config::{Config, ConfigStore, FsConfigStore, ProviderConfig};
use crate::event::AutonomyLevel;
use crate::memory::Memory;
use crate::provider::{Brain, BrainEvent, BrainRequest, ContentBlock, Msg};

use futures::StreamExt;

/// Run the conversational setup. Falls back to a minimal interactive flow if no
/// Brain is reachable.
pub async fn run_setup_agent(
    config: &Config,
    store: &FsConfigStore,
    memory: Arc<dyn Memory>,
    build_brains: impl Fn(
        &Config,
        &Arc<dyn Memory>,
        bool,
    ) -> std::collections::HashMap<String, Vec<Arc<dyn Brain>>>,
) -> anyhow::Result<()> {
    print_welcome(config);

    // 1. Locate a Brain — env-keyed provider, or Ollama, or fall back.
    let providers = build_brains(config, &memory, false);
    let any_brain: Option<Arc<dyn Brain>> = providers
        .values()
        .flat_map(|chain| chain.iter())
        .next()
        .cloned();

    let Some(brain) = any_brain else {
        eprintln!(
            "⚠ Aucun modèle disponible pour piloter le Setup Agent.\n  → fallback en mode interactif simple.\n"
        );
        return fallback_interactive(config, store).await;
    };

    println!("Setup Agent piloté par : {}\n", brain.id());
    println!("Décris ce que tu veux en langage naturel.");
    println!("Exemple :");
    println!("  \"Use my NVIDIA and Anthropic keys, keep daily cost under €5,");
    println!(
        "   default to trusted autonomy, reach me on Telegram, code in a hardened sandbox.\"\n"
    );
    print!("> ");
    io::stdout().flush().ok();

    let mut intent = String::new();
    io::stdin().read_line(&mut intent)?;
    let intent = intent.trim().to_string();
    if intent.is_empty() {
        eprintln!("Aucun intent fourni, setup annulé.");
        return Ok(());
    }

    // 2. Ask the LLM for structured JSON
    let json_value = ask_llm_for_setup(&brain, &intent, None).await?;

    // 3. Handle optional clarifying question (max 1 round per §16)
    let mut value = json_value;
    if let Some(question) = value
        .get("clarifying_question")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
    {
        println!("\n{}\n", question);
        print!("> ");
        io::stdout().flush().ok();
        let mut clarif = String::new();
        io::stdin().read_line(&mut clarif)?;
        let clarif = clarif.trim().to_string();
        value = ask_llm_for_setup(&brain, &intent, Some(&clarif)).await?;
    }

    // 4. Validate + merge into Config
    let updated = apply_setup_json(config, &value)?;

    // 5. Prompt for missing secrets
    let missing: Vec<String> = value
        .get("missing_keys")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    if !missing.is_empty() {
        let auth = crate::auth::store::ChainedAuthStore::new(updated.config_dir.clone());
        for env_var in &missing {
            // Heuristic: env var like `ANTHROPIC_API_KEY` → provider id `anthropic`
            let provider_id = env_var.to_lowercase().replace("_api_key", "");
            print!("\nPaste API key for {} ({}): ", provider_id, env_var);
            io::stdout().flush().ok();
            let key = rpassword::read_password()
                .or_else(|_| {
                    let mut key = String::new();
                    io::stdin().read_line(&mut key)?;
                    Ok::<_, std::io::Error>(key)
                })?
                .trim()
                .to_string();
            if !key.is_empty() {
                auth.set(&provider_id, Credential::api_key(key))?;
                println!("  ✓ Credential stored for {}", provider_id);
            }
        }
    }

    // 6. Persist + summary
    store.save(&updated)?;
    print_summary(&updated, &value);
    Ok(())
}

fn print_welcome(config: &Config) {
    println!("══════════════════════════════════════════════════");
    println!("  Sparrow Setup Agent (§16)");
    println!("══════════════════════════════════════════════════");
    println!("  Config dir : {:?}", config.config_dir);
    println!("  State dir  : {:?}", config.state_dir);
    println!();
}

const SETUP_SYSTEM_PROMPT: &str = r#"You are the Sparrow Setup Agent. The user describes in natural language how they want Sparrow configured. Output ONLY valid JSON conforming to this exact schema (no markdown, no commentary):

{
  "providers_to_enable": ["<provider_id>", ...],
  "routing_policy": {"trivial": "<id>", "small": "<id>", "medium": "<id>", "hard": "<id>", "vision": "<id>"},
  "budget_daily_usd": <number>,
  "budget_session_usd": <number>,
  "default_autonomy": "supervised" | "trusted" | "autonomous",
  "default_sandbox": "local" | "local-hardened" | "docker",
  "surfaces": {
    "telegram": {"enabled": <bool>, "allow_users": [<numbers>]},
    "discord":  {"enabled": <bool>, "allow_users": [<strings>]},
    "slack":    {"enabled": <bool>, "allow_users": [<strings>]}
  },
  "missing_keys": ["ANTHROPIC_API_KEY", ...],
  "clarifying_question": null | "<one short question if absolutely needed>"
}

Valid provider ids: anthropic, openai-codex, nvidia, openrouter, deepseek, gemini, xai, huggingface, nous, novita, alibaba, bedrock, kimi-coding, minimax, xiaomi, zai, gmi, arcee, stepfun, custom, azure-foundry, qwen-oauth, opencode-zen, kilocode, copilot, alibaba-coding-plan, ollama, groq, together, cerebras, mistral, fireworks, perplexity, cohere.

Rules:
- Map user mentions ("NVIDIA", "Anthropic", "Telegram", "local") to the right keys.
- If the user mentions a cost cap in euros or dollars, set both daily_usd and session_usd (session_usd typically 1/5 of daily).
- Only ask a clarifying_question if a critical field is genuinely ambiguous; otherwise null.
- For each provider in providers_to_enable, add its standard env-key name to missing_keys.
- Default to autonomy=trusted, sandbox=local-hardened if not specified.
"#;

async fn ask_llm_for_setup(
    brain: &Arc<dyn Brain>,
    intent: &str,
    clarification: Option<&str>,
) -> anyhow::Result<serde_json::Value> {
    let mut messages = vec![Msg {
        role: "user".into(),
        content: vec![ContentBlock::Text {
            text: format!("User intent:\n{}", intent),
        }],
    }];
    if let Some(c) = clarification {
        messages.push(Msg {
            role: "user".into(),
            content: vec![ContentBlock::Text {
                text: format!("Clarification:\n{}", c),
            }],
        });
    }

    let req = BrainRequest {
        system: Some(SETUP_SYSTEM_PROMPT.into()),
        messages,
        tools: vec![],
        max_tokens: 1500,
        temperature: 0.0,
        stop: vec![],
    };

    let mut stream = brain.complete(req).await?;
    let mut buf = String::new();
    while let Some(evt) = stream.next().await {
        match evt {
            BrainEvent::TextDelta(t) => buf.push_str(&t),
            BrainEvent::Done(_) => break,
            BrainEvent::Error(e) => anyhow::bail!("Setup Agent LLM error: {}", e),
            _ => {}
        }
    }

    // Strip markdown fences if present
    let cleaned = buf
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    serde_json::from_str(cleaned)
        .map_err(|e| anyhow::anyhow!("Setup Agent returned non-JSON: {}\nRaw:\n{}", e, buf))
}

fn apply_setup_json(base: &Config, value: &serde_json::Value) -> anyhow::Result<Config> {
    let mut next = base.clone();
    let registry: std::collections::HashSet<String> = crate::config::providers::provider_registry()
        .into_iter()
        .map(|p| p.id)
        .collect();

    // Providers
    if let Some(arr) = value.get("providers_to_enable").and_then(|v| v.as_array()) {
        for v in arr {
            let Some(id) = v.as_str() else { continue };
            if !registry.contains(id) {
                eprintln!("  ! Unknown provider id '{}' — skipped.", id);
                continue;
            }
            let def = crate::config::providers::find_provider(id);
            next.providers
                .entry(id.to_string())
                .or_insert_with(|| ProviderConfig {
                    adapter: def
                        .as_ref()
                        .map(|d| d.adapter.clone())
                        .unwrap_or_else(|| "openai-compatible".into()),
                    base_url: def.as_ref().map(|d| d.base_url.clone()),
                    models: def
                        .as_ref()
                        .map(|d| crate::config::providers::default_models(&d.id))
                        .unwrap_or_default(),
                    api_key_env: def.and_then(|d| d.api_key_env),
                });
        }
    }

    // Routing policy
    if let Some(obj) = value.get("routing_policy").and_then(|v| v.as_object()) {
        for (tier, provider) in obj {
            if let Some(p) = provider.as_str() {
                if registry.contains(p) {
                    next.routing.policy.insert(tier.clone(), p.to_string());
                }
            }
        }
    }

    // Budgets
    if let Some(d) = value.get("budget_daily_usd").and_then(|v| v.as_f64()) {
        next.budget.daily_usd = d.max(0.0);
    }
    if let Some(s) = value.get("budget_session_usd").and_then(|v| v.as_f64()) {
        next.budget.session_usd = s.max(0.0);
    }

    // Autonomy
    if let Some(s) = value.get("default_autonomy").and_then(|v| v.as_str()) {
        next.defaults.autonomy = match s {
            "supervised" => AutonomyLevel::Supervised,
            "trusted" => AutonomyLevel::Trusted,
            "autonomous" => AutonomyLevel::Autonomous,
            _ => next.defaults.autonomy,
        };
    }

    // Sandbox
    if let Some(s) = value.get("default_sandbox").and_then(|v| v.as_str()) {
        next.defaults.sandbox = s.to_string();
    }

    // Surfaces — telegram (most common)
    if let Some(tg) = value.pointer("/surfaces/telegram") {
        let enabled = tg.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
        let allow_users: Vec<String> = tg
            .get("allow_users")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| {
                        v.as_str()
                            .map(String::from)
                            .or_else(|| v.as_i64().map(|n| n.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        if enabled {
            next.surfaces.telegram = Some(crate::config::MessagingSurface {
                enabled: true,
                allow_users,
                token_env: Some("TELEGRAM_BOT_TOKEN".into()),
            });
        }
    }

    Ok(next)
}

fn print_summary(config: &Config, value: &serde_json::Value) {
    println!("\n══════════════════════════════════════════════════");
    println!("  Setup terminé");
    println!("══════════════════════════════════════════════════");
    let provs: Vec<String> = config.providers.keys().cloned().collect();
    println!("  Providers     : {}", provs.join(", "));
    println!(
        "  Budget        : ${:.2}/day, ${:.2}/session",
        config.budget.daily_usd, config.budget.session_usd
    );
    println!("  Autonomy      : {:?}", config.defaults.autonomy);
    println!("  Sandbox       : {}", config.defaults.sandbox);
    if let Some(tg) = &config.surfaces.telegram {
        if tg.enabled {
            println!(
                "  Telegram      : enabled ({} users allowed)",
                tg.allow_users.len()
            );
        }
    }
    if let Some(arr) = value.get("missing_keys").and_then(|v| v.as_array()) {
        if !arr.is_empty() {
            println!(
                "  Missing keys  : {}",
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
    println!("\n→ Run 'sparrow doctor' to verify.\n");
}

/// Last-resort fallback if no Brain is reachable at all.
async fn fallback_interactive(config: &Config, store: &FsConfigStore) -> anyhow::Result<()> {
    println!("Fallback minimal: configuring ollama (local) as default.\n");
    let mut next = config.clone();
    next.providers.insert(
        "ollama".into(),
        ProviderConfig {
            adapter: "ollama".into(),
            base_url: Some("http://localhost:11434/v1".into()),
            models: vec!["qwen3.5:32b".into()],
            api_key_env: None,
        },
    );
    next.routing
        .policy
        .insert("trivial".into(), "ollama".into());
    next.routing.policy.insert("small".into(), "ollama".into());
    next.routing.policy.insert("medium".into(), "ollama".into());
    next.routing.policy.insert("hard".into(), "ollama".into());
    store.save(&next)?;
    println!(
        "Ollama configured locally. Add real provider keys with 'sparrow auth add <provider>'."
    );
    Ok(())
}
