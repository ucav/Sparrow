// src/cmd_handlers/setup_cmd.rs — sparrow setup command handler

use sparrow::auth::store::ChainedAuthStore;
use sparrow::auth::{AuthStore, Credential};
use sparrow::config::{Config, ConfigStore, FsConfigStore, ProviderConfig};

pub async fn handle_setup(config: &Config, store: &FsConfigStore) -> anyhow::Result<()> {
    use sparrow::tui::theme::boot_sequence;
    use std::io::{self, Write};

    for line in boot_sequence() {
        println!("{}", line);
    }
    println!();
    println!("═══ SPARROW SETUP ═══");
    println!();
    println!("Sparrow setup configures providers, model routing, budget, and autonomy.");
    println!();
    println!("Current configuration:");
    println!("  Config dir : {:?}", config.config_dir);
    println!("  State dir  : {:?}", config.state_dir);
    println!("  Autonomy   : {:?}", config.defaults.autonomy);
    println!(
        "  Budget     : ${}/day, ${}/session",
        config.budget.daily_usd, config.budget.session_usd
    );
    println!();

    let effective = sparrow::config::effective_provider_configs(config);
    if effective.is_empty() {
        println!("No provider detected yet.");
    } else {
        println!("Detected/configured providers:");
        for (name, pconfig) in &effective {
            println!("  {} (adapter: {})", name, pconfig.adapter);
            for model in &pconfig.models {
                println!("    - {}", model);
            }
        }
    }

    println!();
    println!("Recommended first setup:");
    println!("  - local/free: ollama");
    println!("  - cheap cloud: nvidia");
    println!("  - strong cloud: anthropic");
    println!();
    print!("Configure or update a provider now? [Y/n] ");
    io::stdout().flush().ok();
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    if matches!(answer.trim().to_lowercase().as_str(), "n" | "no" | "non") {
        println!("Setup left unchanged. Run 'sparrow console' for the WebView config panel.");
        return Ok(());
    }

    let registry = sparrow::config::providers::provider_registry();
    println!("\nAvailable providers:");
    for def in registry.iter().take(18) {
        let env_state = def
            .api_key_env
            .as_ref()
            .map(|env| {
                if std::env::var(env)
                    .map(|v| !v.trim().is_empty())
                    .unwrap_or(false)
                {
                    "env found"
                } else {
                    "env missing"
                }
            })
            .unwrap_or("no key needed");
        println!("  {:18} {:22} {}", def.id, def.label, env_state);
    }
    println!("  custom             Custom Endpoint");

    print!("\nProvider id [nvidia]: ");
    io::stdout().flush().ok();
    let mut provider_id = String::new();
    io::stdin().read_line(&mut provider_id)?;
    let provider_id = provider_id.trim();
    let provider_id = if provider_id.is_empty() {
        "nvidia"
    } else {
        provider_id
    };
    let Some(def) = sparrow::config::providers::find_provider(provider_id) else {
        anyhow::bail!(
            "Unknown provider '{}'. Use 'sparrow model --list' or the WebView config panel.",
            provider_id
        );
    };

    let default_models = sparrow::config::providers::default_models(&def.id);
    let default_model = default_models
        .first()
        .cloned()
        .unwrap_or_else(|| "model".into());
    print!("Model [{}]: ", default_model);
    io::stdout().flush().ok();
    let mut model = String::new();
    io::stdin().read_line(&mut model)?;
    let model = model.trim();
    let model = if model.is_empty() {
        default_model
    } else {
        model.to_string()
    };

    let mut next = config.clone();
    next.providers.insert(
        def.id.clone(),
        ProviderConfig {
            adapter: def.adapter.clone(),
            base_url: Some(def.base_url.clone()),
            models: vec![model],
            api_key_env: def.api_key_env.clone(),
        },
    );

    print!(
        "Default routing provider for medium tasks [{}]? [Y/n] ",
        def.id
    );
    io::stdout().flush().ok();
    let mut route_answer = String::new();
    io::stdin().read_line(&mut route_answer)?;
    if !matches!(
        route_answer.trim().to_lowercase().as_str(),
        "n" | "no" | "non"
    ) {
        next.routing.policy.insert("medium".into(), def.id.clone());
        if def.tags.iter().any(|t| t == "strong" || t == "code") {
            next.routing.policy.insert("small".into(), def.id.clone());
        }
    }

    if let Some(env_name) = &def.api_key_env {
        if std::env::var(env_name)
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false)
        {
            println!(
                "Credential: {} is already present in environment.",
                env_name
            );
        } else {
            print!(
                "Paste API key for {} now, or leave empty to use env later: ",
                def.label
            );
            io::stdout().flush().ok();
            let mut key = String::new();
            io::stdin().read_line(&mut key)?;
            let key = key.trim();
            if !key.is_empty() {
                let auth = ChainedAuthStore::new(next.config_dir.clone());
                auth.set(&def.id, Credential::api_key(key.to_string()))?;
                println!("Credential stored for {}.", def.id);
            }
        }
    }

    store.save(&next)?;
    println!("\nSetup saved.");
    println!("Run 'sparrow doctor' to verify or 'sparrow console' for the graphical WebView.");

    Ok(())
}
