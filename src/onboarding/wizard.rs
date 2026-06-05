//! First-run interactive terminal wizard.
//!
//! Detects existing API keys from environment variables, checks if the `gh`
//! CLI is installed for GitHub Copilot, offers free providers (NVIDIA NIM,
//! Groq) with setup instructions, and saves the resulting configuration to
//! the user's Sparrow config file.
//!
//! Has a "Quick start with what you have" option that skips all prompts.

use std::io::{self, Write};

use crate::auth::{AuthStore, Credential};
use crate::config::{Config, ConfigStore, ProviderConfig};
use crate::provider::detect::{self, DetectedProvider, ProviderTier};

// ─── Wizard entry point ──────────────────────────────────────────────────────

/// Run the first-run wizard.
///
/// Guides the user through provider detection and configuration. If the user
/// chooses "Quick start", all detected providers are configured immediately
/// without further prompts.
pub async fn run_wizard(
    config: &Config,
    store: &dyn ConfigStore,
) -> anyhow::Result<()> {
    let auth = crate::auth::store::ChainedAuthStore::new(config.config_dir.clone());

    print_banner();

    // Phase 1: Detect everything
    println!("🔍 Détection des providers...\n");
    let mut providers = detect::detect_all_providers();
    detect::validate_detected_providers(&mut providers).await;

    let ready = detect::ready_providers(&providers);
    let free = detect::free_providers(&providers);
    let gh_available = detect::gh_cli_installed();

    // Phase 2: Show what we found
    show_detection_summary(&providers, &ready, &free, gh_available);

    // Phase 3: Offer quick start
    println!();
    println!("1. ⚡ Démarrage rapide — utilise tout ce qui est détecté");
    println!("2. 🧭 Assistant pas à pas — je choisis ce que je veux configurer");
    println!("3. 🚪 Quitter sans configurer");
    print!("\nChoix [1]: ");
    io::stdout().flush().ok();

    let mut choice = String::new();
    io::stdin().read_line(&mut choice)?;
    let choice = choice.trim();

    match choice {
        "3" => {
            println!("\n👋 Pas de souci ! Tu pourras configurer plus tard avec : sparrow setup");
            return Ok(());
        }
        "2" => {
            step_by_step(config, store, &providers, gh_available).await?;
        }
        _ => {
            // Quick start (default)
            quick_start(config, store, &ready, gh_available).await?;
        }
    }

    Ok(())
}

// ─── Banner ──────────────────────────────────────────────────────────────────

fn print_banner() {
    println!("══════════════════════════════════════════════════");
    println!("  🐦 Sparrow — Assistant de configuration");
    println!("  version {}", env!("CARGO_PKG_VERSION"));
    println!("══════════════════════════════════════════════════");
    println!();
    println!("On va détecter tes clés API et configurer Sparrow");
    println!("en quelques secondes. C'est gratuit et open-source !");
    println!();
}

// ─── Detection summary ───────────────────────────────────────────────────────

fn show_detection_summary(
    all: &[DetectedProvider],
    ready: &[&DetectedProvider],
    free: &[&DetectedProvider],
    gh_available: bool,
) {
    if !ready.is_empty() {
        println!("✅ Providers prêts à l'emploi :");
        for p in ready {
            let env_hint = p
                .env_var
                .as_ref()
                .map(|e| format!(" ({e})"))
                .unwrap_or_default();
            println!("   • {} — clé détectée et validée{}", p.label, env_hint);
        }
    } else {
        println!("⚠️  Aucune clé API trouvée dans ton environnement.");
    }

    let need_key: Vec<_> = all
        .iter()
        .filter(|p| p.key_found && p.validated == Some(false))
        .collect();
    if !need_key.is_empty() {
        println!("\n⚠️  Clés trouvées mais invalides :");
        for p in &need_key {
            println!("   • {} — {}", p.label, p.validation_error.as_deref().unwrap_or("?"));
        }
    }

    if !free.is_empty() && ready.is_empty() {
        println!("\n🎁 Providers GRATUITS disponibles :");
        for p in free {
            println!("   • {} — {}", p.label, p.description);
            if let Some(url) = &p.signup_url {
                println!("     → Crée une clé : {url}");
            }
            if let Some(env) = &p.env_var {
                println!("     → Puis exporte-la : export {}=\"ta-clé\"", env);
            }
        }
    }

    if gh_available {
        println!("\n🐙 GitHub CLI détecté (gh) — GitHub Copilot dispo !");
        println!("   → Connecte-toi avec : gh auth login");
        println!("   → Puis : gh copilot suggest \"hello\"");
    }

    // Show un-configured providers count
    let not_configured = all
        .iter()
        .filter(|p| !p.key_found)
        .count();
    if not_configured > 0 {
        println!(
            "\n📋 {} autres providers dispos (payants ou nécessitent une clé).",
            not_configured
        );
    }
}

// ─── Quick start ─────────────────────────────────────────────────────────────

async fn quick_start(
    config: &Config,
    store: &dyn ConfigStore,
    ready: &[&DetectedProvider],
    gh_available: bool,
) -> anyhow::Result<()> {
    println!("\n⚡ Démarrage rapide — configuration automatique...\n");

    let auth = crate::auth::store::ChainedAuthStore::new(config.config_dir.clone());
    let mut updated = config.clone();

    for p in ready {
        add_provider_to_config(&mut updated, p);
        // Store credential from env var
        if let Some(env_var) = &p.env_var {
            if let Ok(key) = std::env::var(env_var) {
                if !key.trim().is_empty() {
                    auth.set(&p.id, Credential::api_key(key))?;
                    println!("   ✓ {:<25} configuré (modèle par défaut)", p.label);
                }
            }
        }
    }

    // If nothing was configured, suggest free providers
    if ready.is_empty() {
        println!("   ⚠️  Aucun provider prêt. Voici des options gratuites :\n");
        println!("   • NVIDIA NIM — gratuit, crée une clé : https://build.nvidia.com/explore/discover");
        println!("     puis : export NVIDIA_API_KEY=\"ta-clé\" && sparrow setup");
        println!("   • Groq — tier gratuit généreux : https://console.groq.com/keys");
        println!("     puis : export GROQ_API_KEY=\"ta-clé\" && sparrow setup");
        println!("   • Google Gemini — gratuit 1500 req/j : https://aistudio.google.com/app/apikey");
        println!("     puis : export GEMINI_API_KEY=\"ta-clé\" && sparrow setup\n");
    }

    if gh_available {
        println!("   💡 GitHub Copilot détecté. Pour l'activer :");
        println!("      sparrow auth add copilot");
    }

    store.save(&updated)?;
    println!("\n✅ Configuration sauvegardée !");
    println!("   Lance sparrow pour démarrer.\n");

    Ok(())
}

// ─── Step-by-step assistant ──────────────────────────────────────────────────

async fn step_by_step(
    config: &Config,
    store: &dyn ConfigStore,
    providers: &[DetectedProvider],
    gh_available: bool,
) -> anyhow::Result<()> {
    println!("\n🧭 Assistant pas à pas — je te guide.\n");

    let auth = crate::auth::store::ChainedAuthStore::new(config.config_dir.clone());
    let mut updated = config.clone();

    // Group providers by tier for display
    let mut free_list: Vec<&DetectedProvider> = Vec::new();
    let mut ready_list: Vec<&DetectedProvider> = Vec::new();
    let mut need_key_list: Vec<&DetectedProvider> = Vec::new();

    for p in providers {
        if p.key_found && p.validated == Some(true) {
            ready_list.push(p);
        } else if p.tier == ProviderTier::Free {
            free_list.push(p);
        } else if p.key_found {
            need_key_list.push(p);
        }
    }

    // First: offer to configure ready providers
    if !ready_list.is_empty() {
        println!("── Providers prêts à l'emploi ──");
        for (i, p) in ready_list.iter().enumerate() {
            println!("  [{i}] {} — clé validée ✓", p.label);
        }
        println!("  [A] Tous les activer");
        println!("  [N] Aucun, passer à la suite");
        print!("\nChoix [A]: ");
        io::stdout().flush().ok();

        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        let answer = answer.trim().to_lowercase();

        if answer == "n" {
            // skip
        } else if answer == "a" || answer.is_empty() {
            for p in &ready_list {
                add_provider_to_config(&mut updated, p);
                if let Some(env_var) = &p.env_var {
                    if let Ok(key) = std::env::var(env_var) {
                        auth.set(&p.id, Credential::api_key(key))?;
                    }
                }
                println!("   ✓ {:<25} configuré", p.label);
            }
        } else if let Ok(idx) = answer.parse::<usize>() {
            if idx < ready_list.len() {
                let p = ready_list[idx];
                add_provider_to_config(&mut updated, p);
                if let Some(env_var) = &p.env_var {
                    if let Ok(key) = std::env::var(env_var) {
                        auth.set(&p.id, Credential::api_key(key))?;
                    }
                }
                println!("   ✓ {} configuré", p.label);
            }
        }
    }

    // Second: offer free providers
    if !free_list.is_empty() && ready_list.is_empty() {
        println!("\n── Providers gratuits (nécessitent une clé) ──");
        for (i, p) in free_list.iter().enumerate() {
            println!("  [{i}] {} — gratuit !", p.label);
            if let Some(url) = &p.signup_url {
                println!("       → Inscription : {url}");
            }
        }
        println!("  [N] Passer");
        print!("\nLequel t'intéresse ? (numéro ou N) [N]: ");
        io::stdout().flush().ok();

        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        let answer = answer.trim().to_lowercase();

        if answer != "n" && !answer.is_empty() {
            if let Ok(idx) = answer.parse::<usize>() {
                if idx < free_list.len() {
                    let p = free_list[idx];
                    handle_provider_key_prompt(&mut updated, p, &auth).await?;
                }
            }
        }
    }

    // Third: offer GitHub Copilot
    if gh_available {
        println!("\n🐙 GitHub Copilot détecté (gh CLI installé).");
        print!("Activer GitHub Copilot ? [O/n] ");
        io::stdout().flush().ok();

        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        if !matches!(answer.trim().to_lowercase().as_str(), "n" | "no" | "non") {
            println!("   → Pour utiliser Copilot, assure-toi d'être connecté : gh auth login");
            updated.providers.entry("copilot".into()).or_insert_with(|| {
                ProviderConfig {
                    adapter: "openai-compatible".into(),
                    base_url: Some("https://api.githubcopilot.com".into()),
                    models: vec!["gpt-4o".into()],
                    api_key_env: Some("COPILOT_TOKEN".into()),
                }
            });
            println!("   ✓ GitHub Copilot ajouté à la config.");
        }
    }

    // Fourth: offer custom/other providers
    println!("\n── Autres providers ──");
    println!("Tu peux aussi configurer un provider manuellement :");
    println!("  → sparrow auth add <provider>");
    println!("  → Ou exporte la clé : export PROVIDER_API_KEY=\"ta-clé\"");
    println!("\nProviders supportés :");
    for chunk in crate::config::providers::provider_registry().chunks(5) {
        let ids: Vec<&str> = chunk.iter().map(|d| d.id.as_str()).collect();
        println!("  {}", ids.join(", "));
    }

    store.save(&updated)?;
    println!("\n✅ Configuration sauvegardée !");
    println!("   Tu peux la modifier à tout moment : sparrow config --edit\n");

    Ok(())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Prompt the user for an API key, validate it, and store it.
async fn handle_provider_key_prompt(
    config: &mut Config,
    provider: &DetectedProvider,
    auth: &crate::auth::store::ChainedAuthStore,
) -> anyhow::Result<()> {
    let env_var_name = provider
        .env_var
        .as_deref()
        .unwrap_or(&provider.id);

    println!();
    println!("Pour utiliser {}, il te faut une clé API.", provider.label);
    if let Some(url) = &provider.signup_url {
        println!("→ Crée une clé ici : {url}");
    }
    println!("→ Puis colle-la ci-dessous (ou laisse vide pour passer).");
    print!("Clé API ({}): ", env_var_name);
    io::stdout().flush().ok();

    let key = rpassword::read_password().unwrap_or_default();
    let key = key.trim().to_string();

    if key.is_empty() {
        println!("   ↳ Passé.");
        return Ok(());
    }

    // Validate the key
    print!("   Validation de la clé... ");
    io::stdout().flush().ok();
    match detect::validate_api_key(&provider.id, &key).await {
        Ok(()) => {
            println!("✓");
            auth.set(&provider.id, Credential::api_key(&key))?;
            // Also set env var for this session
            std::env::set_var(env_var_name, &key);
            add_provider_to_config(config, provider);
            println!("   ✓ {} configuré avec succès !", provider.label);
        }
        Err(err) => {
            println!("✗");
            println!("   Échec de validation : {err}");
            print!("   Ajouter quand même ? [o/N] ");
            io::stdout().flush().ok();
            let mut answer = String::new();
            io::stdin().read_line(&mut answer)?;
            if matches!(answer.trim().to_lowercase().as_str(), "o" | "oui" | "y" | "yes") {
                auth.set(&provider.id, Credential::api_key(&key))?;
                std::env::set_var(env_var_name, &key);
                add_provider_to_config(config, provider);
                println!("   ✓ {} ajouté (clé non validée).", provider.label);
            }
        }
    }

    Ok(())
}

/// Add a detected provider to the config (with its default models).
fn add_provider_to_config(config: &mut Config, provider: &DetectedProvider) {
    let def = crate::config::providers::find_provider(&provider.id);

    let entry = config.providers.entry(provider.id.clone()).or_insert_with(|| {
        ProviderConfig {
            adapter: def
                .as_ref()
                .map(|d| d.adapter.clone())
                .unwrap_or_else(|| "openai-compatible".into()),
            base_url: def.as_ref().map(|d| Some(d.base_url.clone())).unwrap_or(None),
            models: def
                .as_ref()
                .map(|d| {
                    crate::config::providers::default_models(&d.id)
                })
                .unwrap_or_default(),
            api_key_env: provider.env_var.clone(),
        }
    });

    // Ensure the adapter and base_url are correct even for existing entries
    if let Some(d) = &def {
        if entry.adapter.is_empty() || entry.adapter == "openai-compatible" {
            entry.adapter = d.adapter.clone();
        }
        if entry.base_url.is_none() {
            entry.base_url = Some(d.base_url.clone());
        }
        if entry.api_key_env.is_none() {
            entry.api_key_env = d.api_key_env.clone();
        }
        if entry.models.is_empty() {
            entry.models = crate::config::providers::default_models(&d.id);
        }
    }
}
