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
use sparrow::agent::{AgentStore, FsAgentStore};
use sparrow::auth::{AuthStore, Credential};
use sparrow::autonomy::{Checkpoints, GitCheckpoints};
use sparrow::capabilities::{FsSkillLibrary, SkillLibrary};
use sparrow::cli::{Cli, Commands};
use sparrow::config::{ConfigStore, FsConfigStore, ProviderConfig};
use sparrow::console::WebViewServer;
use sparrow::memory::{Memory, SqliteMemory};
use sparrow::runtime::recorder::{FsRecorder, Recorder, Replayer, RunInputs};
use sparrow::runtime::scheduler::{MemoryScheduler, Scheduler};
use sparrow::tui::Tui;
// Cross-handler helpers still used directly by main's dispatcher.
use sparrow::cmd_handlers::handle_agent_cmd::{
    apply_cli_overrides, build_provider_brains, discover_and_cache_provider,
    migrate_inline_provider_keys, refresh_discovery_cache, run_tui,
};
use sparrow::cmd_handlers::handle_memory_graph_cmd::current_repo_head;
use sparrow::cmd_handlers::handle_permissions_cmd::run_swarm;
use sparrow::cmd_handlers::handle_run_task_cmd::redacted_config_snapshot;
use sparrow::cmd_handlers::prelude::{RunFlags, SessionMode};
use std::io::Write;
use std::sync::Arc;

fn main() -> anyhow::Result<()> {
    // The async entry point below is one giant state machine (every branch
    // of the CLI inlines its locals into the future's size). On Windows the
    // OS main thread caps at ~1 MB, which the future + tokio runtime
    // initialisation blow through in debug builds even when the future is
    // Box::pinned. Run the whole thing on a worker thread with an explicit
    // 16 MB stack — release builds are fine on any platform, so this is a
    // no-cost safety net.
    let worker = std::thread::Builder::new()
        .name("sparrow-main".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| -> anyhow::Result<()> {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            runtime.block_on(Box::pin(async_main()))
        })?;
    worker
        .join()
        .map_err(|_| anyhow::anyhow!("sparrow main thread panicked"))?
}

async fn async_main() -> anyhow::Result<()> {
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
            permissions: sparrow::permissions::PermissionConfig {
                store: sparrow::permissions::store::PermissionStore::load(&active_config_dir),
                ..Default::default()
            },
            hooks: Default::default(),
            theme: "captain".into(),
            config_dir: active_config_dir.clone(),
            state_dir: active_state_dir.clone(),
            forced_model: None,
        }
    });
    config.config_dir = active_config_dir.clone();
    config.state_dir = active_state_dir.clone();

    // Load persisted per-tool permission decisions from permissions.json.
    // Durable decisions (AllowAlways, Deny) survive across sessions.
    config.permissions.store =
        sparrow::permissions::store::PermissionStore::load(&config.config_dir);
    // Expire session-scoped decisions from previous sessions.
    config.permissions.store.expire_session_scoped();
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
            sparrow::cmd_handlers::setup_cmd::handle_setup(&config, &config_store).await?;
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
                    cli.bind.clone(),
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
                    sparrow::cmd_handlers::setup_cmd::handle_setup(&config, &config_store).await?;
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
                    cli.bind.clone(),
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
                cli.bind.clone(),
            )
            .await?;
        }
        Some(Commands::Daemon) => {
            sparrow::cmd_handlers::handle_daemon_cmd::handle_daemon(
                &config,
                memory.clone(),
                scheduler.clone(),
                recorder.clone(),
                skill_library.clone(),
            )
            .await?;
        }
        Some(Commands::Run {
            ref task,
            json: _json,
        }) => {
            {
                // NDJSON mode is currently a thin wrapper over the normal run
                // path: the recorder already writes a JSONL transcript per
                // run, so callers piping NDJSON can read $XDG_STATE/sparrow
                // /transcripts/<id>.jsonl. A streamed --json variant is in
                // the roadmap but the old standalone implementation was
                // dead code referencing renamed APIs.
                let flags = RunFlags {
                    session_mode: if cli.fresh {
                        SessionMode::Fresh
                    } else if cli.continue_last {
                        SessionMode::ContinueLast
                    } else {
                        SessionMode::Auto
                    },
                    assume_yes: cli.yes,
                };
                sparrow::cmd_handlers::handle_run_task_cmd::run_task(
                    task,
                    &config,
                    memory.clone(),
                    skill_library.clone(),
                    recorder.clone(),
                    None,
                    flags,
                )
                .await?;
            }
        }
        Some(Commands::Plan { ref task, json }) => {
            sparrow::cmd_handlers::handle_plan_cmd::handle_plan(
                task,
                &config,
                skill_library.clone(),
                json || cli.json,
            )?;
        }
        Some(Commands::Review {
            ref base,
            ref paths,
            dry_run,
        }) => {
            sparrow::cmd_handlers::handle_review_cmd::handle_review(
                base.clone(),
                paths.clone(),
                dry_run,
                &config,
                memory.clone(),
                skill_library.clone(),
                recorder.clone(),
            )
            .await?;
        }
        Some(Commands::Permissions { action }) => {
            sparrow::cmd_handlers::handle_permissions_cmd::handle_permissions(
                action,
                &config,
                &config_store,
            )?;
        }
        Some(Commands::Chat) => {
            sparrow::cmd_handlers::handle_chat_cmd::handle_chat(&config, memory.clone()).await?;
        }
        Some(Commands::Agent { action }) => {
            sparrow::cmd_handlers::handle_agent_cmd::handle_agent(
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
            sparrow::cmd_handlers::handle_skills_cmd::handle_skills(action, &skill_library)?;
        }
        Some(Commands::Plugins { action }) => {
            sparrow::cmd_handlers::handle_plugins_cmd::handle_plugins(action, &config_dir)?;
        }
        Some(Commands::Tools { action }) => {
            sparrow::cmd_handlers::handle_tools_cmd::handle_tools(action, &config_store)?;
        }
        Some(Commands::Security { action }) => {
            sparrow::cmd_handlers::handle_security_cmd::handle_security(action, &config)?;
        }
        Some(Commands::Github { action }) => {
            sparrow::cmd_handlers::handle_github_cmd::handle_github(action)?;
        }
        Some(Commands::Compact { task, out, json }) => {
            sparrow::cmd_handlers::handle_compact_cmd::handle_compact(task, out, json)?;
        }
        Some(Commands::Mcp { action }) => {
            sparrow::cmd_handlers::handle_mcp_cmd::handle_mcp(action, &config_dir).await?;
        }
        Some(Commands::Schedule {
            task,
            cron,
            autonomy,
            report,
        }) => {
            sparrow::cmd_handlers::handle_schedule_cmd::handle_schedule(
                &task, &cron, autonomy, &report, &scheduler,
            )
            .await?;
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
                sparrow::cmd_handlers::handle_replay_cmd::handle_replay(
                    &run_id,
                    &recorder,
                    &config,
                    memory.clone(),
                )
                .await?;
            }
        }
        Some(Commands::Gateway { action }) => {
            sparrow::cmd_handlers::handle_gateway_cmd::handle_gateway(
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
            sparrow::cmd_handlers::handle_sessions_cmd::handle_sessions(action, &active_state_dir)?;
        }
        Some(Commands::Model { set, list }) => {
            if list {
                refresh_discovery_cache(memory.clone(), &config, false, false).await;
                println!("Configured providers:");
                let effective = sparrow::config::effective_provider_configs(&config);
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
                    println!("  mode            : {}", config.routing.routing_mode);
                    println!(
                        "  auto_discover   : {}",
                        if config.routing.auto_discover {
                            "on"
                        } else {
                            "off"
                        }
                    );
                    match &config.routing.preferred_model {
                        Some(m) => println!("  preferred_model : {} (single model pinned)", m),
                        None => match &config.routing.preferred_provider {
                            Some(p) => {
                                println!("  preferred_provider : {} (all models pinned)", p)
                            }
                            None => {
                                println!("  preferred_provider : (none — per-tier policy active)")
                            }
                        },
                    }
                    println!("  Per-tier policy:");
                    let mut tiers: Vec<_> = config.routing.policy.iter().collect();
                    tiers.sort_by_key(|(k, _): &(&String, &String)| k.as_str());
                    for (tier, provider) in tiers {
                        println!("    {} -> {}", tier, provider);
                    }
                }
                sparrow::cli::RouteAction::Set { provider } => {
                    // Support provider/model syntax: "deepseek/deepseek-v4-pro"
                    let (provider_id, model) = if let Some((p, m)) = provider.split_once('/') {
                        (p.to_string(), Some(m.to_string()))
                    } else {
                        (provider.clone(), None)
                    };
                    let known = sparrow::config::providers::find_provider(&provider_id).is_some()
                        || !memory.get_discovered_models(&provider_id).is_empty();
                    if !known {
                        eprintln!(
                            "Unknown provider '{}'. Run 'sparrow model --list' to see options.",
                            provider_id
                        );
                    } else {
                        let mut updated = config.clone();
                        updated.routing.routing_mode = "manual".into();
                        updated.routing.preferred_provider = Some(provider_id);
                        updated.routing.preferred_model = model;
                        config_store.save(&updated)?;
                        if let Some(ref m) = updated.routing.preferred_model {
                            println!(
                                "🔒 Manual mode: pinned to model {} at {}",
                                m,
                                updated.routing.preferred_provider.as_ref().unwrap()
                            );
                        } else {
                            println!(
                                "🔒 Manual mode: pinned to provider {}",
                                updated.routing.preferred_provider.as_ref().unwrap()
                            );
                        }
                        println!("  All tiers will use this. Zero fallback.");
                        println!("  Run 'sparrow route auto' to restore automatic routing.");
                    }
                }
                sparrow::cli::RouteAction::Clear => {
                    let mut updated = config.clone();
                    updated.routing.preferred_provider = None;
                    updated.routing.preferred_model = None;
                    config_store.save(&updated)?;
                    println!(
                        "Preferred provider/model cleared. Per-tier routing policy is now active."
                    );
                }
                sparrow::cli::RouteAction::Manual => {
                    let mut updated = config.clone();
                    updated.routing.routing_mode = "manual".into();
                    config_store.save(&updated)?;
                    if updated.routing.preferred_provider.is_none() {
                        println!("🔒 Manual mode active. Choose a provider/model with:");
                        println!("  sparrow route set <provider>");
                        println!("  sparrow route set <provider>/<model>");
                    } else {
                        println!(
                            "🔒 Manual mode active. Current pin: {}",
                            updated.routing.preferred_provider.as_ref().unwrap()
                        );
                    }
                }
                sparrow::cli::RouteAction::Auto => {
                    let mut updated = config.clone();
                    updated.routing.routing_mode = "auto".into();
                    config_store.save(&updated)?;
                    println!(
                        " Auto mode restored. Tier-based routing + free_first fallback active."
                    );
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
                    sparrow::cmd_handlers::handle_auth_login_cmd::handle_auth_login(
                        &provider, client_id, &auth,
                    )
                    .await?;
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
                    println!("Checkpoints (newest last):");
                    for cp in &list {
                        let when = cp
                            .timestamp
                            .with_timezone(&chrono::Local)
                            .format("%Y-%m-%d %H:%M");
                        let short_id: String = cp.id.0.chars().take(8).collect();
                        // The auto-generated label just repeats the id — skip it.
                        let label = if cp.label.ends_with(&cp.id.0) {
                            String::new()
                        } else {
                            format!("  {}", cp.label)
                        };
                        println!("  {}  {}{}", when, short_id, label);
                    }
                    println!("\nRestore with: sparrow rewind <id>");
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
            println!(
                "All checks done. If something looks wrong: https://github.com/ucav/Sparrow/issues"
            );
        }
        Some(Commands::Update) => {
            println!("Checking for updates...");
            match sparrow::update::self_update() {
                Ok(msg) => println!("{}", msg),
                Err(e) => eprintln!("Update failed: {}", e),
            }
        }
        Some(Commands::Profile { action }) => {
            sparrow::cmd_handlers::handle_profile_cmd::handle_profile(
                action,
                &config_dir,
                &state_dir,
            )?;
        }
        Some(Commands::Import { source }) => {
            sparrow::cmd_handlers::import_cmd::handle_full_import(source)?;
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
                sparrow::cmd_handlers::setup_cmd::handle_setup(&config, &config_store).await?;
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
            sparrow::cmd_handlers::handle_init_cmd::handle_init()?;
        }
        Some(Commands::Status) => {
            sparrow::cmd_handlers::handle_status_cmd::handle_status(
                &(memory.clone() as Arc<dyn Memory>),
                &config,
                &scheduler,
                &recorder,
                &state_dir,
            )?;
        }
        Some(Commands::Memory { action }) => {
            sparrow::cmd_handlers::handle_memory_cmd::handle_memory(
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

// `redacted_config_snapshot` moved to cmd_handlers::handle_run_task_cmd so
// every handler can reach it via the prelude. main.rs imports it at the top.

// Note: an earlier `run_task_json` lived here but referenced renamed APIs
// (provider::traits::Provider, router::resolve_brain, the old single-brain
// Engine::new signature). It was removed; NDJSON-style consumers should
// read the per-run transcript file the recorder produces.

fn extract_webview_protocol_prefixes(input: &str) -> (String, Option<String>, Option<String>) {
    let re = regex::Regex::new(r"__model:([^_]+)__\s*").unwrap();
    if let Some(caps) = re.captures(input) {
        let model_ref = caps.get(1).unwrap().as_str();
        let (provider_id, model) =
            sparrow::cmd_handlers::handle_agent_cmd::parse_agent_model_ref(model_ref)
                .unwrap_or_else(|| ("custom".into(), model_ref.into()));
        let clean = re.replace(input, "").to_string();
        return (clean, Some(provider_id), Some(model));
    }

    let provider_re = regex::Regex::new(r"__provider:([^_]+)__\s*").unwrap();
    if let Some(caps) = provider_re.captures(input) {
        let provider_id = caps.get(1).unwrap().as_str().to_string();
        let clean = provider_re.replace(input, "").to_string();
        return (clean, Some(provider_id), None);
    }

    (input.to_string(), None, None)
}

// ─── WebView command ─────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
async fn handle_webview(
    config: &sparrow::config::Config,
    memory: Arc<dyn Memory>,
    _scheduler: Arc<MemoryScheduler>,
    recorder: Arc<FsRecorder>,
    skills: Arc<dyn SkillLibrary>,
    agent_store: Option<Arc<dyn AgentStore>>,
    port: u16,
    bind: Option<String>,
) -> anyhow::Result<()> {
    use sparrow::engine::Engine;
    use sparrow::router::BasicRouter;
    use std::sync::RwLock;

    // Resolve and validate the bind target up front (D1/D3). Refuse to start
    // over an already-running console (D2) before we touch any other state.
    let bind_target = sparrow::console::resolve_bind_addr(bind.as_deref(), port)?;
    if sparrow::console::console_already_running(port).await {
        anyhow::bail!(
            "Une console Sparrow tourne déjà sur http://127.0.0.1:{port}.\n\
             Ouvre-la dans ton navigateur, ou relance avec --port <AUTRE_PORT>."
        );
    }

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
            // Despite the name, `extract_webview_protocol_prefixes` returns
            // a provider id (e.g. "anthropic"), not a full Identity — keep
            // the binding type matching the function's actual return.
            let pending_identity: Option<String>;
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
                            duration_ms: None,
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
            if let Some(provider_id) = pending_identity.clone() {
                // The webview protocol carries a provider id (anthropic,
                // openai, …); surface it as an engine identity so the
                // `route:` line and persona stay consistent with the
                // picker the user clicked. Role/personality default.
                engine = engine.with_identity(sparrow::engine::Identity {
                    name: provider_id,
                    ..sparrow::engine::Identity::default()
                });
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

    let addr = bind_target.addr;
    let url = format!("http://127.0.0.1:{}", port);
    println!("WebView console: {}", url);
    if bind_target.is_public {
        println!(
            "⚠️  Sparrow écoute sur {} — accessible depuis le réseau local.\n\
             N'utilise ça que sur un réseau de confiance (clés, fichiers et agents y sont exposés).",
            addr
        );
    }
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
