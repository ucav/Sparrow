// src/cmd_handlers/handle_agent_cmd.rs
use super::prelude::*;
use sparrow::autonomy::Checkpoints; // for GitCheckpoints::rewind, a trait method
pub async fn handle_agent(
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
                // Agent runs keep their existing no-prompt behaviour.
                let flags = RunFlags {
                    assume_yes: true,
                    ..Default::default()
                };
                run_task(&task, config, memory, skills, recorder, Some(soul), flags).await?;
            } else {
                anyhow::bail!("Agent '{}' not found.", name);
            }
        }
        sparrow::cli::AgentAction::Mention { name, message } => {
            if let Some(soul) = store.get(&name) {
                let task = format!("@{} {}", soul.name, message);
                println!("Mentioning agent '{}': {}", soul.name, message);
                let flags = RunFlags {
                    assume_yes: true,
                    ..Default::default()
                };
                run_task(&task, config, memory, skills, recorder, Some(soul), flags).await?;
            } else {
                anyhow::bail!("Agent '{}' not found.", name);
            }
        }
    }
    Ok(())
}

pub fn looks_like_inline_secret(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with("sk-")
        || trimmed.starts_with("nvapi-")
        || trimmed.starts_with("gsk_")
        || trimmed.starts_with("sk-or-")
}

pub fn apply_cli_overrides(config: &mut sparrow::config::Config, cli: &Cli) {
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

pub fn migrate_inline_provider_keys(config: &mut sparrow::config::Config, store: &FsConfigStore) {
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

pub async fn refresh_discovery_cache(
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

pub async fn discover_and_cache_provider(
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

pub fn build_provider_brains(
    config: &sparrow::config::Config,
    memory: &Arc<dyn Memory>,
    warn: bool,
) -> std::collections::HashMap<String, Vec<Arc<dyn sparrow::provider::Brain>>> {
    let auth = sparrow::auth::store::ChainedAuthStore::new(config.config_dir.clone());
    let mut providers: std::collections::HashMap<String, Vec<Arc<dyn sparrow::provider::Brain>>> =
        std::collections::HashMap::new();

    for (name, pconfig) in sparrow::config::effective_provider_configs(config) {
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

pub async fn run_tui(
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
pub fn config_for_soul(config: &sparrow::config::Config, soul: &Soul) -> sparrow::config::Config {
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
pub fn parse_agent_model_ref(model_ref: &str) -> Option<(String, String)> {
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

pub fn push_unique_path(values: &mut Vec<std::path::PathBuf>, value: std::path::PathBuf) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

pub fn print_permission_policy(config: &sparrow::config::Config) {
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

pub fn list_or_empty(values: &[String]) -> String {
    if values.is_empty() {
        "(empty)".into()
    } else {
        values.join(", ")
    }
}

pub fn path_list_or_empty(values: &[std::path::PathBuf]) -> String {
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
