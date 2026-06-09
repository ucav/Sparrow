#![allow(
    clippy::collapsible_if,
    clippy::format_in_format_args,
    clippy::manual_clamp,
    clippy::needless_borrow,
    clippy::new_without_default,
    clippy::ptr_arg,
    clippy::type_complexity,
    clippy::useless_format
)]

use clap::Parser;
use sparrow::agent::{AgentStore, FsAgentStore, Soul};
use sparrow::auth::{AuthStore, Credential};
use sparrow::autonomy::{Checkpoints, GitCheckpoints};
use sparrow::capabilities::mcp::{BasicMcpClient, McpClient, McpServer, Transport};
use sparrow::capabilities::{FsSkillLibrary, SkillLibrary};
use sparrow::cli::{Cli, Commands};
use sparrow::config::{ConfigStore, FsConfigStore, ProviderConfig};
use sparrow::console::WebViewServer;
use sparrow::extras::{ChatSession, ReExecuter};
use sparrow::gateway::discord::DiscordTransport;
use sparrow::gateway::slack::SlackTransport;
use sparrow::gateway::telegram::TelegramTransport;
use sparrow::gateway::ws::WebSocketApi;
use sparrow::gateway::{GatewayMessage, GatewayResponse, GatewayTransport, MessageRouter};
use sparrow::memory::{Memory, SqliteMemory};
use sparrow::runtime::event_bus::EventBus;
use sparrow::runtime::recorder::{FsRecorder, Recorder, Replayer, RunInputs};
use sparrow::runtime::scheduler::{Job, MemoryScheduler, Scheduler};
use sparrow::runtime::{Runtime, SparrowRuntime};
use sparrow::tui::Tui;
use std::io::Write;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Quiet by default so structured logs (e.g. "Transcript saved") never
    // interleave with the user-facing answer on stdout. Logs go to stderr;
    // set RUST_LOG=sparrow=info (or debug) for verbose diagnostics.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sparrow=warn".into()),
        )
        .init();

    let cli = Cli::parse();

    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("sparrow");
    // state_dir: dirs::state_dir() is None on Windows/macOS — fall back to the
    // platform's local-data dir so the DB and transcripts never land in the CWD.
    let state_dir = dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .or_else(dirs::data_dir)
        .unwrap_or_else(|| {
            dirs::home_dir()
                .map(|h| h.join(".local").join("state"))
                .unwrap_or_else(|| std::path::PathBuf::from("."))
        })
        .join("sparrow");

    let active_profile = cli.profile.clone().or_else(|| {
        std::fs::read_to_string(config_dir.join("active_profile"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    });
    let active_config_dir = active_profile
        .as_ref()
        .map(|name| config_dir.join("profiles").join(name))
        .unwrap_or_else(|| config_dir.clone());

    // Profile state isolation: each profile gets its own state dir (db, transcripts).
    // The global state_dir is still used for gateway.pid and the profiles/ tree itself.
    let active_state_dir = active_profile
        .as_ref()
        .map(|name| {
            let p = state_dir.join("profiles").join(name);
            std::fs::create_dir_all(&p).ok();
            p
        })
        .unwrap_or_else(|| state_dir.clone());

    let config_store = FsConfigStore::new(active_config_dir.clone());
    let mut config = config_store.load().unwrap_or_else(|e| {
        eprintln!("Warning: could not load config: {}. Using defaults.", e);
        sparrow::config::Config {
            defaults: Default::default(),
            routing: Default::default(),
            budget: Default::default(),
            providers: Default::default(),
            surfaces: Default::default(),
            skills: Default::default(),
            permissions: Default::default(),
            hooks: Default::default(),
            theme: "captain".into(),
            config_dir: active_config_dir.clone(),
            state_dir: active_state_dir.clone(),
            forced_model: None,
        }
    });
    config.config_dir = active_config_dir.clone();
    config.state_dir = active_state_dir.clone();
    migrate_inline_provider_keys(&mut config, &config_store);
    apply_cli_overrides(&mut config, &cli);

    // Initialize memory (SQLite) — isolated per profile via active_state_dir
    let memory = Arc::new(
        SqliteMemory::open(&active_state_dir.join("sparrow.db")).unwrap_or_else(|e| {
            eprintln!(
                "Warning: could not open database: {}. Using in-memory fallback.",
                e
            );
            // In-memory fallback
            SqliteMemory::open(&std::path::PathBuf::from(":memory:")).unwrap()
        }),
    );
    // ── Boot-time model discovery ─────────────────────────────────────────────
    // For every provider with an environment key or stored credential but an
    // empty model cache, kick off a silent background discovery so `sparrow
    // model --list` and the router see the full catalogue on first run.
    {
        let memory_for_discovery: Arc<dyn Memory> = memory.clone();
        let auth_for_discovery =
            sparrow::auth::store::ChainedAuthStore::new(config.config_dir.clone());

        // 1. Ollama — always try (no key needed)
        if memory_for_discovery
            .get_discovered_models("ollama")
            .is_empty()
        {
            let ollama_base_url =
                std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434/v1".into());
            let m = memory_for_discovery.clone();
            tokio::spawn(async move {
                discover_and_cache_provider(
                    m,
                    "ollama".to_string(),
                    "ollama".to_string(),
                    ollama_base_url,
                    String::new(),
                    false,
                )
                .await;
            });
        }

        // 2. Every other provider with an env key or stored credential but empty cache
        for def in sparrow::config::providers::provider_registry() {
            if def.adapter == "ollama" {
                continue; // handled above
            }
            let api_key = def
                .api_key_env
                .as_ref()
                .and_then(|env_var| std::env::var(env_var).ok())
                .or_else(|| {
                    auth_for_discovery
                        .get(&def.id)
                        .and_then(|credential| credential.expose_key().map(str::to_string))
                });
            let Some(api_key) = api_key else {
                continue;
            };
            let api_key = api_key.trim().to_string();
            if api_key.is_empty() {
                continue;
            }
            if !memory_for_discovery
                .get_discovered_models(&def.id)
                .is_empty()
            {
                continue; // already cached
            }
            let m = memory_for_discovery.clone();
            let pid = def.id.clone();
            let adapter = def.adapter.clone();
            let base_url = def.base_url.clone();
            tokio::spawn(async move {
                discover_and_cache_provider(m, pid, adapter, base_url, api_key, false).await;
            });
        }
    }

    // Initialize agent store
    let agent_store: Arc<dyn AgentStore> =
        Arc::new(FsAgentStore::new(config_dir.join("agents")).with_memory(memory.clone()));

    // Initialize skill library
    let skills_dir = config_dir.join("skills");
    let skill_library: Arc<dyn SkillLibrary> =
        Arc::new(FsSkillLibrary::new(skills_dir).with_memory(memory.clone()));

    // Initialize recorder (transcripts) — isolated per profile
    let recorder = Arc::new(FsRecorder::new(active_state_dir.join("transcripts")));

    // Initialize scheduler
    let scheduler = Arc::new(MemoryScheduler::new().with_memory(memory.clone()));

    // ── First-launch detection (§16) ─────────────────────────────────
    // If no config.toml exists yet, run the conversational Setup Agent
    // before launching the TUI — so the user is greeted with onboarding,
    // not a blank cockpit with no providers.
    let is_first_launch = !active_config_dir.join("config.toml").exists();
    if is_first_launch && cli.command.is_none() {
        println!("First launch detected — running setup...\n");
        let setup_result = sparrow::onboarding::setup_agent::run_setup_agent(
            &config,
            &config_store,
            memory.clone(),
            build_provider_brains,
        )
        .await;
        if let Err(err) = setup_result {
            eprintln!("Setup Agent: {} — falling back to interactive setup.", err);
            handle_setup(&config, &config_store).await?;
        }
        // Reload config after setup wrote it
        if let Ok(fresh) = config_store.load() {
            config = fresh;
            config.config_dir = active_config_dir.clone();
            config.state_dir = active_state_dir.clone();
        }
    }

    match cli.command {
        None => {
            if cli.tui {
                run_tui(
                    &config,
                    memory.clone(),
                    skill_library.clone(),
                    &active_state_dir,
                )
                .await?;
            } else if cli.web {
                handle_webview(
                    &config,
                    memory.clone(),
                    scheduler.clone(),
                    recorder.clone(),
                    skill_library.clone(),
                    Some(agent_store.clone()),
                    9339,
                )
                .await?;
            } else {
                run_tui(
                    &config,
                    memory.clone(),
                    skill_library.clone(),
                    &active_state_dir,
                )
                .await?;
            }
        }
        Some(Commands::Tui) => {
            run_tui(
                &config,
                memory.clone(),
                skill_library.clone(),
                &active_state_dir,
            )
            .await?;
        }
        Some(Commands::Launch { port, tui }) => {
            if !active_config_dir.join("config.toml").exists() {
                println!("First launch detected - running setup...\n");
                let setup_result = sparrow::onboarding::setup_agent::run_setup_agent(
                    &config,
                    &config_store,
                    memory.clone(),
                    build_provider_brains,
                )
                .await;
                if let Err(err) = setup_result {
                    eprintln!("Setup Agent: {} - falling back to interactive setup.", err);
                    handle_setup(&config, &config_store).await?;
                }
                if let Ok(fresh) = config_store.load() {
                    config = fresh;
                    config.config_dir = active_config_dir.clone();
                    config.state_dir = active_state_dir.clone();
                }
            }

            if tui {
                run_tui(
                    &config,
                    memory.clone(),
                    skill_library.clone(),
                    &active_state_dir,
                )
                .await?;
            } else {
                handle_webview(
                    &config,
                    memory.clone(),
                    scheduler.clone(),
                    recorder.clone(),
                    skill_library.clone(),
                    Some(agent_store.clone()),
                    port,
                )
                .await?;
            }
        }
        Some(Commands::Console { port }) => {
            handle_webview(
                &config,
                memory.clone(),
                scheduler.clone(),
                recorder.clone(),
                skill_library.clone(),
                Some(agent_store.clone()),
                port,
            )
            .await?;
        }
        Some(Commands::Daemon) => {
            handle_daemon(
                &config,
                memory.clone(),
                scheduler.clone(),
                recorder.clone(),
                skill_library.clone(),
            )
            .await?;
        }
        Some(Commands::Run { ref task, json }) => {
            if cli.json || json {
                // NDJSON mode: output each event as a JSON line
                run_task_json(
                    task,
                    &config,
                    memory.clone(),
                    recorder.clone(),
                    skill_library.clone(),
                )
                .await?;
            } else {
                run_task(
                    task,
                    &config,
                    memory.clone(),
                    skill_library.clone(),
                    recorder.clone(),
                    None,
                )
                .await?;
            }
        }
        Some(Commands::Plan { ref task, json }) => {
            handle_plan(task, &config, skill_library.clone(), json || cli.json)?;
        }
        Some(Commands::Permissions { action }) => {
            handle_permissions(action, &config, &config_store)?;
        }
        Some(Commands::Chat) => {
            handle_chat(&config, memory.clone()).await?;
        }
        Some(Commands::Agent { action }) => {
            handle_agent(
                action,
                &agent_store,
                &config,
                memory.clone(),
                skill_library.clone(),
                recorder.clone(),
            )
            .await?;
        }
        Some(Commands::Swarm { task }) => {
            run_swarm(&task, &config, memory.clone()).await?;
        }
        Some(Commands::Skills { action }) => {
            handle_skills(action, &skill_library)?;
        }
        Some(Commands::Plugins { action }) => {
            handle_plugins(action, &config_dir)?;
        }
        Some(Commands::Tools { action }) => {
            handle_tools(action, &config_store)?;
        }
        Some(Commands::Security { action }) => {
            handle_security(action, &config)?;
        }
        Some(Commands::Github { action }) => {
            handle_github(action)?;
        }
        Some(Commands::Compact { task, out, json }) => {
            handle_compact(task, out, json)?;
        }
        Some(Commands::Mcp { action }) => {
            handle_mcp(action, &config_dir).await?;
        }
        Some(Commands::Schedule {
            task,
            cron,
            autonomy,
            report,
        }) => {
            handle_schedule(&task, &cron, autonomy, &report, &scheduler).await?;
        }
        Some(Commands::Replay { run_id, scrub }) => {
            if scrub {
                match recorder.load(&run_id) {
                    Some(transcript) => {
                        let mut tui = Tui::new().with_replay(transcript.events);
                        tokio::task::spawn_blocking(move || tui.run()).await??;
                    }
                    None => eprintln!("Transcript not found: {}", run_id),
                }
            } else {
                handle_replay(&run_id, &recorder, &config, memory.clone()).await?;
            }
        }
        Some(Commands::Gateway { action }) => {
            handle_gateway(
                action,
                &state_dir,
                &config,
                memory.clone(),
                scheduler.clone(),
                recorder.clone(),
            )
            .await?;
        }
        Some(Commands::Sessions { action }) => {
            handle_sessions(action, &active_state_dir)?;
        }
        Some(Commands::Model { set, list }) => {
            if list {
                refresh_discovery_cache(memory.clone(), &config, false, false).await;
                println!("Configured providers:");
                let effective = effective_provider_configs(&config);
                for (name, pconfig) in &effective {
                    println!("  {} (adapter: {})", name, pconfig.adapter);
                    for model in &pconfig.models {
                        println!("    - {}", model);
                    }
                }
                if effective.is_empty() {
                    println!("  No providers configured.");
                    println!("  Run 'sparrow auth add <provider>' or set *_API_KEY env vars.");
                }
                println!("\nDiscovered models (from API, cached 24h):");
                let mut any_discovered = false;
                for def in sparrow::config::providers::provider_registry() {
                    let discovered: Vec<String> = memory
                        .get_discovered_models(&def.id)
                        .into_iter()
                        .filter(|model| sparrow::provider::discovery::is_chat_model_id(model))
                        .collect();
                    let static_names: std::collections::HashSet<String> =
                        sparrow::config::providers::default_models(&def.id)
                            .into_iter()
                            .collect();
                    let extra: Vec<_> = discovered
                        .iter()
                        .filter(|model| !static_names.contains(*model))
                        .collect();
                    if extra.is_empty() {
                        continue;
                    }
                    any_discovered = true;
                    println!("  {} (+{} discovered):", def.id, extra.len());
                    for model in extra.iter().take(10) {
                        println!("    - {}", model);
                    }
                    if extra.len() > 10 {
                        println!("    ... and {} more", extra.len() - 10);
                    }
                }
                if !any_discovered {
                    println!("  No extra discovered models cached yet.");
                }
            }
            if let Some(route) = set {
                // Parse "provider" or "provider:model"
                let (provider_id, model_opt) = if let Some(pos) = route.find(':') {
                    let (p, m) = route.split_at(pos);
                    (p.trim().to_string(), Some(m[1..].trim().to_string()))
                } else {
                    (route.trim().to_string(), None)
                };

                // Validate provider exists (static registry or discovered)
                let provider_known = sparrow::config::providers::find_provider(&provider_id)
                    .is_some()
                    || !memory.get_discovered_models(&provider_id).is_empty();
                if !provider_known {
                    eprintln!(
                        "Unknown provider '{}'. Run 'sparrow model --list' to see options.",
                        provider_id
                    );
                } else {
                    // Validate model if specified
                    if let Some(ref model) = model_opt {
                        let static_models =
                            sparrow::config::providers::default_models(&provider_id);
                        let discovered = memory.get_discovered_models(&provider_id);
                        let all: Vec<&String> =
                            static_models.iter().chain(discovered.iter()).collect();
                        if !all.is_empty() && !all.contains(&model) {
                            eprintln!(
                                "Warning: model '{}' not found in provider '{}'. \
                                 Run 'sparrow model --list' to see available models.",
                                model, provider_id
                            );
                            // Proceed anyway — user may know a model not in our registry
                        }
                    }

                    // Write routing policy to config
                    let mut updated = config.clone();
                    updated
                        .routing
                        .policy
                        .insert("medium".into(), provider_id.clone());
                    updated
                        .routing
                        .policy
                        .insert("hard".into(), provider_id.clone());
                    if let Some(model) = model_opt {
                        let def = sparrow::config::providers::find_provider(&provider_id);
                        let entry =
                            updated
                                .providers
                                .entry(provider_id.clone())
                                .or_insert_with(|| ProviderConfig {
                                    adapter: def
                                        .as_ref()
                                        .map(|d| d.adapter.clone())
                                        .unwrap_or_else(|| "openai-compatible".into()),
                                    base_url: def
                                        .as_ref()
                                        .map(|d| Some(d.base_url.clone()))
                                        .unwrap_or(None),
                                    models: vec![],
                                    api_key_env: def.as_ref().and_then(|d| d.api_key_env.clone()),
                                });
                        // Refresh base_url + adapter from the registry even for an
                        // EXISTING entry — otherwise a stale/wrong base_url persists
                        // (the OpenCode `zen.opencode.ai` regression came from here).
                        if let Some(d) = &def {
                            entry.adapter = d.adapter.clone();
                            entry.base_url = Some(d.base_url.clone());
                            if entry.api_key_env.is_none() {
                                entry.api_key_env = d.api_key_env.clone();
                            }
                        }
                        entry.models = vec![model.clone()];
                        println!("Routing updated: medium/hard → {}:{}", provider_id, model);
                    } else {
                        if let Some(provider) = updated.providers.get_mut(&provider_id) {
                            let defaults = sparrow::config::providers::default_models(&provider_id);
                            if !defaults.is_empty() {
                                provider.models = defaults;
                            }
                        }
                        println!("Routing updated: medium/hard → {}", provider_id);
                    }
                    config_store.save(&updated)?;
                    println!("Config saved. Run 'sparrow model --list' to verify.");
                }
            }
        }
        Some(Commands::Route { action }) => {
            match action {
                sparrow::cli::RouteAction::Show => {
                    println!("Routing configuration:");
                    println!(
                        "  auto_discover : {}",
                        if config.routing.auto_discover {
                            "on"
                        } else {
                            "off"
                        }
                    );
                    match &config.routing.preferred_provider {
                        Some(p) => println!("  preferred_provider : {} (all tiers pinned)", p),
                        None => println!("  preferred_provider : (none — per-tier policy active)"),
                    }
                    println!("  Per-tier policy:");
                    let mut tiers: Vec<_> = config.routing.policy.iter().collect();
                    tiers.sort_by_key(|(k, _): &(&String, &String)| k.as_str());
                    for (tier, provider) in tiers {
                        println!("    {} -> {}", tier, provider);
                    }
                }
                sparrow::cli::RouteAction::Set { provider } => {
                    // Validate the provider is known
                    let known = sparrow::config::providers::find_provider(&provider).is_some()
                        || !memory.get_discovered_models(&provider).is_empty();
                    if !known {
                        eprintln!(
                            "Unknown provider '{}'. Run 'sparrow model --list' to see options.",
                            provider
                        );
                    } else {
                        let mut updated = config.clone();
                        updated.routing.preferred_provider = Some(provider.clone());
                        config_store.save(&updated)?;
                        println!("Auto-routing pinned to provider: {}", provider);
                        println!("All task tiers will prefer this provider.");
                        println!("Run 'sparrow route clear' to restore per-tier policy.");
                    }
                }
                sparrow::cli::RouteAction::Clear => {
                    let mut updated = config.clone();
                    updated.routing.preferred_provider = None;
                    config_store.save(&updated)?;
                    println!("Preferred provider cleared. Per-tier routing policy is now active.");
                }
            }
        }
        Some(Commands::Auth { action }) => {
            let auth = sparrow::auth::store::ChainedAuthStore::new(config.config_dir.clone());
            match action {
                sparrow::cli::AuthAction::List => {
                    let providers = auth.list();
                    if providers.is_empty() {
                        println!("No credentials stored.");
                        println!("Set env vars like ANTHROPIC_API_KEY, OPENAI_API_KEY, etc.");
                    } else {
                        println!("Stored credentials for:");
                        for p in providers {
                            println!("  - {}", p);
                        }
                    }
                }
                sparrow::cli::AuthAction::Add { provider } => {
                    let provider_def = sparrow::config::providers::onboarding_providers()
                        .into_iter()
                        .find(|p| p.id == provider || p.label.eq_ignore_ascii_case(&provider));
                    let (provider_id, label, env_var, adapter, base_url) = provider_def
                        .clone()
                        .map(|p| {
                            (
                                p.id,
                                p.label,
                                p.api_key_env.unwrap_or_else(|| {
                                    format!("{}_API_KEY", provider.to_uppercase())
                                }),
                                p.adapter,
                                p.base_url,
                            )
                        })
                        .unwrap_or_else(|| {
                            (
                                provider.clone(),
                                provider.clone(),
                                format!("{}_API_KEY", provider.to_uppercase()),
                                "openai-compatible".into(),
                                "https://api.openai.com/v1".into(),
                            )
                        });
                    println!("Add credentials for: {} ({})", label, provider_id);
                    println!("Paste API key for {}:", env_var);
                    let key = rpassword::read_password()
                        .or_else(|_| {
                            let mut key = String::new();
                            std::io::stdin().read_line(&mut key)?;
                            Ok::<_, std::io::Error>(key)
                        })?
                        .trim()
                        .to_string();
                    if key.is_empty() {
                        anyhow::bail!("Empty API key; nothing stored.");
                    }
                    let stored_key = key.clone();
                    auth.set(&provider_id, Credential::api_key(key))?;
                    println!("Stored credential for {}.", provider_id);
                    // Auto-discover models right after storing the key, unless
                    // the user explicitly disabled it in config.
                    if config.routing.auto_discover {
                        discover_and_cache_provider(
                            memory.clone(),
                            provider_id,
                            adapter,
                            base_url,
                            stored_key,
                            true,
                        )
                        .await;
                    }
                }
                sparrow::cli::AuthAction::Rm { provider } => {
                    auth.remove(&provider)?;
                    println!("Removed credentials for: {}", provider);
                }
                sparrow::cli::AuthAction::Login {
                    provider,
                    client_id,
                } => {
                    handle_auth_login(&provider, client_id, &auth).await?;
                }
            }
        }
        Some(Commands::Checkpoint { action }) => match action {
            sparrow::cli::CheckpointAction::List => {
                let cwd = std::env::current_dir().unwrap_or_default();
                let checkpoints = GitCheckpoints::new(cwd);
                let list = checkpoints.list();
                if list.is_empty() {
                    println!("No checkpoints found.");
                    println!("Checkpoints are created automatically before mutating actions.");
                } else {
                    println!("Checkpoints:");
                    for cp in &list {
                        println!("  {}  {}", cp.id.0, cp.label);
                    }
                }
            }
            sparrow::cli::CheckpointAction::Diff { id } => {
                let cwd = std::env::current_dir().unwrap_or_default();
                let checkpoints = GitCheckpoints::new(cwd);
                match checkpoints.diff(&sparrow::event::CheckpointId(id.clone())) {
                    Ok(diff) if diff.trim().is_empty() => {
                        println!("No changes between checkpoint {} and HEAD.", id);
                    }
                    Ok(diff) => print!("{}", diff),
                    Err(e) => eprintln!("Failed to diff checkpoint {}: {}", id, e),
                }
            }
            sparrow::cli::CheckpointAction::Prune { older_than_days } => {
                let cwd = std::env::current_dir().unwrap_or_default();
                let checkpoints = GitCheckpoints::new(cwd);
                match checkpoints.prune(older_than_days) {
                    Ok(0) => println!(
                        "No checkpoints older than {} days to prune.",
                        older_than_days
                    ),
                    Ok(n) => println!(
                        "Pruned {} checkpoint(s) older than {} days.",
                        n, older_than_days
                    ),
                    Err(e) => eprintln!("Failed to prune checkpoints: {}", e),
                }
            }
        },
        Some(Commands::Rewind { id }) => {
            // Safety: `git reset --hard` discards uncommitted changes. Ask for confirmation.
            eprint!(
                "⚠ Rewind to checkpoint `{}`? This will `git reset --hard` [y/N] ",
                id
            );
            let _ = std::io::stdout().flush();
            let mut input = String::new();
            if std::io::stdin().read_line(&mut input).is_ok()
                && input.trim().eq_ignore_ascii_case("y")
            {
                let cwd = std::env::current_dir().unwrap_or_default();
                let checkpoints = GitCheckpoints::new(cwd);
                match checkpoints.rewind(sparrow::event::CheckpointId(id.clone())) {
                    Ok(()) => println!("Rewound to checkpoint: {}", id),
                    Err(e) => eprintln!("Failed to rewind: {}", e),
                }
            } else {
                println!("Rewind cancelled.");
            }
        }
        Some(Commands::Doctor) => {
            // Show boot logo
            for line in sparrow::tui::theme::boot_sequence() {
                println!("{}", line);
            }
            println!();

            println!("Sparrow Diagnostics");
            println!("===================");
            println!("Config dir : {:?}", config.config_dir);
            println!("State dir  : {:?}", config.state_dir);
            println!("Theme      : {}", config.theme);
            println!("Autonomy   : {:?}", config.defaults.autonomy);
            // Sandbox line is printed (platform-aware) further down.
            println!(
                "Budget     : ${}/day, ${}/session",
                config.budget.daily_usd, config.budget.session_usd
            );
            println!();

            let auth = sparrow::auth::store::ChainedAuthStore::new(config.config_dir.clone());
            let stored = auth.list();
            println!("Credentials: {} stored", stored.len());
            for p in &stored {
                println!("  - {}", p);
            }

            let git_ok = std::process::Command::new("git")
                .arg("--version")
                .output()
                .is_ok();
            println!(
                "Git        : {}",
                if git_ok { "available" } else { "not found" }
            );

            // Sandbox reality check
            {
                let sandbox = &config.defaults.sandbox;
                #[cfg(not(target_os = "linux"))]
                {
                    if sandbox == "local-hardened" {
                        println!(
                            "Sandbox    : {} (note: namespace/seccomp isolation is Linux-only; \
                             running with path-boundary enforcement only on this platform)",
                            sandbox
                        );
                    } else {
                        println!("Sandbox    : {}", sandbox);
                    }
                }
                #[cfg(target_os = "linux")]
                println!("Sandbox    : {} (firejail/bwrap/unshare)", sandbox);
            }

            let facts = memory.all_facts();
            println!("Memory     : {} facts stored", facts.len());
            let agents = agent_store.list();
            println!("Agents     : {} defined", agents.len());
            for a in &agents {
                println!("  - {} ({})", a.name, a.role);
            }
            let skills = skill_library.all();
            println!("Skills     : {} in library", skills.len());
            let static_models: usize = sparrow::config::providers::provider_registry()
                .iter()
                .map(|provider| provider.models.len())
                .sum();
            let total_discovered: usize = sparrow::config::providers::provider_registry()
                .iter()
                .map(|provider| {
                    memory
                        .get_discovered_models(&provider.id)
                        .into_iter()
                        .filter(|model| sparrow::provider::discovery::is_chat_model_id(model))
                        .count()
                })
                .sum();
            println!(
                "Models     : {} static + {} discovered (cached 24h)",
                static_models, total_discovered
            );
            let transcripts = recorder.list_transcripts();
            println!("Transcripts: {} recorded", transcripts.len());
            let jobs = scheduler.list();
            println!("Sched. jobs: {} scheduled", jobs.len());

            // Check for updates
            if let Ok(Some(update)) =
                tokio::task::spawn_blocking(sparrow::update::check_update).await
            {
                println!("\nUpdate    : {} (run 'sparrow update')", update);
            }

            println!();
            println!("M6 Polish — v1 ready.");
        }
        Some(Commands::Update) => {
            println!("Checking for updates...");
            match sparrow::update::self_update() {
                Ok(msg) => println!("{}", msg),
                Err(e) => eprintln!("Update failed: {}", e),
            }
        }
        Some(Commands::Profile { action }) => {
            handle_profile(action, &config_dir, &state_dir)?;
        }
        Some(Commands::Import { source }) => {
            handle_full_import(source)?;
        }
        Some(Commands::Setup) => {
            // Conversational Setup Agent (§16). Falls back to the legacy
            // interactive flow if no Brain is reachable at all.
            let result = sparrow::onboarding::setup_agent::run_setup_agent(
                &config,
                &config_store,
                memory.clone(),
                build_provider_brains,
            )
            .await;
            if let Err(err) = result {
                eprintln!(
                    "Setup Agent failed: {}\n→ falling back to the legacy interactive flow.",
                    err
                );
                handle_setup(&config, &config_store).await?;
            }
        }
        Some(Commands::Demo) => {
            sparrow::demo::run_demo(None).await?;
        }
        Some(Commands::Share) => {
            sparrow::share::run_share(&state_dir, false).await?;
        }
        Some(Commands::Hook { action }) => match action {
            sparrow::cli::HookAction::Install => {
                sparrow::hook_cmd::run_hook_install()?;
            }
            sparrow::cli::HookAction::Scan { all } => {
                sparrow::hook_cmd::run_hook_scan(all)?;
            }
        },
        Some(Commands::Learn) => {
            sparrow::onboarding::Onboarding::default().run_interactive()?;
        }
        Some(Commands::Voice { action }) => match action {
            sparrow::cli::VoiceAction::Speak { text } => {
                sparrow::tools::voice::handle_voice(sparrow::tools::voice::VoiceCommand::Speak {
                    text,
                    output: None,
                })?;
            }
            sparrow::cli::VoiceAction::Transcribe { file } => {
                sparrow::tools::voice::handle_voice(
                    sparrow::tools::voice::VoiceCommand::Transcribe {
                        audio_file: file.into(),
                        language: None,
                    },
                )?;
            }
            sparrow::cli::VoiceAction::Providers => {
                sparrow::tools::voice::handle_voice(
                    sparrow::tools::voice::VoiceCommand::ListProviders,
                )?;
            }
        },
        Some(Commands::Browser { url }) => {
            println!("🌐 Sparrow Browser — testing navigation to {}", url);
            println!("   Install deps first: bash scripts/setup-browser.sh\n");
            println!("   Usage in tasks:");
            println!("   sparrow run \"take a screenshot of {}\"", url);
            println!("   sparrow run \"extract the main content from {}\"", url);
        }
        Some(Commands::Init) => {
            handle_init()?;
        }
        Some(Commands::Status) => {
            handle_status(
                &(memory.clone() as Arc<dyn Memory>),
                &config,
                &scheduler,
                &recorder,
                &state_dir,
            )?;
        }
        Some(Commands::Memory { action }) => {
            handle_memory(
                action,
                &(memory.clone() as Arc<dyn Memory>),
                &active_state_dir,
            )?;
        }
        Some(Commands::Config { edit }) => {
            if edit {
                let config_path = config.config_dir.join("config.toml");
                println!("Config file: {}", config_path.display());
                #[cfg(windows)]
                {
                    let _ = std::process::Command::new("notepad")
                        .arg(&config_path)
                        .spawn();
                }
                #[cfg(not(windows))]
                {
                    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".into());
                    let _ = std::process::Command::new(editor).arg(&config_path).spawn();
                }
            }
        }
    }

    Ok(())
}

// ─── Agent commands ─────────────────────────────────────────────────────────────

async fn handle_agent(
    action: sparrow::cli::AgentAction,
    store: &Arc<dyn AgentStore>,
    config: &sparrow::config::Config,
    memory: Arc<dyn Memory>,
    skills: Arc<dyn SkillLibrary>,
    recorder: Arc<FsRecorder>,
) -> anyhow::Result<()> {
    match action {
        sparrow::cli::AgentAction::Create { name } => {
            let soul = Soul {
                name: name.clone(),
                ..Soul::default()
            };
            store.create(&soul)?;
            println!("Agent '{}' created.", name);
            println!("Edit: sparrow agent edit {}", name);
        }
        sparrow::cli::AgentAction::List => {
            let agents = store.list();
            if agents.is_empty() {
                println!("No agents defined.");
                println!("Create one with: sparrow agent create <name>");
            } else {
                println!("Defined agents:");
                for a in &agents {
                    println!(
                        "  {}  |  {}  |  {}  | tools: {} deny: {}",
                        a.name,
                        a.role,
                        if a.description.is_empty() {
                            a.personality.as_str()
                        } else {
                            a.description.as_str()
                        },
                        if a.tools.is_empty() {
                            "all".into()
                        } else {
                            a.tools.join(",")
                        },
                        if a.disallowed_tools.is_empty() {
                            "none".into()
                        } else {
                            a.disallowed_tools.join(",")
                        }
                    );
                }
            }
        }
        sparrow::cli::AgentAction::Edit { name } => {
            let path = dirs::config_dir()
                .unwrap_or_default()
                .join("sparrow")
                .join("agents")
                .join(format!("{}.soul.toml", name));
            if !path.exists() {
                anyhow::bail!("Agent '{}' not found. Create it first.", name);
            }
            println!("Edit agent file: {}", path.display());
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("notepad").arg(&path).spawn();
            }
            #[cfg(not(windows))]
            {
                let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".into());
                let _ = std::process::Command::new(editor).arg(&path).spawn();
            }
        }
        sparrow::cli::AgentAction::Rm { name } => {
            store.remove(&name)?;
            println!("Agent '{}' removed.", name);
        }
        sparrow::cli::AgentAction::Run { name, task } => {
            if let Some(soul) = store.get(&name) {
                println!("Running as agent '{}': {}", soul.name, task);
                run_task(&task, config, memory, skills, recorder, Some(soul)).await?;
            } else {
                anyhow::bail!("Agent '{}' not found.", name);
            }
        }
        sparrow::cli::AgentAction::Mention { name, message } => {
            if let Some(soul) = store.get(&name) {
                let task = format!("@{} {}", soul.name, message);
                println!("Mentioning agent '{}': {}", soul.name, message);
                run_task(&task, config, memory, skills, recorder, Some(soul)).await?;
            } else {
                anyhow::bail!("Agent '{}' not found.", name);
            }
        }
    }
    Ok(())
}

fn looks_like_inline_secret(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with("sk-")
        || trimmed.starts_with("nvapi-")
        || trimmed.starts_with("gsk_")
        || trimmed.starts_with("sk-or-")
}

fn apply_cli_overrides(config: &mut sparrow::config::Config, cli: &Cli) {
    if let Some(level) = cli.autonomy.as_deref() {
        let trimmed = level.trim().to_lowercase();
        // Accept named levels OR a float in [0.0, 1.0] — e.g. --autonomy 0.7
        config.defaults.autonomy = match trimmed.as_str() {
            "supervised" => sparrow::event::AutonomyLevel::Supervised,
            "trusted" => sparrow::event::AutonomyLevel::Trusted,
            "autonomous" => sparrow::event::AutonomyLevel::Autonomous,
            other => {
                if let Ok(f) = other.parse::<f64>() {
                    sparrow::event::AutonomyLevel::from_float(f.clamp(0.0, 1.0))
                } else {
                    config.defaults.autonomy.clone()
                }
            }
        };
        config.permissions.mode = match config.defaults.autonomy {
            sparrow::event::AutonomyLevel::Supervised => {
                sparrow::permissions::PermissionMode::Supervised
            }
            sparrow::event::AutonomyLevel::Trusted => sparrow::permissions::PermissionMode::Trusted,
            sparrow::event::AutonomyLevel::Autonomous => {
                sparrow::permissions::PermissionMode::Autonomous
            }
        };
    }
    if let Some(budget) = cli.budget {
        if budget > 0.0 {
            config.budget.session_usd = budget;
        }
    }
    if let Some(sandbox) = cli
        .sandbox
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        config.defaults.sandbox = sandbox.to_string();
    }
    if cli.local {
        config.routing.free_first = true;
        config
            .routing
            .policy
            .insert("trivial".into(), "ollama".into());
        config
            .routing
            .policy
            .insert("small".into(), "ollama".into());
        config
            .routing
            .policy
            .insert("medium".into(), "ollama".into());
        config.routing.policy.insert("hard".into(), "ollama".into());
        config
            .routing
            .policy
            .insert("vision".into(), "ollama".into());
    }
    if let Some(model_ref) = cli
        .model
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        let (provider, model) = model_ref
            .split_once(':')
            .map(|(p, m)| (p.trim().to_lowercase(), m.trim().to_string()))
            .unwrap_or_else(|| ("custom".into(), model_ref.to_string()));
        if !model.is_empty() {
            config.forced_model = Some((provider.clone(), model.clone()));
            config
                .routing
                .policy
                .insert("trivial".into(), provider.clone());
            config
                .routing
                .policy
                .insert("small".into(), provider.clone());
            config
                .routing
                .policy
                .insert("medium".into(), provider.clone());
            config
                .routing
                .policy
                .insert("hard".into(), provider.clone());
            config
                .routing
                .policy
                .insert("vision".into(), provider.clone());
            config
                .providers
                .entry(provider.clone())
                .or_insert_with(|| {
                    let def = sparrow::config::providers::find_provider(&provider);
                    ProviderConfig {
                        adapter: def
                            .as_ref()
                            .map(|d| d.adapter.clone())
                            .unwrap_or_else(|| "openai-compatible".into()),
                        base_url: def.as_ref().map(|d| d.base_url.clone()),
                        models: vec![],
                        api_key_env: def.and_then(|d| d.api_key_env),
                    }
                })
                .models = vec![model];
        }
    }
}

fn migrate_inline_provider_keys(config: &mut sparrow::config::Config, store: &FsConfigStore) {
    let auth = sparrow::auth::store::ChainedAuthStore::new(config.config_dir.clone());
    let mut changed = false;

    for (name, provider) in config.providers.iter_mut() {
        let Some(inline_key) = provider
            .api_key_env
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| looks_like_inline_secret(value))
        else {
            continue;
        };

        if let Err(err) = auth.set(name, Credential::api_key(inline_key)) {
            // Hard failure: do NOT keep the inline key in the config (it would
            // sit there in plaintext) but DO leave the field intact so the
            // operator can retry. Log loudly so the failure isn't silent.
            eprintln!(
                "warning: failed to migrate inline API key for provider '{}' into the credential \
                 store: {}. The inline key remains in config.toml — fix the store and re-run \
                 `sparrow setup` to migrate, or remove the key from config.toml manually.",
                name, err
            );
            tracing::warn!(
                provider = %name,
                error = %err,
                "inline api-key migration failed; key left in config"
            );
            continue;
        }

        provider.api_key_env =
            sparrow::config::providers::find_provider(name).and_then(|def| def.api_key_env);
        changed = true;
    }

    if changed {
        if let Err(err) = store.save(config) {
            eprintln!(
                "warning: migrated inline API keys into the credential store but FAILED to \
                 update config.toml: {}. Re-run `sparrow setup` to retry, or the inline keys \
                 may reappear on next start.",
                err
            );
            tracing::warn!(error = %err, "config save after key migration failed");
        }
    }
}

fn effective_provider_configs(
    config: &sparrow::config::Config,
) -> std::collections::HashMap<String, ProviderConfig> {
    let mut effective = config.providers.clone();
    let auth = sparrow::auth::store::ChainedAuthStore::new(config.config_dir.clone());

    for (name, pconfig) in effective.iter_mut() {
        if pconfig.models.is_empty() {
            pconfig.models = sparrow::config::providers::default_models(name);
        }
    }

    for def in sparrow::config::providers::provider_registry() {
        if effective.contains_key(&def.id) {
            continue;
        }

        let has_env_credential = def
            .api_key_env
            .as_ref()
            .map(|env| {
                if def.adapter == "ollama" {
                    true
                } else {
                    std::env::var(env)
                        .map(|value| !value.trim().is_empty())
                        .unwrap_or(false)
                }
            })
            .unwrap_or(def.adapter == "ollama");
        let has_stored_credential = auth.get(&def.id).is_some();

        if !has_env_credential && !has_stored_credential {
            continue;
        }

        let base_url = if def.adapter == "ollama" {
            std::env::var("OLLAMA_HOST")
                .ok()
                .or(Some(def.base_url.clone()))
        } else {
            Some(def.base_url.clone())
        };

        effective.insert(
            def.id.clone(),
            ProviderConfig {
                adapter: def.adapter,
                base_url,
                models: sparrow::config::providers::default_models(&def.id),
                api_key_env: def.api_key_env,
            },
        );
    }

    effective
}

async fn refresh_discovery_cache(
    memory: Arc<dyn Memory>,
    config: &sparrow::config::Config,
    force: bool,
    announce: bool,
) {
    let auth = sparrow::auth::store::ChainedAuthStore::new(config.config_dir.clone());

    if force || memory.get_discovered_models("ollama").is_empty() {
        let ollama_base_url =
            std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434/v1".into());
        discover_and_cache_provider(
            memory.clone(),
            "ollama".to_string(),
            "ollama".to_string(),
            ollama_base_url,
            String::new(),
            announce,
        )
        .await;
    }

    for def in sparrow::config::providers::provider_registry() {
        if def.adapter == "ollama" {
            continue;
        }
        if !force && !memory.get_discovered_models(&def.id).is_empty() {
            continue;
        }

        let api_key = def
            .api_key_env
            .as_ref()
            .and_then(|env_var| std::env::var(env_var).ok())
            .or_else(|| {
                auth.get(&def.id)
                    .and_then(|credential| credential.expose_key().map(str::to_string))
            })
            .map(|key| key.trim().to_string())
            .filter(|key| !key.is_empty());
        let Some(api_key) = api_key else {
            continue;
        };

        discover_and_cache_provider(
            memory.clone(),
            def.id,
            def.adapter,
            def.base_url,
            api_key,
            announce,
        )
        .await;
    }
}

async fn discover_and_cache_provider(
    memory: Arc<dyn Memory>,
    provider_id: String,
    adapter: String,
    base_url: String,
    api_key: String,
    announce: bool,
) {
    match sparrow::provider::discovery::discover_models(&adapter, &base_url, &api_key).await {
        Ok(models) if !models.is_empty() => {
            let count = models.len();
            if let Err(err) = memory.cache_discovered_models(&provider_id, &models) {
                if announce {
                    eprintln!(
                        "  Model discovery cache failed for {}: {}",
                        provider_id, err
                    );
                }
            } else if announce {
                println!("  {} models discovered for {}.", count, provider_id);
            }
        }
        Ok(_) => {}
        Err(err) => {
            if announce {
                eprintln!("  Model discovery skipped for {}: {}", provider_id, err);
            }
        }
    }
}

fn build_provider_brains(
    config: &sparrow::config::Config,
    memory: &Arc<dyn Memory>,
    warn: bool,
) -> std::collections::HashMap<String, Vec<Arc<dyn sparrow::provider::Brain>>> {
    let auth = sparrow::auth::store::ChainedAuthStore::new(config.config_dir.clone());
    let mut providers: std::collections::HashMap<String, Vec<Arc<dyn sparrow::provider::Brain>>> =
        std::collections::HashMap::new();

    for (name, pconfig) in effective_provider_configs(config) {
        // A forced model (--model provider:model) is exclusive: build only that
        // provider so the router can't fall back to a cheaper/free other provider.
        if let Some((forced_provider, _)) = &config.forced_model {
            if &name != forced_provider {
                continue;
            }
        }

        let api_key = pconfig
            .api_key_env
            .as_ref()
            .and_then(|env| {
                let trimmed = env.trim();
                if looks_like_inline_secret(trimmed) {
                    Some(trimmed.to_string())
                } else {
                    std::env::var(trimmed).ok()
                }
            })
            .filter(|key| !key.is_empty())
            .or_else(|| {
                auth.get(&name)
                    .and_then(|c| c.expose_key().map(String::from))
            })
            .unwrap_or_default();

        if api_key.is_empty() && pconfig.adapter != "ollama" {
            if warn {
                eprintln!("Warning: no credentials for provider '{}', skipping", name);
            }
            continue;
        }

        let forced_model = config
            .forced_model
            .as_ref()
            .filter(|(provider, _)| provider == &name)
            .map(|(_, model)| model.clone());
        let mut model_names = forced_model
            .as_ref()
            .map(|model| vec![model.clone()])
            .unwrap_or_else(|| pconfig.models.clone());
        if forced_model.is_none() {
            for discovered in memory
                .get_discovered_models(&name)
                .into_iter()
                .filter(|model| sparrow::provider::discovery::is_chat_model_id(model))
            {
                if !model_names.iter().any(|model| model == &discovered) {
                    model_names.push(discovered);
                }
            }
        }

        let mut brains: Vec<Arc<dyn sparrow::provider::Brain>> = Vec::new();
        match pconfig.adapter.as_str() {
            "anthropic-messages" => {
                for model in &model_names {
                    brains.push(Arc::new(
                        sparrow::provider::anthropic::AnthropicAdapter::new(
                            model,
                            api_key.clone(),
                            pconfig.base_url.as_deref(),
                        )
                        .with_caps(sparrow::config::providers::model_caps(&name, model)),
                    ));
                }
            }
            "openai-responses" => {
                let base_url = pconfig
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "https://api.openai.com/v1".into());
                for model in &model_names {
                    brains.push(Arc::new(
                        sparrow::provider::responses::OpenAIResponsesAdapter::new(
                            model,
                            api_key.clone(),
                            Some(&base_url),
                        )
                        .with_caps(sparrow::config::providers::model_caps(&name, model)),
                    ));
                }
            }
            "openai-compatible" | "ollama" | "openai-chat" => {
                let base_url = pconfig
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "https://api.openai.com/v1".into());
                for model in &model_names {
                    let adapter: Arc<dyn sparrow::provider::Brain> = if pconfig.adapter == "ollama"
                    {
                        Arc::new(
                            sparrow::provider::ollama::OllamaAdapter::new(model, &base_url)
                                .with_caps(sparrow::config::providers::model_caps(&name, model)),
                        )
                    } else {
                        Arc::new(
                            sparrow::provider::openai_compat::OpenAICompatAdapter::new(
                                model,
                                api_key.clone(),
                                &base_url,
                            )
                            .with_caps(sparrow::config::providers::model_caps(&name, model)),
                        )
                    };
                    brains.push(adapter);
                }
            }
            _ if warn => eprintln!("Unknown adapter: {}", pconfig.adapter),
            _ => {}
        }

        if !brains.is_empty() {
            providers.insert(name.clone(), brains);
        }
    }

    if providers.is_empty() {
        let ollama_url =
            std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434/v1".into());
        if warn {
            println!(
                "No configured providers found. Trying Ollama at {}...",
                ollama_url
            );
        }
        let adapter = sparrow::provider::ollama::OllamaAdapter::new("qwen3.5:32b", &ollama_url);
        providers.insert(
            "ollama".into(),
            vec![Arc::new(adapter) as Arc<dyn sparrow::provider::Brain>],
        );
    }

    providers
}

async fn run_tui(
    config: &sparrow::config::Config,
    memory: Arc<dyn Memory>,
    skills: Arc<dyn SkillLibrary>,
    state_dir: &std::path::Path,
) -> anyhow::Result<()> {
    use sparrow::engine::{Engine, Task};
    use sparrow::provider::{ContentBlock, Msg};
    use sparrow::router::BasicRouter;

    let providers = build_provider_brains(config, &memory, true);
    let router = Arc::new(BasicRouter::new(config, providers));
    let engine = Arc::new(
        Engine::new(router, config.clone())
            .with_memory(memory)
            .with_skills(skills),
    );

    let (task_tx, mut task_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();

    // ── Multi-turn session for the TUI ────────────────────────────────
    // Every message the user types in the TUI is added to a growing
    // conversation that carries context across turns, just like `sparrow
    // chat`. The session is also persisted to sessions.db so quitting
    // and relaunching resumes the conversation.
    let session_key = format!(
        "tui:{}",
        std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "default".into())
    );
    let sessions = sparrow::runtime::session::SessionStore::open(&state_dir.join("sessions.db"))
        .ok()
        .map(Arc::new);
    let prior: Vec<Msg> = sessions
        .as_ref()
        .and_then(|s| s.load(&session_key))
        .and_then(|sess| serde_json::from_str(&sess.messages_json).ok())
        .unwrap_or_default();
    let conversation: Arc<tokio::sync::Mutex<Vec<Msg>>> = Arc::new(tokio::sync::Mutex::new(prior));

    let inject_holder: Arc<tokio::sync::Mutex<Option<tokio::sync::mpsc::UnboundedSender<String>>>> =
        Arc::new(tokio::sync::Mutex::new(None));
    let inject_holder_task = inject_holder.clone();
    let conversation_task = conversation.clone();
    let sessions_task = sessions.clone();
    let session_key_task = session_key.clone();

    tokio::spawn(async move {
        while let Some(description) = task_rx.recv().await {
            // Intercept inject prefix
            if let Some(payload) = description.strip_prefix("__inject__:") {
                if let Some(tx) = inject_holder_task.lock().await.as_ref() {
                    let _ = tx.send(payload.to_string());
                } else {
                    let _ = event_tx.send(sparrow::event::Event::Error {
                        run: sparrow::event::RunId("tui".into()),
                        message: "no active run to inject into".into(),
                    });
                }
                continue;
            }
            if let Some(id) = description.strip_prefix("__rewind__:") {
                let checkpoints = GitCheckpoints::new(std::env::current_dir().unwrap_or_default());
                match checkpoints.rewind(sparrow::event::CheckpointId(id.to_string())) {
                    Ok(()) => {
                        let _ = event_tx.send(sparrow::event::Event::ToolOutput {
                            run: sparrow::event::RunId("tui".into()),
                            id: "rewind".into(),
                            blocks: vec![sparrow::event::Block::Text(format!(
                                "rewound to checkpoint {}",
                                id
                            ))],
                        });
                    }
                    Err(err) => {
                        let _ = event_tx.send(sparrow::event::Event::Error {
                            run: sparrow::event::RunId("tui".into()),
                            message: format!("checkpoint rewind failed: {}", err),
                        });
                    }
                }
                continue;
            }

            // ── Multi-turn: build task with conversation context ──────
            let mut conv = conversation_task.lock().await;
            conv.push(Msg {
                role: "user".into(),
                content: vec![ContentBlock::Text {
                    text: description.clone(),
                }],
            });
            let task = Task {
                description: description.clone(),
                context: conv.clone(),
            };

            let (inject_tx, inject_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
            *inject_holder_task.lock().await = Some(inject_tx);

            let run_id = sparrow::event::RunId::new();
            // Collect assistant reply for conversation history.
            let (run_event_tx, mut run_event_rx) = tokio::sync::mpsc::unbounded_channel();
            let fwd_tx = event_tx.clone();
            let fwd_handle = tokio::spawn(async move {
                let mut buf = String::new();
                while let Some(ev) = run_event_rx.recv().await {
                    if let sparrow::event::Event::ThinkingDelta { text, .. } = &ev {
                        buf.push_str(text);
                    }
                    let _ = fwd_tx.send(ev);
                }
                buf
            });

            if let Err(err) = engine
                .drive_with_inject(task, run_event_tx, run_id, Some(inject_rx))
                .await
            {
                let _ = event_tx.send(sparrow::event::Event::Error {
                    run: sparrow::event::RunId("tui".into()),
                    message: err.to_string(),
                });
            }
            *inject_holder_task.lock().await = None;

            // Append assistant reply to conversation
            if let Ok(reply) = fwd_handle.await {
                if !reply.trim().is_empty() {
                    conv.push(Msg {
                        role: "assistant".into(),
                        content: vec![ContentBlock::Text { text: reply }],
                    });
                }
            }

            // Cap conversation + persist
            let len = conv.len();
            if len > 60 {
                conv.drain(..len - 60);
            }
            if let Some(store) = &sessions_task {
                let _ = store.save(&session_key_task, &conv, None);
            }
        }
    });

    let mut tui = Tui::new().with_channels(task_tx, event_rx);
    drop(inject_holder);
    tokio::task::spawn_blocking(move || tui.run()).await??;

    // Save conversation on TUI exit (belt-and-suspenders)
    if let Some(store) = &sessions {
        let conv = conversation.lock().await;
        let _ = store.save(&session_key, &conv, None);
    }
    Ok(())
}

async fn handle_daemon(
    config: &sparrow::config::Config,
    memory: Arc<dyn Memory>,
    scheduler: Arc<MemoryScheduler>,
    recorder: Arc<FsRecorder>,
    skills: Arc<dyn SkillLibrary>,
) -> anyhow::Result<()> {
    use sparrow::engine::Engine;
    use sparrow::router::BasicRouter;

    let providers = build_provider_brains(config, &memory, true);
    let router = Arc::new(BasicRouter::new(config, providers));
    let engine = Arc::new(
        Engine::new(router, config.clone())
            .with_memory(memory.clone())
            .with_skills(skills),
    );
    let event_bus = EventBus::new(256);
    let runtime = SparrowRuntime::new(
        engine,
        scheduler,
        recorder,
        event_bus,
        memory,
        config.clone(),
    );
    runtime.start().await?;
    println!("Sparrow daemon running. API on 127.0.0.1:9337. Ctrl+C to stop.");
    tokio::signal::ctrl_c().await?;
    runtime.stop().await?;
    Ok(())
}

// ─── OAuth device-flow login — registry-driven ───────────────────────────────
//
// Any provider in the registry with `auth_flow: DeviceOAuth { .. }` is
// automatically supported.  The `client_id` can be passed on the CLI or
// read from the env var declared in the registry entry (`client_id_env`).

async fn handle_auth_login(
    provider: &str,
    client_id: Option<String>,
    auth: &sparrow::auth::store::ChainedAuthStore,
) -> anyhow::Result<()> {
    use sparrow::auth::AuthStore;
    use sparrow::config::providers::{AuthFlow, find_provider, list_oauth_providers};
    use sparrow::extras::OAuthFlow;

    let def = find_provider(provider).ok_or_else(|| {
        let oauth_ids: Vec<String> = list_oauth_providers().into_iter().map(|p| p.id).collect();
        anyhow::anyhow!(
            "Unknown provider '{}'. OAuth-capable providers: {}.\nFor API-key providers use 'sparrow auth add {}'.",
            provider,
            oauth_ids.join(", "),
            provider,
        )
    })?;

    let (device_ep, token_ep, scope, cid_env) = match &def.auth_flow {
        AuthFlow::DeviceOAuth {
            device_endpoint,
            token_endpoint,
            scope,
            client_id_env,
        } => (
            device_endpoint.clone(),
            token_endpoint.clone(),
            scope.clone(),
            client_id_env.clone(),
        ),
        AuthFlow::ApiKey => {
            anyhow::bail!(
                "Provider '{}' uses API-key auth, not OAuth.\nUse 'sparrow auth add {}' instead.",
                provider,
                provider
            )
        }
    };

    let cid = client_id
        .or_else(|| std::env::var(&cid_env).ok())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No OAuth client id for '{}'.\nPass --client-id <id> or set {}.\nRegister an OAuth app with your provider to obtain one.",
                provider, cid_env
            )
        })?;

    println!(
        "Starting OAuth device flow for {} ({})...",
        def.label, provider
    );
    let (verification_uri, user_code, device_code) =
        OAuthFlow::start_device_flow(&device_ep, &token_ep, &cid, &scope).await?;
    println!("\n  1. Open: {}", verification_uri);
    println!("  2. Enter code: {}\n", user_code);
    println!("Waiting for authorization (up to 5 min)...");

    let token = OAuthFlow::poll_token(&token_ep, &cid, &device_code, 300).await?;
    auth.set(provider, sparrow::auth::Credential::api_key(token))?;
    println!("Authenticated. Credential stored for {}.", provider);
    Ok(())
}

async fn run_task(
    task: &str,
    config: &sparrow::config::Config,
    memory: Arc<dyn Memory>,
    skills: Arc<dyn SkillLibrary>,
    recorder: Arc<FsRecorder>,
    soul: Option<Soul>,
) -> anyhow::Result<()> {
    use sparrow::engine::Engine;
    use sparrow::router::BasicRouter;
    use std::sync::Arc;

    let run_config = soul
        .as_ref()
        .map(|soul| config_for_soul(config, soul))
        .unwrap_or_else(|| config.clone());

    let providers = build_provider_brains(&run_config, &memory, true);

    let router = Arc::new(BasicRouter::new(&run_config, providers));
    let mut engine = Engine::new(router, run_config.clone())
        .with_memory(memory.clone())
        .with_skills(skills);
    if let Some(soul) = &soul {
        engine = engine.with_identity(soul.to_identity());
    }

    // ── Session continuity (§8) ───────────────────────────────────────────
    // Load prior conversation so context follows the user across runs and
    // surfaces. Key: $SPARROW_SESSION (set it to "user:<id>" to continue a
    // Telegram/Slack thread) else a per-workspace CLI session.
    let sessions =
        sparrow::runtime::session::SessionStore::open(&run_config.state_dir.join("sessions.db"))
            .ok()
            .map(Arc::new);
    let session_key = std::env::var("SPARROW_SESSION").unwrap_or_else(|_| {
        format!(
            "cli:{}",
            std::env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "default".into())
        )
    });
    let prior_msgs: Vec<sparrow::provider::Msg> = sessions
        .as_ref()
        .and_then(|s| s.load(&session_key))
        .and_then(|sess| match serde_json::from_str(&sess.messages_json) {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("session '{}' deserialize failed: {}", session_key, e);
                None
            }
        })
        .unwrap_or_default();

    let task_obj = sparrow::engine::Task {
        description: task.to_string(),
        context: prior_msgs.clone(),
    };

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    let task_for_recording = task.to_string();
    let config_snapshot = redacted_config_snapshot(&run_config);
    let repo_head = current_repo_head();
    let start_time = std::time::Instant::now();
    eprintln!("\x1b[36m⚡ Sparrow running: {}\x1b[0m", task);
    let print_handle = tokio::spawn(async move {
        let mut full_reply = String::new();
        let mut reasoning_reply = String::new();
        let mut think = sparrow::event::ThinkStripper::new();
        use std::io::Write as _;
        while let Some(event) = rx.recv().await {
            if let sparrow::event::Event::ThinkingDelta { text, .. } = &event {
                full_reply.push_str(text);
            }
            if let sparrow::event::Event::ReasoningDelta { text, .. } = &event {
                reasoning_reply.push_str(text);
            }
            if let sparrow::event::Event::RunStarted { run, agent, .. } = &event {
                recorder.start_run(
                    run.0.clone(),
                    RunInputs {
                        task: task_for_recording.clone(),
                        config_snapshot: config_snapshot.clone(),
                        model_id: "router-selected".into(),
                        repo_head: repo_head.clone(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        agent: agent.clone(),
                    },
                );
            }
            recorder.record(&event);
            if let sparrow::event::Event::RunFinished { run, .. } = &event {
                let _ = recorder.finalize(&run.0);
            }
            match &event {
                sparrow::event::Event::ThinkingDelta { text, .. } => {
                    // Strip <think> reasoning blocks; stream the rest.
                    let visible = think.feed(text);
                    if !visible.is_empty() {
                        print!("{}", visible);
                        let _ = std::io::stdout().flush();
                    }
                }
                sparrow::event::Event::ToolUseProposed { name, .. } => {
                    println!("\n[Tool: {}]", name);
                }
                sparrow::event::Event::ApprovalRequested { summary, .. } => {
                    println!("\n[APPROVAL NEEDED: {}]", summary);
                }
                sparrow::event::Event::CheckpointCreated { id, label, .. } => {
                    println!("\n[Checkpoint: {} — {}]", id.0, label);
                }
                sparrow::event::Event::ModelSwitched {
                    from, to, reason, ..
                } => {
                    let clean = sparrow::event::friendly_model_switch_reason(reason);
                    if sparrow::event::is_local_model_unavailable(reason) {
                        println!(
                            "\n[Routing] modèle local indisponible → routage modèle cloud ({})",
                            to
                        );
                    } else {
                        println!("\n[Routing] {} → {} ({})", from, to, clean);
                    }
                }
                // Cost is shown once at the end (no noisy inline $0.0000 prints).
                sparrow::event::Event::RunFinished { outcome, .. } => {
                    // Flush any text held back by the think-stripper (recovers an
                    // unclosed <think> so the answer is never silently swallowed).
                    let tail = think.flush();
                    if !tail.trim().is_empty() {
                        print!("{}", tail);
                        let _ = std::io::stdout().flush();
                    }
                    println!(
                        "\nDone. Cost: ${:.4}, Tokens: {} in / {} out",
                        outcome.cost_usd, outcome.tokens.input, outcome.tokens.output,
                    );
                    // Full cost comparison — Sparrow's competitive moat
                    if outcome.tokens.input > 0 || outcome.tokens.output > 0 {
                        println!(
                            "{}",
                            sparrow::cost::format_comparison(outcome.cost_usd, &outcome.tokens)
                        );
                    }
                }
                sparrow::event::Event::Error { message, .. }
                    if !sparrow::event::is_local_model_unavailable(message) =>
                {
                    eprintln!("\nError: {}", message);
                }
                _ => {}
            }
        }
        (full_reply, reasoning_reply)
    });

    println!("Running: {}", task);
    let drive_result = engine.drive(task_obj, tx).await;
    let (full_reply, reasoning_reply) = print_handle.await.unwrap_or_default();

    // Persist the turn to the session BEFORE propagating any error, so a
    // transient failure never erases the user's message from the conversation.
    if let Some(store) = &sessions {
        let mut updated = prior_msgs;
        updated.push(sparrow::provider::Msg {
            role: "user".into(),
            content: vec![sparrow::provider::ContentBlock::Text {
                text: task.to_string(),
            }],
        });
        if !full_reply.trim().is_empty() {
            let mut content = Vec::new();
            if !reasoning_reply.trim().is_empty() {
                content.push(sparrow::provider::ContentBlock::Reasoning {
                    text: reasoning_reply,
                });
            }
            content.push(sparrow::provider::ContentBlock::Text { text: full_reply });
            updated.push(sparrow::provider::Msg {
                role: "assistant".into(),
                content,
            });
        }
        let len = updated.len();
        if len > 40 {
            updated.drain(..len - 40);
        }
        let _ = store.save(&session_key, &updated, None);
    }

    let outcome = drive_result?;
    let elapsed = start_time.elapsed();
    println!(
        "Status: {}  ⏱ {:.1}s",
        outcome.status,
        elapsed.as_secs_f64()
    );
    Ok(())
}

fn config_for_soul(config: &sparrow::config::Config, soul: &Soul) -> sparrow::config::Config {
    let mut run_config = config.clone();
    if let Some(model_ref) = soul.default_model.as_deref() {
        if let Some((provider, model)) = parse_agent_model_ref(model_ref) {
            run_config.forced_model = Some((provider.clone(), model.clone()));
            for tier in ["trivial", "small", "medium", "hard", "vision"] {
                run_config
                    .routing
                    .policy
                    .insert(tier.to_string(), provider.clone());
            }
            run_config
                .providers
                .entry(provider)
                .or_insert_with(|| ProviderConfig {
                    adapter: "openai-compatible".into(),
                    base_url: None,
                    models: vec![],
                    api_key_env: None,
                })
                .models = vec![model];
        }
    }
    if let Some(mode) = soul
        .permission_mode
        .as_deref()
        .or(soul.default_autonomy.as_deref())
        .and_then(sparrow::permissions::PermissionMode::parse)
    {
        run_config.defaults.autonomy = mode.autonomy_level();
        run_config.permissions.mode = mode;
    }
    for tool in &soul.disallowed_tools {
        if !run_config.permissions.tools.deny.contains(tool) {
            run_config.permissions.tools.deny.push(tool.clone());
        }
    }
    if !soul.tools.is_empty() {
        for tool in &soul.tools {
            if !run_config.permissions.tools.allow.contains(tool) {
                run_config.permissions.tools.allow.push(tool.clone());
            }
        }
    }
    run_config
}

fn parse_agent_model_ref(model_ref: &str) -> Option<(String, String)> {
    let model_ref = model_ref.trim();
    if model_ref.is_empty() {
        return None;
    }
    if let Some((provider, model)) = model_ref.split_once(':') {
        let provider = provider.trim();
        let model = model.trim();
        if !provider.is_empty() && !model.is_empty() {
            return Some((provider.to_string(), model.to_string()));
        }
    }
    if let Some((provider, rest)) = model_ref.split_once('/') {
        let provider = provider.trim();
        if !provider.is_empty() {
            return Some((provider.to_string(), model_ref.to_string()));
        }
        if !rest.trim().is_empty() {
            return Some(("custom".into(), model_ref.to_string()));
        }
    }
    Some(("custom".into(), model_ref.to_string()))
}

fn handle_plan(
    task: &str,
    config: &sparrow::config::Config,
    skills: Arc<dyn SkillLibrary>,
    json: bool,
) -> anyhow::Result<()> {
    let project_root = std::env::current_dir()?;
    let commands =
        sparrow::commands::all_commands(&project_root, &config.config_dir, Some(skills.as_ref()));
    let plan = sparrow::plan::build_read_only_plan(task, &commands);
    if json {
        println!("{}", serde_json::to_string_pretty(&plan)?);
    } else {
        println!("{}", plan.render_markdown());
    }
    Ok(())
}

fn handle_permissions(
    action: sparrow::cli::PermissionAction,
    config: &sparrow::config::Config,
    store: &FsConfigStore,
) -> anyhow::Result<()> {
    let mut updated = config.clone();
    match action {
        sparrow::cli::PermissionAction::List => {
            print_permission_policy(&updated);
            return Ok(());
        }
        sparrow::cli::PermissionAction::Set { mode } => {
            let Some(mode) = sparrow::permissions::PermissionMode::parse(&mode) else {
                anyhow::bail!(
                    "Unknown permission mode '{}'. Use read-only, plan, supervised, trusted, autonomous, or emergency-stop.",
                    mode
                );
            };
            updated.permissions.mode = mode.clone();
            updated.defaults.autonomy = mode.autonomy_level();
            println!(
                "Permission mode set to '{}' (autonomy: {:?}).",
                mode.as_str(),
                updated.defaults.autonomy
            );
        }
        sparrow::cli::PermissionAction::AllowTool { tool } => {
            push_unique(&mut updated.permissions.tools.allow, tool);
            println!("Tool allow rule added.");
        }
        sparrow::cli::PermissionAction::AskTool { tool } => {
            push_unique(&mut updated.permissions.tools.ask, tool);
            println!("Tool approval rule added.");
        }
        sparrow::cli::PermissionAction::DenyTool { tool } => {
            push_unique(&mut updated.permissions.tools.deny, tool);
            println!("Tool deny rule added.");
        }
        sparrow::cli::PermissionAction::AllowPath { path } => {
            push_unique_path(&mut updated.permissions.paths.allow, path);
            println!("Path allow rule added.");
        }
        sparrow::cli::PermissionAction::DenyPath { path } => {
            push_unique_path(&mut updated.permissions.paths.deny, path);
            println!("Path deny rule added.");
        }
    }
    store.save(&updated)?;
    print_permission_policy(&updated);
    Ok(())
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn push_unique_path(values: &mut Vec<std::path::PathBuf>, value: std::path::PathBuf) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn print_permission_policy(config: &sparrow::config::Config) {
    let policy = &config.permissions;
    println!("Permission policy");
    println!("=================");
    println!("Mode     : {}", policy.mode.as_str());
    println!("Autonomy : {:?}", config.defaults.autonomy);
    println!("Tools");
    println!("  allow : {}", list_or_empty(&policy.tools.allow));
    println!("  ask   : {}", list_or_empty(&policy.tools.ask));
    println!("  deny  : {}", list_or_empty(&policy.tools.deny));
    println!("Paths");
    println!("  allow : {}", path_list_or_empty(&policy.paths.allow));
    println!("  deny  : {}", path_list_or_empty(&policy.paths.deny));
    println!("Providers");
    println!("  allow : {}", list_or_empty(&policy.providers.allow));
    println!("  ask   : {}", list_or_empty(&policy.providers.ask));
    println!("  deny  : {}", list_or_empty(&policy.providers.deny));
    println!("Surfaces");
    println!("  allow : {}", list_or_empty(&policy.surfaces.allow));
    println!("  ask   : {}", list_or_empty(&policy.surfaces.ask));
    println!("  deny  : {}", list_or_empty(&policy.surfaces.deny));
}

fn list_or_empty(values: &[String]) -> String {
    if values.is_empty() {
        "(empty)".into()
    } else {
        values.join(", ")
    }
}

fn path_list_or_empty(values: &[std::path::PathBuf]) -> String {
    if values.is_empty() {
        "(empty)".into()
    } else {
        values
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

// ─── Swarm command ──────────────────────────────────────────────────────────────

async fn run_swarm(
    task: &str,
    config: &sparrow::config::Config,
    memory: Arc<dyn Memory>,
) -> anyhow::Result<()> {
    use sparrow::orchestrator::{DefaultOrchestrator, Orchestrator, SwarmPlan};
    use sparrow::router::BasicRouter;
    use std::sync::Arc;

    let providers = build_provider_brains(config, &memory, true);

    if providers.is_empty() {
        anyhow::bail!("No providers configured. Set up at least one provider with an API key.");
    }

    let router = Arc::new(BasicRouter::new(config, providers));
    let orchestrator = DefaultOrchestrator::new(router, config.clone(), memory.clone());

    let cwd = std::env::current_dir().unwrap_or_default();
    let plan = SwarmPlan {
        task: task.to_string(),
        workspace: cwd,
        max_reworks: 3,
    };

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    let print_handle = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            match &event {
                sparrow::event::Event::AgentSpawned { role, model, .. } => {
                    println!("\n┌─ {} spawned ({})", role.to_uppercase(), model);
                }
                sparrow::event::Event::AgentStatus {
                    role, status, note, ..
                } => {
                    let icon = match status {
                        sparrow::event::AgentStatus::Done => "✓",
                        sparrow::event::AgentStatus::Working => "●",
                        sparrow::event::AgentStatus::Thinking => "○",
                        sparrow::event::AgentStatus::Error => "✗",
                        _ => "◌",
                    };
                    println!("│ {} {} — {}", icon, role, note);
                }
                sparrow::event::Event::TestResult {
                    passed: _,
                    failed,
                    detail,
                    ..
                } => {
                    if *failed > 0 {
                        println!("├─ ✗ VERIFY FAILED ({} issues)", failed);
                        for line in detail.lines() {
                            println!("│    {}", line);
                        }
                    } else {
                        println!("└─ ✓ VERIFY PASSED");
                    }
                }
                sparrow::event::Event::RunFinished { outcome, .. } => {
                    println!("\n═══ Swarm complete ═══");
                    println!("Status : {}", outcome.status);
                    println!("Diffs  : {} files", outcome.diffs.len());
                    for d in &outcome.diffs {
                        println!("  {}  +{}/-{}", d.file, d.plus, d.minus);
                    }
                }
                sparrow::event::Event::Error { message, .. }
                    if !sparrow::event::is_local_model_unavailable(message) =>
                {
                    eprintln!("Error: {}", message);
                }
                _ => {}
            }
        }
    });

    println!("═══ Swarm: {task} ═══\n");

    let outcome = orchestrator.run_swarm(plan, tx).await?;
    print_handle.await?;

    println!(
        "\nPlan  : {} chars",
        outcome.plan.as_ref().map(|p| p.len()).unwrap_or(0)
    );
    println!("Passes: {}", outcome.passes);
    println!("Reworks: {}", outcome.reworks);
    if let Some(plan) = &outcome.plan {
        if plan.len() < 500 {
            println!("\n{}", plan);
        }
    }

    Ok(())
}

// ─── Skills commands ────────────────────────────────────────────────────────────

fn handle_skills(
    action: sparrow::cli::SkillsAction,
    library: &Arc<dyn SkillLibrary>,
) -> anyhow::Result<()> {
    match action {
        sparrow::cli::SkillsAction::List => {
            let skills = library.all();
            if skills.is_empty() {
                println!("No skills in library.");
                println!("Skills are automatically learned from successful runs.");
                println!("Create one manually: sparrow skills create <name>");
            } else {
                println!("Skill library ({} skills):", skills.len());
                for s in &skills {
                    let tag = if s.auto_generated { "[auto]" } else { "[user]" };
                    println!(
                        "  {} {} | triggers: {} | score: {:.2} | used: {}",
                        tag,
                        s.name,
                        s.trigger.join(", "),
                        s.score,
                        s.usage_count
                    );
                }
            }
        }
        sparrow::cli::SkillsAction::View { name } => match library.invoke(&name)? {
            Some(invocation) => {
                println!("# {}", invocation.skill.name);
                println!("{}", invocation.skill.description);
                println!("Triggers: {}", invocation.skill.trigger.join(", "));
                println!();
                println!("{}", invocation.skill.body);
                if !invocation.loaded_references.is_empty() {
                    println!("\nLoaded references:");
                    for (path, content) in invocation.loaded_references {
                        println!("## {}", path);
                        println!("{}", content);
                    }
                }
            }
            None => println!("No skill named '{}'.", name),
        },
        sparrow::cli::SkillsAction::Create { name } => {
            let skill = sparrow::capabilities::Skill {
                name: name.clone(),
                description: format!("User-created skill: {}", name),
                trigger: vec![name.to_lowercase()],
                body: format!("# {}\n\nAdd skill content here.", name),
                source_file: format!("{}.skill.md", name),
                usage_count: 0,
                created_at: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                score: 0.5,
                auto_generated: false,
                references: Vec::new(),
                templates: Vec::new(),
                scripts: Vec::new(),
                assets: Vec::new(),
            };
            library.add(skill)?;
            println!(
                "Skill '{}' created. Edit: ~/.config/sparrow/skills/{}/SKILL.md",
                name, name
            );
        }
        sparrow::cli::SkillsAction::Install { source } => {
            let skill = load_skill_from_source(&source)?;
            let name = skill.name.clone();
            library.add(skill)?;
            println!("Installed skill '{}'.", name);
        }
        sparrow::cli::SkillsAction::Update { name } => {
            let Some(skill) = library.get(&name) else {
                println!("No skill named '{}'.", name);
                return Ok(());
            };
            library.add(skill)?;
            println!("Skill '{}' refreshed.", name);
        }
        sparrow::cli::SkillsAction::Prune => {
            let removed = library.prune(0.2)?;
            println!(
                "Curator pruned {} low-score auto-generated skill(s).",
                removed
            );
            let skills = library.all();
            println!("Library now has {} skills.", skills.len());
        }
        sparrow::cli::SkillsAction::Rm { name } => {
            if library.remove(&name)? {
                println!("Removed skill '{}'.", name);
            } else {
                println!(
                    "No skill named '{}'. Run 'sparrow skills list' to see names.",
                    name
                );
            }
        }
    }
    Ok(())
}

fn load_skill_from_source(source: &str) -> anyhow::Result<sparrow::capabilities::Skill> {
    let source = source.trim();
    let temp_dir;
    let path = if source.starts_with("http://")
        || source.starts_with("https://")
        || source.ends_with(".git")
        || source.contains("github.com")
    {
        temp_dir = std::env::temp_dir().join(format!("sparrow-skill-{}", uuid::Uuid::new_v4()));
        let status = std::process::Command::new("git")
            .args([
                "clone",
                "--depth",
                "1",
                source,
                temp_dir.to_string_lossy().as_ref(),
            ])
            .status()?;
        if !status.success() {
            anyhow::bail!("git clone failed for skill source {}", source);
        }
        temp_dir.clone()
    } else {
        temp_dir = std::path::PathBuf::new();
        std::path::PathBuf::from(source)
    };

    let skill_file = if path.is_dir() {
        path.join("SKILL.md")
    } else {
        path.clone()
    };
    let content = std::fs::read_to_string(&skill_file)?;
    let source_file = skill_file
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "SKILL.md".into());
    let skill = sparrow::capabilities::Skill::from_markdown(&content, &source_file)
        .ok_or_else(|| anyhow::anyhow!("could not parse skill from {}", skill_file.display()))?;
    if !temp_dir.as_os_str().is_empty() {
        let _ = std::fs::remove_dir_all(temp_dir);
    }
    Ok(skill)
}

fn handle_plugins(
    action: sparrow::cli::PluginsAction,
    config_dir: &std::path::Path,
) -> anyhow::Result<()> {
    let plugins_dir = config_dir.join("plugins");
    match action {
        sparrow::cli::PluginsAction::List => {
            let registry = sparrow::capabilities::plugin::PluginRegistry::new(plugins_dir);
            let plugins = registry.scan();
            if plugins.is_empty() {
                println!("No plugins installed.");
            } else {
                println!("Plugins ({}):", plugins.len());
                for plugin in plugins {
                    let audit = registry.audit(&plugin);
                    println!(
                        "  {} {} | commands:{} skills:{} hooks:{} | {}",
                        plugin.manifest.name,
                        plugin.manifest.version,
                        plugin.manifest.commands.len(),
                        plugin.manifest.skills.len(),
                        plugin.manifest.hooks.len(),
                        if audit.allowed { "allowed" } else { "blocked" }
                    );
                    for warning in audit.warnings {
                        println!("    - {}", warning);
                    }
                }
            }
        }
        sparrow::cli::PluginsAction::Install { source, allow } => {
            let source_path = std::path::PathBuf::from(&source);
            let mut allowlist = Vec::new();
            if allow {
                if let Ok(plugin) = sparrow::capabilities::plugin::load_plugin(&source_path) {
                    allowlist.push(plugin.manifest.name);
                }
            }
            let registry = sparrow::capabilities::plugin::PluginRegistry::new(plugins_dir)
                .with_allowlist(allowlist);
            let plugin = if source.starts_with("http://")
                || source.starts_with("https://")
                || source.ends_with(".git")
                || source.contains("github.com")
            {
                registry.install_github(&source)?
            } else {
                registry.install_local(&source_path)?
            };
            println!("Installed plugin '{}'.", plugin.manifest.name);
        }
        sparrow::cli::PluginsAction::Rm { name } => {
            let path = plugins_dir.join(&name);
            if path.exists() {
                std::fs::remove_dir_all(path)?;
                println!("Removed plugin '{}'.", name);
            } else {
                println!("No plugin named '{}'.", name);
            }
        }
    }
    Ok(())
}

fn handle_tools(
    action: sparrow::cli::ToolsAction,
    config_store: &FsConfigStore,
) -> anyhow::Result<()> {
    match action {
        sparrow::cli::ToolsAction::List { surface } => {
            let metas = sparrow::tools::known_tool_metadata(surface.as_deref());
            println!("Toolsets: {}", sparrow::tools::TOOLSETS.join(", "));
            println!("Tools ({}):", metas.len());
            for meta in metas {
                println!(
                    "  {:16} set:{:14} risk:{:?} auth:{} mutates:{} network:{} exec:{}",
                    meta.name,
                    meta.toolset,
                    meta.risk,
                    meta.requires_auth,
                    meta.mutates_files,
                    meta.network,
                    meta.exec
                );
            }
        }
        sparrow::cli::ToolsAction::Enable { tool } => {
            let mut cfg = config_store.load()?;
            cfg.permissions.tools.deny.retain(|item| item != &tool);
            if !cfg.permissions.tools.allow.contains(&tool) {
                cfg.permissions.tools.allow.push(tool.clone());
            }
            config_store.save(&cfg)?;
            println!("Tool '{}' enabled in permissions.", tool);
        }
        sparrow::cli::ToolsAction::Disable { tool } => {
            let mut cfg = config_store.load()?;
            cfg.permissions.tools.allow.retain(|item| item != &tool);
            if !cfg.permissions.tools.deny.contains(&tool) {
                cfg.permissions.tools.deny.push(tool.clone());
            }
            config_store.save(&cfg)?;
            println!("Tool '{}' disabled in permissions.", tool);
        }
    }
    Ok(())
}

// ─── Security audit ─────────────────────────────────────────────────────────────

fn handle_security(
    action: sparrow::cli::SecurityAction,
    config: &sparrow::config::Config,
) -> anyhow::Result<()> {
    match action {
        sparrow::cli::SecurityAction::Audit { json } => {
            let audit = sparrow::security::SecurityAudit::run(config, &config.hooks);
            if json {
                println!("{}", audit.to_json());
            } else {
                println!("{}", audit.summary());
                for f in &audit.findings {
                    let tag = match f.severity {
                        sparrow::security::Severity::Critical => "CRIT",
                        sparrow::security::Severity::Warning => "WARN",
                        sparrow::security::Severity::Info => "INFO",
                    };
                    println!("  [{}] {}: {}", tag, f.category, f.message);
                    if !f.recommendation.is_empty() {
                        println!("        → {}", f.recommendation);
                    }
                }
            }
        }
    }
    Ok(())
}

// ─── Context compaction / handoff ───────────────────────────────────────────────

fn handle_compact(
    task: Option<String>,
    out: Option<std::path::PathBuf>,
    json: bool,
) -> anyhow::Result<()> {
    use sparrow::context::HandoffDoc;

    let task_str = task.unwrap_or_else(|| "ad-hoc handoff".into());
    let doc = HandoffDoc::new(task_str);

    let default_path = std::path::PathBuf::from(".sparrow/handoff").join(format!(
        "{}.md",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
    ));
    let path = out.unwrap_or(default_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let md = doc.to_markdown();
    std::fs::write(&path, &md)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": true,
                "path": path.to_string_lossy(),
                "doc": doc,
            }))?
        );
    } else {
        println!("handoff written: {}", path.display());
        println!("---");
        println!("{}", md);
    }
    Ok(())
}

// ─── GitHub Action / remote PR ──────────────────────────────────────────────────

fn handle_github(action: sparrow::cli::GithubAction) -> anyhow::Result<()> {
    use sparrow::cli::GithubAction;
    use sparrow::github;

    match action {
        GithubAction::Review {
            pr,
            dry_run,
            model,
            allowed_tools,
        } => {
            let mut plan = github::plan_review(pr, model, allowed_tools, dry_run);
            if dry_run {
                println!("{}", serde_json::to_string_pretty(&plan)?);
                return Ok(());
            }
            github::require_action_env()?;
            plan.diff_preview = github::fetch_pr_diff(pr)?;
            // We intentionally do NOT call the model here. The actual review
            // is performed by `sparrow run` invoked separately by the Action
            // composite step, so this command stays a pure data-fetcher.
            println!("{}", serde_json::to_string_pretty(&plan)?);
        }
        GithubAction::Status => {
            github::require_action_env()?;
            println!("{}", github::ci_status()?);
        }
        GithubAction::Logs { run_id } => {
            github::require_action_env()?;
            println!("{}", github::ci_logs(&run_id)?);
        }
    }
    Ok(())
}

// ─── MCP commands ───────────────────────────────────────────────────────────────

async fn handle_mcp(
    action: sparrow::cli::McpAction,
    config_dir: &std::path::PathBuf,
) -> anyhow::Result<()> {
    let client = BasicMcpClient::new(config_dir.join("mcp"));

    match action {
        sparrow::cli::McpAction::List => {
            let servers = client.list_servers().await;
            if servers.is_empty() {
                println!("No MCP servers configured.");
                println!("Add one: sparrow mcp add <name> --command <cmd> --args <args>");
            } else {
                println!("MCP servers:");
                for s in &servers {
                    let transport = match s.transport {
                        Transport::Stdio => "stdio",
                        Transport::Sse => "sse",
                        Transport::Url => "url",
                    };
                    println!(
                        "  {} ({}) | {} tools allowed",
                        s.name,
                        transport,
                        if s.allow_tools.is_empty() {
                            "all".to_string()
                        } else {
                            s.allow_tools.len().to_string()
                        }
                    );
                }
            }
        }
        sparrow::cli::McpAction::Add {
            server,
            command,
            args,
            transport,
        } => {
            if let Some(command) = command {
                let transport = match transport.as_deref().unwrap_or("stdio") {
                    "stdio" => Transport::Stdio,
                    "sse" => Transport::Sse,
                    "url" => Transport::Url,
                    other => anyhow::bail!("Unsupported MCP transport: {}", other),
                };
                client.add_server(McpServer {
                    name: server.clone(),
                    transport,
                    command: Some(command),
                    args,
                    url: None,
                    env: Default::default(),
                    allow_tools: vec![],
                })?;
                println!("Added MCP server: {}", server);
            } else {
                println!("Adding MCP server: {}", server);
                println!(
                    "Usage: sparrow mcp add {} --command <cmd> --args \"<args>\"",
                    server
                );
                println!("Example:");
                println!(
                    r#"  sparrow mcp add {} --command npx --args "-y @modelcontextprotocol/server-filesystem C:\Sparrow""#,
                    server
                );
            }
        }
        sparrow::cli::McpAction::Rm { server } => {
            client.remove_server(&server)?;
            println!("Removed MCP server: {}", server);
        }
    }
    Ok(())
}

// ─── Schedule command ───────────────────────────────────────────────────────────

async fn handle_schedule(
    task: &str,
    cron: &str,
    autonomy: Option<String>,
    _report: &[String],
    scheduler: &Arc<MemoryScheduler>,
) -> anyhow::Result<()> {
    let level = match autonomy.as_deref() {
        Some("autonomous") => sparrow::event::AutonomyLevel::Autonomous,
        Some("trusted") => sparrow::event::AutonomyLevel::Trusted,
        _ => sparrow::event::AutonomyLevel::Supervised,
    };

    // Try natural-language → cron translation before creating the job
    let resolved_cron =
        sparrow::runtime::scheduler::parse_nl_cron(cron).unwrap_or_else(|| cron.to_string());

    if resolved_cron != cron {
        println!("Parsed schedule: \"{}\" → {}", cron, resolved_cron);
    }

    // Validate the cron expression before storing
    {
        use cron::Schedule;
        use std::str::FromStr;
        if Schedule::from_str(&resolved_cron).is_err() {
            anyhow::bail!(
                "Invalid cron expression: '{}'. Use cron syntax (e.g. '0 2 * * *') \
                 or natural language (e.g. 'every day at 2am').",
                resolved_cron
            );
        }
    }

    let mut job = Job::new(task.to_string(), resolved_cron.clone());
    job.autonomy = level.clone();
    job.next_run = job.next_schedule().map(|dt| dt.to_rfc3339());

    let id = scheduler.schedule(job)?;
    let jobs = scheduler.list();

    println!("Job scheduled: {}", id);
    println!("Task    : {}", task);
    println!("Cron    : {}", resolved_cron);
    println!("Autonomy: {:?}", level);

    if let Some(j) = jobs.iter().find(|j| j.id == id) {
        if let Some(next) = &j.next_run {
            println!("Next run: {}", next);
        }
    }

    println!("\nAll scheduled jobs ({}):", jobs.len());
    for j in &jobs {
        let next = j.next_run.as_deref().unwrap_or("pending");
        let status = if j.enabled { "active" } else { "paused" };
        println!("  {} {} | {} | {}", status, j.id, j.cron, next);
    }

    Ok(())
}

// ─── Replay command ─────────────────────────────────────────────────────────────

async fn handle_replay(
    run_id: &str,
    recorder: &Arc<FsRecorder>,
    config: &sparrow::config::Config,
    memory: Arc<dyn Memory>,
) -> anyhow::Result<()> {
    match recorder.load(run_id) {
        Some(transcript) => {
            println!("═══ REPLAY: {} ═══", run_id);
            println!("Task  : {}", transcript.inputs.task);
            println!("Agent : {}", transcript.inputs.agent);
            println!("Model : {}", transcript.inputs.model_id);
            println!("Events: {}", transcript.events.len());
            println!();

            for event in &transcript.events {
                match event {
                    sparrow::event::Event::ThinkingDelta { text, .. } => {
                        print!("{}", text);
                    }
                    sparrow::event::Event::ToolUseProposed { name, .. } => {
                        println!("\n[Tool: {}]", name);
                    }
                    sparrow::event::Event::RunFinished { outcome, .. } => {
                        println!(
                            "\n--- Done: {} | Cost: ${:.4} {}---",
                            outcome.status,
                            outcome.cost_usd,
                            sparrow::cost::format_comparison_oneliner(
                                outcome.cost_usd,
                                &outcome.tokens
                            )
                        );
                    }
                    sparrow::event::Event::Error { message, .. }
                        if !sparrow::event::is_local_model_unavailable(message) =>
                    {
                        eprintln!("\n[Error: {}]", message);
                    }
                    _ => {}
                }
            }

            println!("\n═══ Re-execute? (y/N) ═══");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if input.trim().to_lowercase() == "y" {
                use sparrow::engine::Engine;
                use sparrow::router::BasicRouter;
                let providers = build_provider_brains(config, &memory, true);
                let router = Arc::new(BasicRouter::new(config, providers));
                let engine = Arc::new(Engine::new(router, config.clone()).with_memory(memory));
                let re_executer = ReExecuter::new(engine);
                println!("Re-executing against current model...");
                match re_executer.re_execute(&transcript).await {
                    Ok(outcome) => println!(
                        "Re-execute done: {} | ${:.4}",
                        outcome.status, outcome.cost_usd
                    ),
                    Err(e) => eprintln!("Re-execute failed: {}", e),
                }
            }
            println!("\n═══ End of replay ═══");
        }
        None => {
            let transcripts = recorder.list_transcripts();
            if transcripts.is_empty() {
                println!("No transcripts found.");
            } else {
                println!("Transcript not found: {}", run_id);
                println!("\nAvailable:");
                for t in &transcripts {
                    if let Some(tr) = recorder.load(t) {
                        println!(
                            "  {} | {} events | {}",
                            t,
                            tr.events.len(),
                            tr.inputs.task.chars().take(60).collect::<String>()
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

// ─── Chat command ───────────────────────────────────────────────────────────────

async fn handle_chat(
    config: &sparrow::config::Config,
    memory: Arc<dyn Memory>,
) -> anyhow::Result<()> {
    use sparrow::engine::Engine;
    use sparrow::router::BasicRouter;

    let providers = build_provider_brains(config, &memory, true);

    let router = Arc::new(BasicRouter::new(config, providers));
    let engine = Arc::new(Engine::new(router, config.clone()).with_memory(memory));
    let mut session = ChatSession::new(engine);
    session.run_interactive().await
}

// ─── Gateway command ────────────────────────────────────────────────────────────

async fn handle_gateway(
    action: sparrow::cli::GatewayAction,
    state_dir: &std::path::PathBuf,
    config: &sparrow::config::Config,
    memory: Arc<dyn Memory>,
    scheduler: Arc<MemoryScheduler>,
    recorder: Arc<FsRecorder>,
) -> anyhow::Result<()> {
    match action {
        sparrow::cli::GatewayAction::Start => {
            println!("Starting gateway daemon...");
            write_gateway_pid(state_dir)?;

            use sparrow::engine::Engine;
            use sparrow::router::BasicRouter;

            let providers = build_provider_brains(config, &memory, true);

            let router = Arc::new(BasicRouter::new(config, providers));
            let engine = Arc::new(Engine::new(router, config.clone()).with_memory(memory.clone()));
            let _cron_handle = scheduler.start_cron_loop(engine.clone(), recorder.clone());

            // Event bus for pub/sub
            let (event_bus_tx, _) = tokio::sync::broadcast::channel::<sparrow::event::Event>(256);

            // Session store for cross-surface continuity (§8)
            let session_store = std::sync::Arc::new(sparrow::runtime::session::SessionStore::open(
                &config.state_dir.join("sessions.db"),
            )?);

            // Message router
            let router_handler = Arc::new(
                MessageRouter::new(engine, recorder.clone(), event_bus_tx, vec![])
                    .with_sessions(session_store),
            );

            // Channel: transports → router
            let (msg_tx, mut msg_rx) = tokio::sync::mpsc::unbounded_channel::<GatewayMessage>();
            // Channel: router → transports
            let (resp_tx, mut resp_rx) = tokio::sync::mpsc::unbounded_channel::<GatewayResponse>();

            // Start transports based on config
            let mut transports: Vec<Box<dyn GatewayTransport>> = Vec::new();

            // Telegram
            if let Some(ref tg) = config.surfaces.telegram {
                if tg.enabled {
                    let token = tg
                        .token_env
                        .as_ref()
                        .and_then(|env| std::env::var(env).ok())
                        .unwrap_or_default();
                    if !token.is_empty() {
                        println!("  Telegram : enabled");
                        transports.push(Box::new(TelegramTransport::new(
                            token,
                            tg.allow_users.clone(),
                        )));
                    } else {
                        println!("  Telegram : no token (set TELEGRAM_BOT_TOKEN)");
                    }
                }
            }

            // Discord
            if let Some(ref dc) = config.surfaces.discord {
                if dc.enabled {
                    let token = dc
                        .token_env
                        .as_ref()
                        .and_then(|env| std::env::var(env).ok())
                        .unwrap_or_default();
                    if !token.is_empty() {
                        println!("  Discord  : enabled");
                        transports.push(Box::new(DiscordTransport::new(
                            token,
                            dc.allow_users.clone(),
                        )));
                    } else {
                        println!("  Discord  : no token (set DISCORD_BOT_TOKEN)");
                    }
                }
            }

            // Slack
            if let Some(ref sl) = config.surfaces.slack {
                if sl.enabled {
                    let app_token = std::env::var("SLACK_APP_TOKEN").unwrap_or_default();
                    let bot_token = sl
                        .token_env
                        .as_ref()
                        .and_then(|env| std::env::var(env).ok())
                        .unwrap_or_default();
                    if !app_token.is_empty() && !bot_token.is_empty() {
                        println!("  Slack    : enabled (Socket Mode)");
                        transports.push(Box::new(SlackTransport::new(
                            app_token,
                            bot_token,
                            sl.allow_users.clone(),
                        )));
                    } else {
                        println!("  Slack    : no token (set SLACK_APP_TOKEN + SLACK_BOT_TOKEN)");
                    }
                }
            }

            // Email (outbound only, behind `email` feature)
            if let Some(ref em) = config.surfaces.email {
                if em.enabled {
                    let user = std::env::var(&em.username_env).unwrap_or_default();
                    let pass = std::env::var(&em.password_env).unwrap_or_default();
                    if !user.is_empty() && !pass.is_empty() {
                        println!(
                            "  Email    : enabled (SMTP {}:{})",
                            em.smtp_host, em.smtp_port
                        );
                        let mut email_transport = sparrow::gateway::email::EmailTransport::new(
                            em.from.clone(),
                            em.smtp_host.clone(),
                            em.smtp_port,
                            user,
                            pass,
                            em.allowed_to.clone(),
                        );
                        if let Some(imap_host) = &em.imap_host {
                            email_transport =
                                email_transport.with_imap(imap_host.clone(), em.imap_port);
                            println!("             + IMAP inbound {}:{}", imap_host, em.imap_port);
                        }
                        transports.push(Box::new(email_transport));
                    } else {
                        println!(
                            "  Email    : no credentials (set {} + {})",
                            em.username_env, em.password_env
                        );
                    }
                }
            }

            // Always start WebSocket API
            println!("  WS API   : ws://127.0.0.1:9338");
            let ws_api = WebSocketApi::new("127.0.0.1:9338");
            transports.push(Box::new(ws_api));

            println!(
                "  Extra    : WhatsApp/Signal/Feishu/WeCom/QQ/Teams adapters present, not started without credentials"
            );

            if transports.is_empty() {
                println!("\nNo transports configured. Set up tokens in config.toml or env vars.");
                return Ok(());
            }

            // Start all transports
            for transport in &transports {
                let tx = msg_tx.clone();
                let name = transport.name().to_string();
                if let Err(e) = transport.start(tx).await {
                    eprintln!("Failed to start {}: {}", name, e);
                }
            }

            println!("\nGateway running. Press Ctrl+C to stop.");
            println!("Send messages via any configured surface.\n");

            // Abort poller: watch `<state>/gateway-abort/<run>.abort` files
            // written by `sparrow gateway abort <run>` and cancel the matching
            // in-process run via the registry.
            {
                let registry = router_handler.run_registry.clone();
                let abort_dir = state_dir.join("gateway-abort");
                std::fs::create_dir_all(&abort_dir).ok();
                tokio::spawn(async move {
                    loop {
                        if let Ok(entries) = std::fs::read_dir(&abort_dir) {
                            for entry in entries.flatten() {
                                let path = entry.path();
                                let run_id = path
                                    .file_stem()
                                    .map(|s| s.to_string_lossy().to_string())
                                    .unwrap_or_default();
                                if run_id.is_empty() {
                                    continue;
                                }
                                let aborted = registry.abort(&run_id);
                                let _ = std::fs::remove_file(&path);
                                if aborted {
                                    eprintln!("[gateway] aborted run {}", run_id);
                                }
                            }
                        }
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    }
                });
            }

            // Main loop: route messages and responses
            loop {
                tokio::select! {
                    Some(msg) = msg_rx.recv() => {
                        let handler = router_handler.clone();
                        let resp = resp_tx.clone();
                        tokio::spawn(async move {
                            handler.route(msg, &resp).await;
                        });
                    }
                    Some(response) = resp_rx.recv() => {
                        let surface = response.surface.clone();
                        let mut delivered = false;
                        for transport in &transports {
                            if transport.name() == surface {
                                delivered = true;
                                if let Err(e) = transport.send(response.clone()).await {
                                    eprintln!("Failed to send {} response: {}", surface, e);
                                }
                                break;
                            }
                        }
                        if !delivered {
                            eprintln!("No gateway transport for surface: {}", surface);
                        }
                    }
                    _ = tokio::signal::ctrl_c() => {
                        println!("\nStopping gateway...");
                        for transport in &transports {
                            let _ = transport.stop().await;
                        }
                        let _ = remove_gateway_pid(state_dir);
                        println!("Gateway stopped.");
                        break;
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(60)) => {
                        // Keep-alive
                    }
                }
            }
            Ok(())
        }
        sparrow::cli::GatewayAction::Status => {
            let pid = read_gateway_pid(state_dir);
            let pid_running = pid.is_some_and(process_is_running);
            let ws_open = gateway_ws_port_open();
            if pid_running || ws_open {
                println!("Gateway status: running");
                if let Some(pid) = read_gateway_pid(state_dir) {
                    println!("PID: {}", pid);
                }
                println!(
                    "WS API: {}",
                    if ws_open {
                        "online on ws://127.0.0.1:9338"
                    } else {
                        "not reachable"
                    }
                );
            } else {
                println!("Gateway status: not running");
                println!("Start with: sparrow gateway start");
            }
            Ok(())
        }
        sparrow::cli::GatewayAction::Health => {
            let pid = read_gateway_pid(state_dir);
            let pid_running = pid.is_some_and(process_is_running);
            let ws_open = gateway_ws_port_open();
            println!("Gateway health");
            println!(
                "  pid_file : {}",
                if pid.is_some() { "present" } else { "absent" }
            );
            println!(
                "  process  : {}",
                if pid_running { "running" } else { "stopped" }
            );
            println!(
                "  ws       : {}",
                if ws_open { "online" } else { "offline" }
            );
            println!(
                "  sessions : {}",
                config.state_dir.join("sessions.db").display()
            );
            if pid.is_some() && !pid_running && !ws_open {
                println!("  warning  : stale gateway pid file");
            }
            Ok(())
        }
        sparrow::cli::GatewayAction::Abort { run } => {
            let abort_dir = state_dir.join("gateway-abort");
            std::fs::create_dir_all(&abort_dir)?;
            std::fs::write(
                abort_dir.join(format!("{}.abort", sanitize_file_component(&run))),
                chrono::Utc::now().to_rfc3339(),
            )?;
            println!("Gateway abort requested for run '{}'.", run);
            println!("Abort signal: {}", abort_dir.display());
            Ok(())
        }
        sparrow::cli::GatewayAction::Stop => {
            match read_gateway_pid(state_dir) {
                Some(pid) if process_is_running(pid) => {
                    stop_gateway_process(pid)?;
                    let _ = remove_gateway_pid(state_dir);
                    println!("Gateway stop requested for PID {}.", pid);
                }
                Some(pid) => {
                    let _ = remove_gateway_pid(state_dir);
                    println!("Gateway PID {} was stale; cleaned status file.", pid);
                }
                None => {
                    println!("Gateway status: not running");
                }
            }
            Ok(())
        }
    }
}

fn gateway_pid_path(state_dir: &std::path::Path) -> std::path::PathBuf {
    state_dir.join("gateway.pid")
}

fn write_gateway_pid(state_dir: &std::path::Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(state_dir)?;
    std::fs::write(gateway_pid_path(state_dir), std::process::id().to_string())?;
    Ok(())
}

fn read_gateway_pid(state_dir: &std::path::Path) -> Option<u32> {
    std::fs::read_to_string(gateway_pid_path(state_dir))
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
}

fn remove_gateway_pid(state_dir: &std::path::Path) -> std::io::Result<()> {
    let path = gateway_pid_path(state_dir);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

fn sanitize_file_component(value: &str) -> String {
    let cleaned = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    if cleaned.is_empty() {
        "run".into()
    } else {
        cleaned
    }
}

fn gateway_ws_port_open() -> bool {
    std::net::TcpStream::connect_timeout(
        &"127.0.0.1:9338".parse().expect("valid socket address"),
        std::time::Duration::from_millis(250),
    )
    .is_ok()
}

fn process_is_running(pid: u32) -> bool {
    #[cfg(windows)]
    {
        std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV", "/NH"])
            .output()
            .map(|out| {
                let stdout = String::from_utf8_lossy(&out.stdout);
                out.status.success() && stdout.contains(&pid.to_string())
            })
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

fn stop_gateway_process(pid: u32) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        let status = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status()?;
        if !status.success() {
            anyhow::bail!("taskkill failed for PID {}", pid);
        }
    }
    #[cfg(not(windows))]
    {
        let status = std::process::Command::new("kill")
            .arg(pid.to_string())
            .status()?;
        if !status.success() {
            anyhow::bail!("kill failed for PID {}", pid);
        }
    }
    Ok(())
}

fn handle_sessions(
    action: sparrow::cli::SessionAction,
    state_dir: &std::path::Path,
) -> anyhow::Result<()> {
    let store = sparrow::runtime::session::SessionStore::open(&state_dir.join("sessions.db"))?;
    match action {
        sparrow::cli::SessionAction::List => {
            let sessions = store.list();
            if sessions.is_empty() {
                println!("No sessions stored.");
            } else {
                println!("Sessions ({}):", sessions.len());
                for session in sessions {
                    println!(
                        "  {} | status:{} | updated:{} | {} bytes",
                        session.id,
                        session.status,
                        session.updated_at,
                        session.messages_json.len()
                    );
                }
            }
        }
        sparrow::cli::SessionAction::Export { id, path } => {
            let Some(session) = store.load(&id) else {
                anyhow::bail!("session '{}' not found", id);
            };
            let output = path.unwrap_or_else(|| {
                state_dir.join(format!("session-{}.json", sanitize_file_component(&id)))
            });
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&output, serde_json::to_string_pretty(&session)?)?;
            println!("Exported session '{}' to {}", id, output.display());
        }
        sparrow::cli::SessionAction::Cleanup { older_than_days } => {
            let cutoff = chrono::Utc::now().timestamp() - (older_than_days as i64 * 86_400);
            let mut removed = 0usize;
            for session in store.list() {
                if session.updated_at < cutoff {
                    store.delete(&session.id)?;
                    removed += 1;
                }
            }
            println!(
                "Removed {} session(s) older than {} day(s).",
                removed, older_than_days
            );
        }
        sparrow::cli::SessionAction::Search { query, limit } => {
            let fts = sparrow::memory::fts::SessionSearch::open(state_dir.join("sessions_fts.db"))?;
            let hits = fts.search(&query, limit)?;
            if hits.is_empty() {
                println!("No results for \"{}\".", query);
            } else {
                println!("Results for \"{}\" ({}):\n", query, hits.len());
                for hit in &hits {
                    let title = hit.title.as_deref().unwrap_or("(untitled)");
                    println!("  {} — {}", hit.session_id, title);
                    println!("    {}", hit.snippet);
                    println!();
                }
            }
        }
    }
    Ok(())
}

// ─── Profile commands ───────────────────────────────────────────────────────────

fn handle_profile(
    action: sparrow::cli::ProfileAction,
    config_dir: &std::path::PathBuf,
    state_dir: &std::path::PathBuf,
) -> anyhow::Result<()> {
    match action {
        sparrow::cli::ProfileAction::Create { name } => {
            let profile_dir = config_dir.join("profiles").join(&name);
            std::fs::create_dir_all(&profile_dir)?;
            // Copy default config as starting point
            let default_config = config_dir.join("config.toml");
            if default_config.exists() {
                std::fs::copy(&default_config, profile_dir.join("config.toml"))?;
            }
            std::fs::create_dir_all(state_dir.join("profiles").join(&name))?;
            println!("Profile '{}' created.", name);
            println!("Config: {:?}", profile_dir.join("config.toml"));
            println!("Use: sparrow --profile {} <command>", name);
        }
        sparrow::cli::ProfileAction::List => {
            let profiles_dir = config_dir.join("profiles");
            let active_profile = std::fs::read_to_string(config_dir.join("active_profile"))
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            if !profiles_dir.exists() {
                println!("No profiles yet. Create one with: sparrow profile create <name>");
                return Ok(());
            }
            println!("Profiles:");
            if let Ok(entries) = std::fs::read_dir(&profiles_dir) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        if let Some(name) = entry.file_name().to_str() {
                            let marker = if active_profile.as_deref() == Some(name) {
                                "*"
                            } else {
                                " "
                            };
                            println!("{} {}", marker, name);
                        }
                    }
                }
            }
        }
        sparrow::cli::ProfileAction::Use { name } => {
            let profile_config = config_dir.join("profiles").join(&name).join("config.toml");
            if !profile_config.exists() {
                anyhow::bail!("Profile '{}' not found. Create it first.", name);
            }
            std::fs::write(config_dir.join("active_profile"), &name)?;
            println!("Active profile is now '{}'.", name);
            println!("Config: {}", profile_config.display());
        }
    }
    Ok(())
}

// ─── Import command ─────────────────────────────────────────────────────────────

fn handle_memory(
    action: sparrow::cli::MemoryAction,
    memory: &Arc<dyn Memory>,
    state_dir: &std::path::Path,
) -> anyhow::Result<()> {
    match action {
        sparrow::cli::MemoryAction::List => {
            let facts = memory.all_facts();
            let stats = memory.memory_stats();
            println!(
                "Memory docs: MEMORY.md {}/{} chars, USER.md {}/{} chars",
                stats.memory_chars, stats.memory_limit, stats.user_chars, stats.user_limit
            );
            if facts.is_empty() {
                println!("No facts stored. Facts are auto-distilled from successful runs.");
            } else {
                println!("Stored facts ({}):", facts.len());
                for f in &facts {
                    println!("  {}  {}: {}", f.id, f.key, f.value);
                }
            }
        }
        sparrow::cli::MemoryAction::Forget { id } => {
            memory.forget(&id)?;
            println!("Fact '{}' forgotten.", id);
        }
        sparrow::cli::MemoryAction::Add { key, value } => {
            let fact = sparrow::memory::Fact {
                id: uuid::Uuid::new_v4().to_string(),
                key,
                value,
                created_at: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                updated_at: chrono::Utc::now().format("%Y-%m-%d").to_string(),
            };
            memory.remember(fact)?;
            println!("Fact added.");
        }
        sparrow::cli::MemoryAction::Replace { id, key, value } => {
            let fact = sparrow::memory::Fact {
                id: id.clone(),
                key,
                value,
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            };
            memory.remember(fact)?;
            println!("Fact '{}' replaced.", id);
        }
        sparrow::cli::MemoryAction::Recall { query, limit } => {
            let facts = memory.recall(&query, limit);
            if facts.is_empty() {
                println!("No facts match '{}'.", query);
            } else {
                println!("Matching facts ({}):", facts.len());
                for f in &facts {
                    println!("  {}  {}: {}", f.id, f.key, f.value);
                }
            }
        }
        sparrow::cli::MemoryAction::Consolidate => {
            memory.consolidate_memory()?;
            println!("Memory consolidated into bounded MEMORY.md/USER.md docs.");
        }
        sparrow::cli::MemoryAction::Docs => {
            let mut found = false;
            for kind in [
                sparrow::memory::MemoryDocKind::Memory,
                sparrow::memory::MemoryDocKind::User,
            ] {
                if let Some(doc) = memory.memory_doc(kind) {
                    found = true;
                    println!("## {} ({})", kind.as_str(), doc.updated_at);
                    println!("{}", doc.content);
                    println!();
                }
            }
            if !found {
                println!("No MEMORY.md/USER.md docs stored yet.");
            }
        }
        sparrow::cli::MemoryAction::Search { query, limit } => {
            let store =
                sparrow::runtime::session::SessionStore::open(&state_dir.join("sessions.db"))?;
            let hits = store.search(&query, limit);
            if hits.is_empty() {
                println!("No sessions match '{}'.", query);
            } else {
                for hit in hits {
                    println!(
                        "{} #{} [{}] {}",
                        hit.session_id,
                        hit.turn_index,
                        hit.role,
                        hit.text.replace('\n', " ")
                    );
                }
            }
        }
        sparrow::cli::MemoryAction::Scroll {
            session,
            around,
            before,
            after,
        } => {
            let store =
                sparrow::runtime::session::SessionStore::open(&state_dir.join("sessions.db"))?;
            if let Some(slice) = store.scroll(&session, around, before, after) {
                for (offset, msg) in slice.messages.iter().enumerate() {
                    let idx = slice.start + offset;
                    let text = msg
                        .content
                        .iter()
                        .filter_map(|block| match block {
                            sparrow::provider::ContentBlock::Text { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    println!("#{} [{}] {}", idx, msg.role, text.replace('\n', " "));
                }
            } else {
                println!("Session '{}' not found.", session);
            }
        }
        sparrow::cli::MemoryAction::Graph { action } => handle_memory_graph(action, memory)?,
    }
    Ok(())
}

fn handle_memory_graph(
    action: sparrow::cli::GraphAction,
    memory: &Arc<dyn Memory>,
) -> anyhow::Result<()> {
    use sparrow::memory::{GraphDirection, GraphEdge, GraphNode};
    let now = chrono::Utc::now().to_rfc3339();
    match action {
        sparrow::cli::GraphAction::UpsertNode {
            id,
            label,
            kind,
            properties,
        } => {
            let properties = parse_json_properties(&properties)?;
            memory.upsert_graph_node(GraphNode {
                id: id.clone(),
                label,
                kind,
                properties,
                created_at: now.clone(),
                updated_at: now,
            })?;
            println!("Graph node stored: {}", id);
        }
        sparrow::cli::GraphAction::UpsertEdge {
            from_id,
            relation,
            to_id,
            id,
            weight,
            properties,
        } => {
            let edge_id = id.unwrap_or_else(|| format!("{}:{}:{}", from_id, relation, to_id));
            let properties = parse_json_properties(&properties)?;
            memory.upsert_graph_edge(GraphEdge {
                id: edge_id.clone(),
                from_id,
                to_id,
                relation,
                weight,
                properties,
                created_at: now.clone(),
                updated_at: now,
            })?;
            println!("Graph edge stored: {}", edge_id);
        }
        sparrow::cli::GraphAction::Get { id } => {
            if let Some(node) = memory.graph_node(&id) {
                println!("{}", serde_json::to_string_pretty(&node)?);
            } else {
                println!("Graph node '{}' not found.", id);
            }
        }
        sparrow::cli::GraphAction::Neighbors {
            id,
            direction,
            limit,
        } => {
            let rows = memory.graph_neighbors(&id, GraphDirection::parse(&direction), limit);
            println!("{}", serde_json::to_string_pretty(&rows)?);
        }
        sparrow::cli::GraphAction::Search { query, limit } => {
            let nodes = memory.search_graph(&query, limit);
            println!("{}", serde_json::to_string_pretty(&nodes)?);
        }
        sparrow::cli::GraphAction::Export => {
            println!("{}", serde_json::to_string_pretty(&memory.graph_export())?);
        }
        sparrow::cli::GraphAction::DeleteNode { id } => {
            memory.delete_graph_node(&id)?;
            println!("Graph node deleted: {}", id);
        }
        sparrow::cli::GraphAction::DeleteEdge { id } => {
            memory.delete_graph_edge(&id)?;
            println!("Graph edge deleted: {}", id);
        }
        sparrow::cli::GraphAction::SyncNeo4j => {
            let graph = memory.graph_export();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            let statements =
                runtime.block_on(sparrow::tools::knowledge_graph::sync_graph_to_neo4j(&graph))?;
            println!(
                "Synced graph to Neo4j: {} nodes, {} edges, {} statements",
                graph.nodes.len(),
                graph.edges.len(),
                statements
            );
        }
    }
    Ok(())
}

fn parse_json_properties(raw: &str) -> anyhow::Result<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_str(raw)
        .map_err(|e| anyhow::anyhow!("properties must be valid JSON object: {}", e))?;
    if !value.is_object() {
        anyhow::bail!("properties must be a JSON object");
    }
    Ok(value)
}

fn handle_full_import(source: sparrow::cli::ImportSource) -> anyhow::Result<()> {
    use sparrow::onboarding::migration::Migration;
    match source {
        sparrow::cli::ImportSource::Openclaw { path } => {
            let src =
                path.unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".openclaw"));
            let result = Migration::import_openclaw(&src)?;
            println!(
                "Imported from OpenClaw: {} agents, {} skills, {} cron jobs",
                result.agents, result.skills, result.cron_jobs
            );
        }
    }
    Ok(())
}

// ─── Setup command ──────────────────────────────────────────────────────────────

async fn handle_setup(
    config: &sparrow::config::Config,
    store: &FsConfigStore,
) -> anyhow::Result<()> {
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

    let effective = effective_provider_configs(config);
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
                let auth = sparrow::auth::store::ChainedAuthStore::new(next.config_dir.clone());
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

// ─── JSON NDJSON run ────────────────────────────────────────────────────────────

fn current_repo_head() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let head = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if head.is_empty() { None } else { Some(head) }
}

fn redacted_config_snapshot(config: &sparrow::config::Config) -> serde_json::Value {
    /// Returns true if `value` matches a vendor-specific secret pattern.
    /// Each pattern is anchored on a recognisable prefix and a plausible
    /// length so we don't false-positive UUIDs, hex hashes, or commit SHAs.
    fn looks_like_known_secret(value: &str) -> bool {
        let v = value.trim();
        // OpenAI / OpenRouter / fine-tuned-org keys: sk-..., sk-or-..., sk-proj-...
        if v.starts_with("sk-") && v.len() >= 20 {
            return true;
        }
        // Anthropic: sk-ant-api03-..., sk-ant-...
        if v.starts_with("sk-ant-") && v.len() >= 20 {
            return true;
        }
        // Groq, NVIDIA NIM, OpenRouter, DeepSeek, Mistral, xAI variants
        if (v.starts_with("gsk_")
            || v.starts_with("nvapi-")
            || v.starts_with("xai-")
            || v.starts_with("mr-"))
            && v.len() >= 20
        {
            return true;
        }
        // GitHub personal / fine-grained / app tokens
        if (v.starts_with("ghp_")
            || v.starts_with("gho_")
            || v.starts_with("ghu_")
            || v.starts_with("ghs_")
            || v.starts_with("ghr_")
            || v.starts_with("github_pat_"))
            && v.len() >= 30
        {
            return true;
        }
        // GitLab personal access tokens
        if v.starts_with("glpat-") && v.len() >= 20 {
            return true;
        }
        // Slack: xoxb-/xoxa-/xoxp-/xoxs-, plus webhook URLs
        if (v.starts_with("xoxb-")
            || v.starts_with("xoxa-")
            || v.starts_with("xoxp-")
            || v.starts_with("xoxs-"))
            && v.len() >= 20
        {
            return true;
        }
        if v.starts_with("https://hooks.slack.com/") {
            return true;
        }
        // AWS access key id (AKIA/ASIA + 16 base32 chars = 20 total)
        if (v.starts_with("AKIA") || v.starts_with("ASIA"))
            && v.len() == 20
            && v.chars().all(|c| c.is_ascii_alphanumeric())
        {
            return true;
        }
        // Stripe live/test secret keys
        if (v.starts_with("sk_live_") || v.starts_with("sk_test_") || v.starts_with("rk_live_"))
            && v.len() >= 24
        {
            return true;
        }
        // Google API keys
        if v.starts_with("AIza") && v.len() >= 35 && v.len() <= 45 {
            return true;
        }
        // JWT (header.payload.signature, all base64url)
        if v.matches('.').count() == 2 && v.len() >= 30 {
            let parts: Vec<&str> = v.split('.').collect();
            if parts.len() == 3
                && parts.iter().all(|p| {
                    p.chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                })
                && v.starts_with("eyJ")
            // JWT header always decodes to {"...
            {
                return true;
            }
        }
        false
    }

    /// Heuristic for *any* high-entropy-looking string in a secret-named field.
    /// We require length >= 32 AND mixed case OR digits, so a label like
    /// "API_KEY_NAME" stays visible while a real opaque key gets masked.
    fn looks_like_opaque_secret(value: &str) -> bool {
        let v = value.trim();
        if v.len() < 32 {
            return false;
        }
        let has_lower = v.chars().any(|c| c.is_ascii_lowercase());
        let has_upper = v.chars().any(|c| c.is_ascii_uppercase());
        let has_digit = v.chars().any(|c| c.is_ascii_digit());
        // Reject pure-uppercase identifiers and pure-hex hashes that look like
        // commit SHAs / UUIDs.
        if !has_lower && !has_digit {
            return false; // looks like an UPPER_SNAKE identifier
        }
        let entropy_chars = has_lower as u8 + has_upper as u8 + has_digit as u8;
        entropy_chars >= 2
    }

    fn redact(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, val) in map.iter_mut() {
                    let key_lc = key.to_lowercase();
                    let key_is_secret = key_lc.contains("key")
                        || key_lc.contains("token")
                        || key_lc.contains("secret")
                        || key_lc.contains("password")
                        || key_lc.contains("passwd")
                        || key_lc.contains("auth")
                        || key_lc.contains("credential")
                        || key_lc.contains("apikey");
                    if key_is_secret {
                        match val {
                            serde_json::Value::String(s) => {
                                if looks_like_known_secret(s) || looks_like_opaque_secret(s) {
                                    *val = serde_json::Value::String("<redacted>".into());
                                }
                                // else: probably a placeholder label, leave visible.
                                continue;
                            }
                            serde_json::Value::Null => continue,
                            // Nested object/array under a "secret" key: redact aggressively.
                            _ => {
                                *val = serde_json::Value::String("<redacted>".into());
                                continue;
                            }
                        }
                    }
                    redact(val);
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    redact(item);
                }
            }
            serde_json::Value::String(s) if looks_like_known_secret(s) => {
                *value = serde_json::Value::String("<redacted>".into());
            }
            _ => {}
        }
    }

    let mut snapshot = serde_json::to_value(config).unwrap_or_else(|_| serde_json::json!({}));
    redact(&mut snapshot);
    snapshot
}

async fn run_task_json(
    task: &str,
    config: &sparrow::config::Config,
    memory: Arc<dyn Memory>,
    recorder: Arc<FsRecorder>,
    skills: Arc<dyn SkillLibrary>,
) -> anyhow::Result<()> {
    use sparrow::engine::Engine;
    use sparrow::router::BasicRouter;

    let providers = build_provider_brains(config, &memory, false);

    let router = Arc::new(BasicRouter::new(config, providers));
    let engine = Engine::new(router, config.clone())
        .with_memory(memory)
        .with_skills(skills);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    let task_for_recording = task.to_string();
    let config_snapshot = redacted_config_snapshot(config);
    let repo_head = current_repo_head();
    let print_handle = tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        use tokio::io::AsyncWriteExt;
        while let Some(event) = rx.recv().await {
            if let sparrow::event::Event::RunStarted { run, agent, .. } = &event {
                recorder.start_run(
                    run.0.clone(),
                    RunInputs {
                        task: task_for_recording.clone(),
                        config_snapshot: config_snapshot.clone(),
                        model_id: "router-selected".into(),
                        repo_head: repo_head.clone(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        agent: agent.clone(),
                    },
                );
            }
            recorder.record(&event);
            if let sparrow::event::Event::RunFinished { run, .. } = &event {
                let _ = recorder.finalize(&run.0);
            }
            let line = sparrow::tools::extras::ndjson_output(&event);
            let _ = stdout.write_all(line.as_bytes()).await;
        }
    });

    let task_obj = sparrow::engine::Task {
        description: task.to_string(),
        context: vec![],
    };
    let outcome = engine.drive(task_obj, tx).await?;
    print_handle.await?;

    // Structured exit codes for CI/hook/script consumption:
    //   0  = completed successfully
    //   1  = generic error
    //   62 = budget cap exceeded
    //   63 = denied by autonomy / approval
    //   64 = timeout / interrupt
    let exit_code = match outcome.status.as_str() {
        "completed" => 0,
        "denied" => 63,
        "waiting_for_approval" => 63,
        s if s.contains("budget") => 62,
        s if s.contains("timeout") || s.contains("interrupt") => 64,
        s if s.starts_with("error") || s.contains("error") => 1,
        _ => 0,
    };

    if exit_code != 0 {
        std::process::exit(exit_code);
    }

    Ok(())
}

// ─── WebView console ───────────────────────────────────────────────────────────

/// Pull the WebView-internal protocol prefixes off the start of a task string.
///
/// The WebView wraps user input with metadata it cannot pass as a separate
/// channel:
///   `__agent:NAME__ROLE__BASE64_PERSONALITY__ __model:MODEL__ <user text>`
///
/// Either, both, or neither prefix may be present. Whatever survives at the
/// end is the actual prompt the user typed and must be the only string we:
///   - send to the LLM,
///   - echo to the UI,
///   - persist in conversation history.
///
/// Returns (clean_task, optional identity, optional model override).
fn extract_webview_protocol_prefixes(
    raw: &str,
) -> (String, Option<sparrow::engine::Identity>, Option<String>) {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    let mut remaining = raw.to_string();
    let mut identity: Option<sparrow::engine::Identity> = None;
    let mut model_override: Option<String> = None;

    // __agent:NAME__ROLE__BASE64__ <rest>
    if let Some(rest) = remaining.strip_prefix("__agent:") {
        if let Some((name, after_name)) = rest.split_once("__") {
            if let Some((role, after_role)) = after_name.split_once("__") {
                if let Some((b64, after_b64)) = after_role.split_once("__ ") {
                    let personality =
                        String::from_utf8(STANDARD.decode(b64.as_bytes()).unwrap_or_default())
                            .unwrap_or_default();
                    identity = Some(sparrow::engine::Identity {
                        name: name.to_string(),
                        role: role.to_string(),
                        personality,
                    });
                    remaining = after_b64.to_string();
                }
            }
        }
    }

    // __model:M__ <rest>
    if let Some(rest) = remaining.strip_prefix("__model:") {
        if let Some((model, after_model)) = rest.split_once("__ ") {
            model_override = Some(model.to_string());
            remaining = after_model.to_string();
        }
    }

    (remaining, identity, model_override)
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod webview_protocol_tests {
    use super::extract_webview_protocol_prefixes;
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    #[test]
    fn plain_task_is_unchanged() {
        let (clean, id, model) = extract_webview_protocol_prefixes("hello world");
        assert_eq!(clean, "hello world");
        assert!(id.is_none());
        assert!(model.is_none());
    }

    #[test]
    fn agent_prefix_is_stripped_and_decoded() {
        let b64 = STANDARD.encode(b"sarcastic helper");
        let raw = format!("__agent:nova__assistant__{}__ tu es en quel version?", b64);
        let (clean, id, model) = extract_webview_protocol_prefixes(&raw);
        assert_eq!(clean, "tu es en quel version?");
        let id = id.expect("identity extracted");
        assert_eq!(id.name, "nova");
        assert_eq!(id.role, "assistant");
        assert_eq!(id.personality, "sarcastic helper");
        assert!(model.is_none());
    }

    #[test]
    fn model_only_prefix_is_stripped() {
        let (clean, id, model) =
            extract_webview_protocol_prefixes("__model:deepseek-v4-pro__ run tests");
        assert_eq!(clean, "run tests");
        assert!(id.is_none());
        assert_eq!(model.as_deref(), Some("deepseek-v4-pro"));
    }

    #[test]
    fn agent_and_model_together_are_both_stripped() {
        // This is the exact shape the WebView sends today and the one that
        // was leaking the base64 personality + model id into the chat.
        let b64 = STANDARD.encode(b"P");
        let raw = format!(
            "__agent:nova__personal assistant__{}__ __model:deepseek-v4-pro__ ?",
            b64
        );
        let (clean, id, model) = extract_webview_protocol_prefixes(&raw);
        assert_eq!(clean, "?");
        assert_eq!(id.as_ref().unwrap().name, "nova");
        assert_eq!(model.as_deref(), Some("deepseek-v4-pro"));
    }

    #[test]
    fn malformed_agent_prefix_is_left_alone() {
        // Defensive: a half-formed prefix must NOT silently drop the user's
        // text. Better to send the raw to the LLM than to swallow it.
        let raw = "__agent:nova__broken-without-base64";
        let (clean, id, _) = extract_webview_protocol_prefixes(raw);
        assert_eq!(clean, raw);
        assert!(id.is_none());
    }
}

async fn handle_webview(
    config: &sparrow::config::Config,
    memory: Arc<dyn Memory>,
    _scheduler: Arc<MemoryScheduler>,
    recorder: Arc<FsRecorder>,
    skills: Arc<dyn SkillLibrary>,
    agent_store: Option<Arc<dyn AgentStore>>,
    port: u16,
) -> anyhow::Result<()> {
    use sparrow::engine::Engine;
    use sparrow::router::BasicRouter;
    use std::net::SocketAddr;
    use std::sync::RwLock;

    let (event_tx, _) = tokio::sync::broadcast::channel::<sparrow::event::Event>(1024);
    let (command_tx, mut command_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let shared_config = Arc::new(RwLock::new(config.clone()));
    let approvals = Arc::new(sparrow::console::WebApprovalBroker::new());

    // Persistent conversational context shared across runs — fixes the bug where
    // every model switch dropped prior turns. Capped to last 40 messages.
    // Backed by the SQLite SessionStore under a stable id so context AND the
    // session list survive a restart.
    let session_db_path = dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .or_else(dirs::data_dir)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("sparrow")
        .join("sessions.db");
    const WEBVIEW_SESSION_ID: &str = "webview";
    let session_store = sparrow::runtime::session::SessionStore::open(&session_db_path).ok();
    // Hydrate prior context from the persisted webview session.
    let initial_history: Vec<sparrow::provider::Msg> = session_store
        .as_ref()
        .and_then(|s| s.load(WEBVIEW_SESSION_ID))
        .and_then(|sess| serde_json::from_str(&sess.messages_json).ok())
        .unwrap_or_default();
    let conv_history: Arc<std::sync::Mutex<Vec<sparrow::provider::Msg>>> =
        Arc::new(std::sync::Mutex::new(initial_history));
    let conv_for_runs = conv_history.clone();
    let conv_for_capture = conv_history.clone();
    let session_store = session_store.map(Arc::new);
    let session_for_capture = session_store.clone();
    let session_for_loop = session_store.clone();

    let config_for_runs = shared_config.clone();
    let memory_for_runs = memory.clone();
    let skills_for_runs = skills.clone();
    let events_for_runs = event_tx.clone();
    let approvals_for_runs = approvals.clone();
    let recorder_for_runs = recorder.clone();
    tokio::spawn(async move {
        // Tracks the currently running task so we can inject mid-run messages
        // and abort on stop. `inject_tx` forwards user text into the live run;
        // `handle` lets us cancel it.
        let mut active: Option<(
            tokio::task::JoinHandle<()>,
            tokio::sync::mpsc::UnboundedSender<String>,
        )> = None;

        while let Some(mut task) = command_rx.recv().await {
            // FIRST thing in the loop: strip the WebView protocol prefixes
            // (`__agent:NAME__ROLE__BASE64__ `, `__model:M__ `) from `task`.
            //
            // If we don't, the raw protocol payload leaks to three places
            // downstream:
            //   1. the mid-run inject channel (the model sees the base64
            //      personality on every subsequent turn — PII to the provider)
            //   2. the UI `Message` event emitted for the user turn
            //      (the user sees the base64 blob in their own transcript)
            //   3. the persistent `conv_for_runs` history (every future run
            //      replays the same blob to the model)
            // All three are visible bugs we previously observed.
            let pending_identity: Option<sparrow::engine::Identity>;
            let pending_model_override: Option<String>;
            {
                let (clean, identity, model) = extract_webview_protocol_prefixes(&task);
                task = clean;
                pending_identity = identity;
                pending_model_override = model;
            }

            // Sentinel: clear conversation history without driving the engine.
            if task == "__reset_conversation__" {
                let mut guard = conv_for_runs.lock().expect("conv lock poisoned");
                guard.clear();
                drop(guard);
                if let Some(store) = &session_for_loop {
                    let _ = store.save("webview", &[], Some("WebView console"));
                }
                continue;
            }
            // Sentinel: switch the conversation context to a stored session.
            // Format: `__load_session__:<session_id>`.
            if let Some(target_id) = task.strip_prefix("__load_session__:") {
                if let Some(store) = &session_for_loop {
                    if let Some(session) = store.load(target_id) {
                        let parsed: Vec<sparrow::provider::Msg> =
                            serde_json::from_str(&session.messages_json).unwrap_or_default();
                        let turn_count = parsed.len();
                        {
                            let mut guard = conv_for_runs.lock().expect("conv lock poisoned");
                            *guard = parsed;
                        }
                        let _ = events_for_runs.send(sparrow::event::Event::Message {
                            run: sparrow::event::RunId("webview".into()),
                            role: "system".into(),
                            text: format!(
                                "loaded session {} ({} turns)",
                                session.name.as_deref().unwrap_or(&session.id),
                                turn_count
                            ),
                        });
                    } else {
                        let _ = events_for_runs.send(sparrow::event::Event::Error {
                            run: sparrow::event::RunId("webview".into()),
                            message: format!("session not found: {}", target_id),
                        });
                    }
                }
                continue;
            }
            // Sentinel: abort the active run.
            if task == "__stop__" {
                if let Some((handle, _)) = active.take() {
                    handle.abort();
                    let _ = events_for_runs.send(sparrow::event::Event::Message {
                        run: sparrow::event::RunId("webview".into()),
                        role: "system".into(),
                        text: "run aborted by user".into(),
                    });
                    let _ = events_for_runs.send(sparrow::event::Event::RunFinished {
                        run: sparrow::event::RunId("webview".into()),
                        outcome: sparrow::event::OutcomeSummary {
                            status: "aborted".into(),
                            diffs: vec![],
                            cost_usd: 0.0,
                            tokens: sparrow::event::TokenUsage {
                                input: 0,
                                output: 0,
                            },
                            cost_comparison: String::new(),
                        },
                    });
                }
                continue;
            }
            // If a run is still active, treat this message as a mid-run injection
            // instead of starting a new run.
            if let Some((handle, inject_tx)) = active.as_ref() {
                if !handle.is_finished() {
                    let _ = inject_tx.send(task.clone());
                    let _ = events_for_runs.send(sparrow::event::Event::Message {
                        run: sparrow::event::RunId("webview".into()),
                        role: "user".into(),
                        text: format!("(injected) {task}"),
                    });
                    // Also append to persistent history so future runs see it.
                    let mut guard = conv_for_runs.lock().expect("conv lock poisoned");
                    guard.push(sparrow::provider::Msg {
                        role: "user".into(),
                        content: vec![sparrow::provider::ContentBlock::Text { text: task.clone() }],
                    });
                    continue;
                }
            }
            // Previous run finished (or was never started) — drop the old handle.
            drop(active.take());
            let current_config = config_for_runs
                .read()
                .expect("config lock poisoned")
                .clone();
            let task_for_recording = task.clone();
            let config_snapshot = redacted_config_snapshot(&current_config);
            let repo_head = current_repo_head();
            let providers = build_provider_brains(&current_config, &memory_for_runs, false);
            let router = Arc::new(BasicRouter::new(&current_config, providers));
            let mut engine = Engine::new(router, current_config)
                .with_memory(memory_for_runs.clone())
                .with_skills(skills_for_runs.clone())
                .with_approval_handler(approvals_for_runs.clone());
            // Apply the identity extracted at the top of the loop. The
            // model_override is consumed inside the engine by the existing
            // `__model:` parser when present in the task description, so we
            // leave that prefix off the clean task — the engine never sees
            // it. If the WebView selected a model, we re-encode the
            // `__model:M__ ` prefix HERE so the engine can pick it up via
            // its existing strip logic without changing engine internals.
            if let Some(identity) = pending_identity.clone() {
                engine = engine.with_identity(identity);
            }
            if let Some(ref model) = pending_model_override {
                task = format!("__model:{}__ {}", model, task);
            }
            // Pull the persisted conversation history so a model switch never
            // drops prior turns. The Vec is cloned so the engine owns it for
            // the duration of the run; new turns get captured by the forwarder.
            let prior_context: Vec<sparrow::provider::Msg> = {
                let guard = conv_for_runs.lock().expect("conv lock poisoned");
                guard.clone()
            };
            // Push the user turn we are about to drive into the persisted log.
            {
                let mut guard = conv_for_runs.lock().expect("conv lock poisoned");
                guard.push(sparrow::provider::Msg {
                    role: "user".into(),
                    content: vec![sparrow::provider::ContentBlock::Text { text: task.clone() }],
                });
                // Cap at last 40 turns (~ generous; engine compaction handles tokens).
                if guard.len() > 40 {
                    let drop = guard.len() - 40;
                    guard.drain(..drop);
                }
            }
            let task_obj = sparrow::engine::Task {
                description: task,
                context: prior_context,
            };
            // Channel for injecting user messages mid-run.
            let (inject_tx, inject_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
            let (run_tx, mut run_rx) = tokio::sync::mpsc::unbounded_channel();
            let forward_tx = events_for_runs.clone();
            let recorder = recorder_for_runs.clone();
            let conv_capture = conv_for_capture.clone();
            let session_capture = session_for_capture.clone();
            let forward = tokio::spawn(async move {
                // Accumulate the assistant's streamed text for this run. The engine
                // emits the final response as ThinkingDelta events (not a Message),
                // so we concatenate the deltas and flush one assistant Msg into the
                // persistent conversation history when the run finishes. THIS is what
                // makes context survive across model switches and separate prompts.
                let mut assistant_buf = String::new();
                let mut reasoning_buf = String::new();
                while let Some(event) = run_rx.recv().await {
                    if let sparrow::event::Event::RunStarted { run, agent, .. } = &event {
                        recorder.start_run(
                            run.0.clone(),
                            RunInputs {
                                task: task_for_recording.clone(),
                                config_snapshot: config_snapshot.clone(),
                                model_id: "router-selected".into(),
                                repo_head: repo_head.clone(),
                                timestamp: chrono::Utc::now().to_rfc3339(),
                                agent: agent.clone(),
                            },
                        );
                    }
                    // Accumulate streamed assistant text.
                    if let sparrow::event::Event::ThinkingDelta { text, .. } = &event {
                        assistant_buf.push_str(text);
                    }
                    if let sparrow::event::Event::ReasoningDelta { text, .. } = &event {
                        reasoning_buf.push_str(text);
                    }
                    recorder.record(&event);
                    if let sparrow::event::Event::RunFinished { run, .. } = &event {
                        // Flush the accumulated assistant turn into the shared history.
                        let trimmed = assistant_buf.trim();
                        let snapshot = {
                            let mut guard = conv_capture.lock().expect("conv lock poisoned");
                            if !trimmed.is_empty() {
                                let mut content = Vec::new();
                                if !reasoning_buf.trim().is_empty() {
                                    content.push(sparrow::provider::ContentBlock::Reasoning {
                                        text: reasoning_buf.clone(),
                                    });
                                }
                                content.push(sparrow::provider::ContentBlock::Text {
                                    text: trimmed.to_string(),
                                });
                                guard.push(sparrow::provider::Msg {
                                    role: "assistant".into(),
                                    content,
                                });
                                if guard.len() > 40 {
                                    let drop = guard.len() - 40;
                                    guard.drain(..drop);
                                }
                            }
                            guard.clone()
                        };
                        // Persist the full conversation so it survives a restart and
                        // shows up in the /sessions panel.
                        if let Some(store) = &session_capture {
                            let _ = store.save("webview", &snapshot, Some("WebView console"));
                        }
                        let _ = recorder.finalize(&run.0);
                    }
                    let _ = forward_tx.send(event);
                }
            });

            // Spawn the run as a task so the command loop keeps receiving messages
            // (for mid-run injection and stop) while the engine works.
            let events_for_err = events_for_runs.clone();
            let run_handle = tokio::spawn(async move {
                if let Err(err) = engine
                    .drive_with_inject(
                        task_obj,
                        run_tx,
                        sparrow::event::RunId::new(),
                        Some(inject_rx),
                    )
                    .await
                {
                    let _ = events_for_err.send(sparrow::event::Event::Error {
                        run: sparrow::event::RunId("webview".into()),
                        message: format!("run failed: {}", err),
                    });
                }
                let _ = forward.await;
            });
            active = Some((run_handle, inject_tx));
        }
    });

    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;
    let url = format!("http://{}", addr);
    println!("WebView console: {}", url);
    println!("Press Ctrl+C to stop.\n");

    // Auto-open the browser for the local WebView cockpit.
    // Fire-and-forget — the server keeps running regardless.
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", &url])
            .spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(&url).spawn();
    }

    let server = WebViewServer::new(
        addr,
        event_tx,
        Some(command_tx),
        Some(shared_config),
        Some(approvals),
        Some(skills),
        Some(memory.clone()),
        agent_store,
    );
    server.serve().await?;

    Ok(())
}

// ─── Init command ──────────────────────────────────────────────────────────────

fn handle_init() -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let sparrow_dir = cwd.join(".sparrow");
    if sparrow_dir.exists() {
        println!("Project already initialized (.sparrow/ exists)");
        return Ok(());
    }
    std::fs::create_dir_all(&sparrow_dir)?;
    std::fs::create_dir_all(sparrow_dir.join("agents"))?;
    std::fs::create_dir_all(sparrow_dir.join("skills"))?;

    // Write team config template
    std::fs::write(
        sparrow_dir.join("team.toml"),
        r#"# Sparrow team config
# This file is shared via version control.
# Individual API keys go in ~/.config/sparrow/config.toml

[routing]
preferred = "nvidia"
free_first = true

[budget]
daily_per_seat_usd = 5.0

[org]
max_autonomy = "trusted"
blocked_paths = [".env", "*.pem", "secrets/"]
"#,
    )?;

    println!("Initialized .sparrow/ in {}", cwd.display());
    println!("  .sparrow/team.toml   — shared routing + budget + org policy");
    println!("  .sparrow/agents/     — team-shared agent definitions");
    println!("  .sparrow/skills/     — team-shared skills");
    println!("\nCommit .sparrow/ to your repo to share with the team.");
    Ok(())
}

// ─── Status command ────────────────────────────────────────────────────────────

fn handle_status(
    memory: &Arc<dyn Memory>,
    config: &sparrow::config::Config,
    scheduler: &Arc<sparrow::runtime::scheduler::MemoryScheduler>,
    recorder: &Arc<sparrow::runtime::recorder::FsRecorder>,
    state_dir: &std::path::PathBuf,
) -> anyhow::Result<()> {
    println!("Sparrow Status");
    println!("──────────────");

    // Budget & autonomy
    println!(
        "Budget     : ${:.2}/session  ${:.2}/day",
        config.budget.session_usd, config.budget.daily_usd
    );
    println!("Autonomy   : {:?}", config.defaults.autonomy);
    println!("Sandbox    : {}", config.defaults.sandbox);

    // Gateway up/down
    let gw_pid_path = state_dir.join("gateway.pid");
    let gw_ws_open = std::net::TcpStream::connect_timeout(
        &"127.0.0.1:9338".parse().unwrap(),
        std::time::Duration::from_millis(150),
    )
    .is_ok();
    let gw_pid_alive = gw_pid_path
        .exists()
        .then(|| {
            std::fs::read_to_string(&gw_pid_path)
                .ok()
                .and_then(|s| s.trim().parse::<u32>().ok())
        })
        .flatten()
        .map(|pid| {
            #[cfg(windows)]
            {
                std::process::Command::new("tasklist")
                    .args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV", "/NH"])
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
                    .unwrap_or(false)
            }
            #[cfg(not(windows))]
            {
                std::process::Command::new("kill")
                    .args(["-0", &pid.to_string()])
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
            }
        })
        .unwrap_or(false);
    println!(
        "Gateway    : {}",
        if gw_ws_open || gw_pid_alive {
            "running"
        } else {
            "stopped  (start with: sparrow gateway start)"
        }
    );

    // Scheduled jobs
    let jobs = scheduler.list();
    if jobs.is_empty() {
        println!("Cron jobs  : none scheduled");
    } else {
        println!("Cron jobs  : {} scheduled", jobs.len());
        for j in &jobs {
            let st = if j.enabled { "active" } else { "paused" };
            let next = j.next_run.as_deref().unwrap_or("pending");
            println!("  [{}] {}  cron:{}  next:{}", st, j.id, j.cron, next);
        }
    }

    // Recent transcripts
    let transcripts = recorder.list_transcripts();
    println!("Transcripts: {} total", transcripts.len());
    for id in transcripts.iter().rev().take(3) {
        if let Some(tr) = recorder.load(id) {
            println!(
                "  {} | {} events | {}",
                id,
                tr.events.len(),
                tr.inputs.task.chars().take(50).collect::<String>()
            );
        }
    }

    // Memory & model cache
    let mem_stats = memory.memory_stats();
    println!(
        "Memory     : {} facts | MEMORY.md {}/{} | USER.md {}/{} chars",
        mem_stats.facts,
        mem_stats.memory_chars,
        mem_stats.memory_limit,
        mem_stats.user_chars,
        mem_stats.user_limit
    );
    let total_discovered: usize = sparrow::config::providers::provider_registry()
        .iter()
        .map(|p| {
            memory
                .get_discovered_models(&p.id)
                .into_iter()
                .filter(|model| sparrow::provider::discovery::is_chat_model_id(model))
                .count()
        })
        .sum();
    let static_count: usize = sparrow::config::providers::provider_registry()
        .iter()
        .map(|p| p.models.len())
        .sum();
    println!(
        "Models     : {} static + {} discovered (cached 24h)",
        static_count, total_discovered
    );

    println!("\nRun 'sparrow doctor' for full diagnostics.");
    Ok(())
}
