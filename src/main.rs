use clap::Parser;
use sparrow::agent::{AgentStore, FsAgentStore, Soul};
use sparrow::auth::{AuthStore, Credential};
use sparrow::autonomy::{Checkpoints, GitCheckpoints};
use sparrow::capabilities::{FsSkillLibrary, SkillLibrary};
use sparrow::capabilities::mcp::{BasicMcpClient, McpClient, Transport};
use sparrow::cli::{Cli, Commands};
use sparrow::config::{ConfigStore, FsConfigStore};
use sparrow::extras::{ChatSession, OAuthFlow, PipelineConfig, Profile, ReExecuter};
use sparrow::console::WebViewServer;
use sparrow::gateway::{
    GatewayMessage, GatewayResponse, GatewayTransport, MessageRouter,
};
use sparrow::gateway::telegram::TelegramTransport;
use sparrow::gateway::discord::DiscordTransport;
use sparrow::gateway::slack::SlackTransport;
use sparrow::gateway::ws::WebSocketApi;
use sparrow::gateway::extra_transports::{
    WhatsAppTransport, SignalTransport, EmailTransport,
    FeishuTransport, WeComTransport, QQBotTransport, TeamsTransport,
};
use sparrow::memory::{Fact, Memory, SqliteMemory};
use sparrow::runtime::recorder::{FsRecorder, Recorder, Replayer, RunInputs};
use sparrow::runtime::scheduler::{Job, MemoryScheduler, Scheduler};
use sparrow::tui::Tui;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sparrow=info".into()),
        )
        .init();

    let cli = Cli::parse();

    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("sparrow");
    let state_dir = dirs::state_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("sparrow");

    let config_store = FsConfigStore::new(config_dir.clone());
    let mut config = config_store.load().unwrap_or_else(|e| {
        eprintln!("Warning: could not load config: {}. Using defaults.", e);
        sparrow::config::Config {
            defaults: Default::default(),
            routing: Default::default(),
            budget: Default::default(),
            providers: Default::default(),
            surfaces: Default::default(),
            skills: Default::default(),
            theme: "captain".into(),
            config_dir: config_dir.clone(),
            state_dir: state_dir.clone(),
        }
    });
    migrate_inline_provider_keys(&mut config, &config_store);

    // Initialize memory (SQLite)
    let memory = Arc::new(
        SqliteMemory::open(&state_dir.join("sparrow.db"))
            .unwrap_or_else(|e| {
                eprintln!("Warning: could not open database: {}. Using in-memory fallback.", e);
                // In-memory fallback
                SqliteMemory::open(&std::path::PathBuf::from(":memory:")).unwrap()
            }),
    );

    // Initialize agent store
    let agent_store: Arc<dyn AgentStore> = Arc::new(
        FsAgentStore::new(config_dir.join("agents")).with_memory(memory.clone()),
    );

    // Initialize skill library
    let skills_dir = config_dir.join("skills");
    let skill_library: Arc<dyn SkillLibrary> = Arc::new(
        FsSkillLibrary::new(skills_dir).with_memory(memory.clone()),
    );

    // Initialize recorder (transcripts)
    let recorder = Arc::new(FsRecorder::new(state_dir.join("transcripts")));

    // Initialize scheduler
    let scheduler = Arc::new(
        MemoryScheduler::new().with_memory(memory.clone()),
    );

    match cli.command {
        None => {
            if cli.tui {
                let mut tui = Tui::new();
                tui.run()?;
            } else if cli.web {
                handle_webview(&config, memory.clone(), scheduler.clone(), recorder.clone(), skill_library.clone()).await?;
            } else {
                let mut tui = Tui::new();
                tui.run()?;
            }
        }
        Some(Commands::Tui) => {
            let mut tui = Tui::new();
            tui.run()?;
        }
        Some(Commands::Console) => {
            handle_webview(&config, memory.clone(), scheduler.clone(), recorder.clone(), skill_library.clone()).await?;
        }
        Some(Commands::Run { ref task }) => {
            if cli.json {
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
                run_task(task, &cli, &config, memory.clone(), agent_store.clone(), skill_library.clone()).await?;
            }
        }
        Some(Commands::Chat) => {
            handle_chat(&config, memory.clone()).await?;
        }
        Some(Commands::Agent { action }) => {
            handle_agent(action, &agent_store)?;
        }
        Some(Commands::Swarm { task }) => {
            run_swarm(&task, &config, memory.clone()).await?;
        }
        Some(Commands::Skills { action }) => {
            handle_skills(action, &skill_library)?;
        }
        Some(Commands::Mcp { action }) => {
            handle_mcp(action, &config_dir).await?;
        }
        Some(Commands::Schedule { task, cron, autonomy, report }) => {
            handle_schedule(&task, &cron, autonomy, &report, &scheduler).await?;
        }
        Some(Commands::Replay { run_id }) => {
            handle_replay(&run_id, &recorder, &config, memory.clone()).await?;
        }
        Some(Commands::Gateway { action }) => {
            handle_gateway(action, &state_dir, &config, memory.clone(), scheduler.clone(), recorder.clone()).await?;
        }
        Some(Commands::Model { set, list }) => {
            if list {
                println!("Configured providers:");
                for (name, pconfig) in &config.providers {
                    println!("  {} (adapter: {})", name, pconfig.adapter);
                    for model in &pconfig.models {
                        println!("    - {}", model);
                    }
                }
                if config.providers.is_empty() {
                    println!("  No providers configured.");
                    println!("  Run 'sparrow auth add <provider>' or set *_API_KEY env vars.");
                }
            }
            if let Some(route) = set {
                println!("Model routing set to: {} (apply in config.toml)", route);
            }
        }
        Some(Commands::Auth { action }) => {
            let auth = sparrow::auth::store::ChainedAuthStore::new(config.config_dir.clone());
            match action {
                sparrow::cli::AuthAction::List => {
                    let providers = auth.list();
                    if providers.is_empty() {
                        println!("No credentials stored.");
                        println!(
                            "Set env vars like ANTHROPIC_API_KEY, OPENAI_API_KEY, etc."
                        );
                    } else {
                        println!("Stored credentials for:");
                        for p in providers {
                            println!("  - {}", p);
                        }
                    }
                }
                sparrow::cli::AuthAction::Add { provider } => {
                    println!("Add credentials for: {}", provider);
                    println!(
                        "Set {}_API_KEY env variable or use 'sparrow config edit'",
                        provider.to_uppercase()
                    );
                }
                sparrow::cli::AuthAction::Rm { provider } => {
                    auth.remove(&provider)?;
                    println!("Removed credentials for: {}", provider);
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
        },
        Some(Commands::Rewind { id }) => {
            let cwd = std::env::current_dir().unwrap_or_default();
            let checkpoints = GitCheckpoints::new(cwd);
            match checkpoints.rewind(sparrow::event::CheckpointId(id.clone())) {
                Ok(()) => println!("Rewound to checkpoint: {}", id),
                Err(e) => eprintln!("Failed to rewind: {}", e),
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
            println!("Sandbox    : {}", config.defaults.sandbox);
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

            let facts = memory.all_facts();
            println!("Memory     : {} facts stored", facts.len());
            let agents = agent_store.list();
            println!("Agents     : {} defined", agents.len());
            for a in &agents {
                println!("  - {} ({})", a.name, a.role);
            }
            let skills = skill_library.all();
            println!("Skills     : {} in library", skills.len());
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
            handle_import(source)?;
        }
        Some(Commands::Setup) => {
            handle_setup(&config).await?;
        }
        Some(Commands::Config { edit }) => {
            if edit {
                let config_path = config.config_dir.join("config.toml");
                println!("Config file: {}", config_path.display());
                #[cfg(windows)]
                {
                    let _ = std::process::Command::new("notepad").arg(&config_path).spawn();
                }
                #[cfg(not(windows))]
                {
                    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".into());
                    let _ = std::process::Command::new(editor).arg(&config_path).spawn();
                }
            }
        }
        _ => {
            println!("Command not yet implemented in v1.");
            println!("Available: run, tui, model, auth, agent, checkpoint, rewind, swarm, skills, mcp, schedule, replay, gateway, profile, import, setup, update, doctor, config");
        }
    }

    Ok(())
}

// ─── Agent commands ─────────────────────────────────────────────────────────────

fn handle_agent(
    action: sparrow::cli::AgentAction,
    store: &Arc<dyn AgentStore>,
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
                    println!("  {}  |  {}  |  {}", a.name, a.role, a.personality);
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
            println!("Run task as agent '{}': {}", name, task);
            if let Some(soul) = store.get(&name) {
                println!("Agent identity: {} ({})", soul.name, soul.role);
                println!("(Agent-aware run via 'sparrow run')");
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

fn migrate_inline_provider_keys(
    config: &mut sparrow::config::Config,
    store: &FsConfigStore,
) {
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

        if auth.set(name, Credential::api_key(inline_key)).is_err() {
            continue;
        }

        provider.api_key_env = sparrow::config::providers::find_provider(name)
            .and_then(|def| def.api_key_env);
        changed = true;
    }

    if changed {
        let _ = store.save(config);
    }
}

fn build_provider_brains(
    config: &sparrow::config::Config,
    warn: bool,
) -> std::collections::HashMap<String, Vec<Arc<dyn sparrow::provider::Brain>>> {
    let auth = sparrow::auth::store::ChainedAuthStore::new(config.config_dir.clone());
    let mut providers: std::collections::HashMap<String, Vec<Arc<dyn sparrow::provider::Brain>>> =
        std::collections::HashMap::new();

    for (name, pconfig) in &config.providers {
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
            .or_else(|| auth.get(name).and_then(|c| c.expose_key().map(String::from)))
            .unwrap_or_default();

        if api_key.is_empty() && pconfig.adapter != "ollama" {
            if warn {
                eprintln!("Warning: no credentials for provider '{}', skipping", name);
            }
            continue;
        }

        let mut brains: Vec<Arc<dyn sparrow::provider::Brain>> = Vec::new();
        match pconfig.adapter.as_str() {
            "anthropic-messages" => {
                for model in &pconfig.models {
                    brains.push(Arc::new(
                        sparrow::provider::anthropic::AnthropicAdapter::new(
                            model,
                            api_key.clone(),
                            pconfig.base_url.as_deref(),
                        ),
                    ));
                }
            }
            "openai-compatible" | "ollama" | "openai-chat" => {
                let base_url = pconfig
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "https://api.openai.com/v1".into());
                for model in &pconfig.models {
                    let adapter: Arc<dyn sparrow::provider::Brain> = if pconfig.adapter == "ollama" { Arc::new(sparrow::provider::ollama::OllamaAdapter::new(model, &base_url)) } else { Arc::new(sparrow::provider::openai_compat::OpenAICompatAdapter::new(model, api_key.clone(), &base_url)) };
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
        let adapter =
            sparrow::provider::ollama::OllamaAdapter::new("qwen3.5:32b", &ollama_url);
        providers.insert(
            "ollama".into(),
            vec![Arc::new(adapter) as Arc<dyn sparrow::provider::Brain>],
        );
    }

    providers
}

async fn run_task(
    task: &str,
    _cli: &Cli,
    config: &sparrow::config::Config,
    memory: Arc<dyn Memory>,
    _agent_store: Arc<dyn AgentStore>,
    skills: Arc<dyn SkillLibrary>,
) -> anyhow::Result<()> {
    use sparrow::engine::Engine;
    use sparrow::router::BasicRouter;
    use std::sync::Arc;

    let providers = build_provider_brains(config, true);

    let router = Arc::new(BasicRouter::new(config, providers));
    let engine = Engine::new(router, config.clone())
        .with_memory(memory.clone())
        .with_skills(skills);

    let task_obj = sparrow::engine::Task {
        description: task.to_string(),
        context: vec![],
    };

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    let print_handle = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            match &event {
                sparrow::event::Event::ThinkingDelta { text, .. } => {
                    print!("{}", text);
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
                sparrow::event::Event::CostUpdate { usd, .. } => {
                    println!("\n[Cost: ${:.4}]", usd);
                }
                sparrow::event::Event::RunFinished { outcome, .. } => {
                    println!(
                        "\nDone. Cost: ${:.4}, Tokens: {} in / {} out",
                        outcome.cost_usd, outcome.tokens.input, outcome.tokens.output
                    );
                }
                sparrow::event::Event::Error { message, .. } => {
                    eprintln!("\nError: {}", message);
                }
                _ => {}
            }
        }
    });

    println!("Running: {}", task);
    let outcome = engine.drive(task_obj, tx).await?;
    print_handle.await?;
    println!("Status: {}", outcome.status);
    Ok(())
}

// ─── Swarm command ──────────────────────────────────────────────────────────────

async fn run_swarm(
    task: &str,
    config: &sparrow::config::Config,
    memory: Arc<dyn Memory>,
) -> anyhow::Result<()> {
    use sparrow::orchestrator::{DefaultOrchestrator, Orchestrator, SwarmPlan};
    use sparrow::provider::Brain;
    use sparrow::router::BasicRouter;
    use std::collections::HashMap;
    use std::sync::Arc;

    let auth = sparrow::auth::store::ChainedAuthStore::new(config.config_dir.clone());
    let mut providers: HashMap<String, Vec<Arc<dyn sparrow::provider::Brain>>> = HashMap::new();

    for (name, pconfig) in &config.providers {
        let cred = auth.get(name);
        let api_key = cred
            .and_then(|c| c.expose_key().map(String::from))
            .unwrap_or_default();
        if api_key.is_empty() && pconfig.adapter != "ollama" {
            continue;
        }
        let mut brains: Vec<Arc<dyn sparrow::provider::Brain>> = Vec::new();
        match pconfig.adapter.as_str() {
            "anthropic-messages" => {
                for model in &pconfig.models {
                    brains.push(Arc::new(
                        sparrow::provider::anthropic::AnthropicAdapter::new(
                            model,
                            api_key.clone(),
                            pconfig.base_url.as_deref(),
                        ),
                    ));
                }
            }
            "openai-compatible" | "ollama" | "openai-chat" => {
                let base_url = pconfig
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "https://api.openai.com/v1".into());
                for model in &pconfig.models {
                    let adapter: Arc<dyn sparrow::provider::Brain> = if pconfig.adapter == "ollama" { Arc::new(sparrow::provider::ollama::OllamaAdapter::new(model, &base_url)) } else { Arc::new(sparrow::provider::openai_compat::OpenAICompatAdapter::new(model, api_key.clone(), &base_url)) };
                    brains.push(adapter);
                }
            }
            _ => {}
        }
        if !brains.is_empty() {
            providers.insert(name.clone(), brains);
        }
    }

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
                sparrow::event::Event::AgentStatus { role, status, note, .. } => {
                    let icon = match status {
                        sparrow::event::AgentStatus::Done => "✓",
                        sparrow::event::AgentStatus::Working => "●",
                        sparrow::event::AgentStatus::Thinking => "○",
                        sparrow::event::AgentStatus::Error => "✗",
                        _ => "◌",
                    };
                    println!("│ {} {} — {}", icon, role, note);
                }
                sparrow::event::Event::TestResult { passed: _, failed, detail, .. } => {
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
                sparrow::event::Event::Error { message, .. } => {
                    eprintln!("Error: {}", message);
                }
                _ => {}
            }
        }
    });

    println!("═══ Swarm: {task} ═══\n");

    let outcome = orchestrator.run_swarm(plan, tx).await?;
    print_handle.await?;

    println!("\nPlan  : {} chars", outcome.plan.as_ref().map(|p| p.len()).unwrap_or(0));
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
            };
            library.add(skill)?;
            println!("Skill '{}' created. Edit: ~/.config/sparrow/skills/{}/SKILL.md", name, name);
        }
        sparrow::cli::SkillsAction::Prune => {
            let removed = library.prune(0.2)?;
            println!("Curator pruned {} low-score auto-generated skill(s).", removed);
            let skills = library.all();
            println!("Library now has {} skills.", skills.len());
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
        sparrow::cli::McpAction::Add { server } => {
            println!("Adding MCP server: {}", server);
            println!("For now, edit ~/.config/sparrow/mcp/mcp_servers.json manually");
            println!("Example:");
            println!(r#"  {{"name":"{}","transport":"stdio","command":"npx","args":["-y","@modelcontextprotocol/server-filesystem","/path"],"allow_tools":[]}}"#, server);
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

    let mut job = Job::new(task.to_string(), cron.to_string());
    job.autonomy = level.clone();
    job.next_run = job.next_schedule().map(|dt| dt.to_rfc3339());

    let id = scheduler.schedule(job)?;
    let jobs = scheduler.list();

    println!("Job scheduled: {}", id);
    println!("Task    : {}", task);
    println!("Cron    : {}", cron);
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
                    sparrow::event::Event::ThinkingDelta { text, .. } => { print!("{}", text); }
                    sparrow::event::Event::ToolUseProposed { name, .. } => { println!("\n[Tool: {}]", name); }
                    sparrow::event::Event::RunFinished { outcome, .. } => {
                        println!("\n--- Done: {} | Cost: ${:.4} ---", outcome.status, outcome.cost_usd);
                    }
                    sparrow::event::Event::Error { message, .. } => { eprintln!("\n[Error: {}]", message); }
                    _ => {}
                }
            }

            println!("\n═══ Re-execute? (y/N) ═══");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if input.trim().to_lowercase() == "y" {
                use sparrow::engine::Engine;
                use sparrow::provider::Brain;
                use sparrow::router::BasicRouter;
                let auth = sparrow::auth::store::ChainedAuthStore::new(config.config_dir.clone());
                let mut providers: std::collections::HashMap<String, Vec<Arc<dyn sparrow::provider::Brain>>> = std::collections::HashMap::new();
                for (name, pconfig) in &config.providers {
                    let cred = auth.get(name);
                    let api_key = cred.and_then(|c| c.expose_key().map(String::from)).unwrap_or_default();
                    if api_key.is_empty() && pconfig.adapter != "ollama" { continue; }
                    if pconfig.adapter == "anthropic-messages" {
                        for model in &pconfig.models {
                            providers.entry(name.clone()).or_default().push(Arc::new(
                                sparrow::provider::anthropic::AnthropicAdapter::new(model, api_key.clone(), pconfig.base_url.as_deref())
                            ));
                        }
                    }
                }
                let router = Arc::new(BasicRouter::new(config, providers));
                let engine = Arc::new(Engine::new(router, config.clone()).with_memory(memory));
                let re_executer = ReExecuter::new(engine);
                println!("Re-executing against current model...");
                match re_executer.re_execute(&transcript).await {
                    Ok(outcome) => println!("Re-execute done: {} | ${:.4}", outcome.status, outcome.cost_usd),
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
                        println!("  {} | {} events | {}", t, tr.events.len(), tr.inputs.task.chars().take(60).collect::<String>());
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
    use sparrow::provider::Brain;
    use sparrow::router::BasicRouter;

    let auth = sparrow::auth::store::ChainedAuthStore::new(config.config_dir.clone());
    let mut providers: std::collections::HashMap<String, Vec<Arc<dyn sparrow::provider::Brain>>> = std::collections::HashMap::new();
    for (name, pconfig) in &config.providers {
        let cred = auth.get(name);
        let api_key = cred.and_then(|c| c.expose_key().map(String::from)).unwrap_or_default();
        if api_key.is_empty() && pconfig.adapter != "ollama" { continue; }
        let mut brains: Vec<Arc<dyn sparrow::provider::Brain>> = Vec::new();
        match pconfig.adapter.as_str() {
            "anthropic-messages" => {
                for model in &pconfig.models {
                    brains.push(Arc::new(sparrow::provider::anthropic::AnthropicAdapter::new(model, api_key.clone(), pconfig.base_url.as_deref())));
                }
            }
            "openai-compatible" | "ollama" | "openai-chat" => {
                let base_url = pconfig.base_url.clone().unwrap_or_else(|| "https://api.openai.com/v1".into());
                for model in &pconfig.models {
                    if pconfig.adapter == "ollama" {
                        brains.push(Arc::new(sparrow::provider::ollama::OllamaAdapter::new(model, &base_url)));
                    } else {
                        brains.push(Arc::new(sparrow::provider::openai_compat::OpenAICompatAdapter::new(model, api_key.clone(), &base_url)));
                    }
                }
            }
            _ => {}
        }
        if !brains.is_empty() { providers.insert(name.clone(), brains); }
    }

    let router = Arc::new(BasicRouter::new(config, providers));
    let engine = Arc::new(Engine::new(router, config.clone()).with_memory(memory));
    let mut session = ChatSession::new(engine);
    session.run_interactive().await
}

// ─── Facts commands ─────────────────────────────────────────────────────────────

fn handle_facts(memory: &Arc<dyn Memory>, action: &str, args: &[String]) -> anyhow::Result<()> {
    match action {
        "list" => {
            let facts = memory.all_facts();
            if facts.is_empty() {
                println!("No facts stored.");
                println!("Facts are automatically distilled from successful runs.");
            } else {
                println!("Stored facts ({}):", facts.len());
                for f in &facts {
                    println!("  {}: {}", f.key, f.value);
                }
            }
        }
        "add" => {
            if args.len() < 2 {
                anyhow::bail!("Usage: facts add <key> <value>");
            }
            let fact = Fact {
                id: uuid::Uuid::new_v4().to_string(),
                key: args[0].clone(),
                value: args[1..].join(" "),
                created_at: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                updated_at: chrono::Utc::now().format("%Y-%m-%d").to_string(),
            };
            memory.remember(fact)?;
            println!("Fact added.");
        }
        "rm" | "forget" => {
            if args.is_empty() {
                anyhow::bail!("Usage: facts rm <id>");
            }
            memory.forget(&args[0])?;
            println!("Fact removed.");
        }
        _ => {
            println!("Usage: facts list|add <key> <value>|rm <id>");
        }
    }
    Ok(())
}

// ─── Gateway command ────────────────────────────────────────────────────────────

async fn handle_gateway(
    action: sparrow::cli::GatewayAction,
    state_dir: &std::path::PathBuf,
    config: &sparrow::config::Config,
    memory: Arc<dyn Memory>,
    _scheduler: Arc<MemoryScheduler>,
    recorder: Arc<FsRecorder>,
) -> anyhow::Result<()> {
    match action {
        sparrow::cli::GatewayAction::Start => {
            println!("Starting gateway daemon...");

            use sparrow::engine::Engine;
            use sparrow::provider::Brain;
            use sparrow::router::BasicRouter;
            use std::collections::HashMap;

            // Build providers
            let auth = sparrow::auth::store::ChainedAuthStore::new(config.config_dir.clone());
            let mut providers: HashMap<String, Vec<Arc<dyn sparrow::provider::Brain>>> = HashMap::new();

            for (name, pconfig) in &config.providers {
                let cred = auth.get(name);
                let api_key = cred
                    .and_then(|c| c.expose_key().map(String::from))
                    .unwrap_or_default();
                if api_key.is_empty() && pconfig.adapter != "ollama" {
                    continue;
                }
                let mut brains: Vec<Arc<dyn sparrow::provider::Brain>> = Vec::new();
                match pconfig.adapter.as_str() {
                    "anthropic-messages" => {
                        for model in &pconfig.models {
                            brains.push(Arc::new(
                                sparrow::provider::anthropic::AnthropicAdapter::new(
                                    model,
                                    api_key.clone(),
                                    pconfig.base_url.as_deref(),
                                ),
                            ));
                        }
                    }
                    "openai-compatible" | "ollama" | "openai-chat" => {
                        let base_url = pconfig.base_url.clone().unwrap_or_else(|| {
                            "https://api.openai.com/v1".into()
                        });
                        for model in &pconfig.models {
                            let adapter: Arc<dyn sparrow::provider::Brain> = if pconfig.adapter == "ollama" { Arc::new(sparrow::provider::ollama::OllamaAdapter::new(model, &base_url)) } else { Arc::new(sparrow::provider::openai_compat::OpenAICompatAdapter::new(model, api_key.clone(), &base_url)) };
                            brains.push(adapter);
                        }
                    }
                    _ => {}
                }
                if !brains.is_empty() {
                    providers.insert(name.clone(), brains);
                }
            }

            let router = Arc::new(BasicRouter::new(config, providers));
            let engine = Arc::new(Engine::new(router, config.clone()).with_memory(memory.clone()));

            // Event bus for pub/sub
            let (event_bus_tx, _) = tokio::sync::broadcast::channel::<sparrow::event::Event>(256);

            // Message router
            let router_handler = Arc::new(MessageRouter::new(
                engine,
                recorder.clone(),
                event_bus_tx,
                vec![],
            ));

            // Channel: transports → router
            let (msg_tx, mut msg_rx) = tokio::sync::mpsc::unbounded_channel::<GatewayMessage>();
            // Channel: router → transports
            let (resp_tx, _resp_rx) = tokio::sync::mpsc::unbounded_channel::<GatewayResponse>();

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

            // Always start WebSocket API
            println!("  WS API   : ws://127.0.0.1:9338");
            let ws_api = WebSocketApi::new("127.0.0.1:9338");
            transports.push(Box::new(ws_api));

            // Additional transports (Hermes-parity §15)
            transports.push(Box::new(WhatsAppTransport::new(String::new(), String::new(), vec![])));
            transports.push(Box::new(SignalTransport::new(vec![])));
            transports.push(Box::new(EmailTransport::new(String::new(), 587, String::new(), String::new(), String::new(), vec![])));
            transports.push(Box::new(FeishuTransport::new(String::new(), String::new(), vec![])));
            transports.push(Box::new(WeComTransport::new(String::new(), String::new(), vec![])));
            transports.push(Box::new(QQBotTransport::new(String::new(), String::new(), vec![])));
            transports.push(Box::new(TeamsTransport::new(String::new(), String::new(), vec![])));

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
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(60)) => {
                        // Keep-alive
                    }
                }
            }
        }
        sparrow::cli::GatewayAction::Status => {
            println!("Gateway status: not running");
            println!("Start with: sparrow gateway start");
            Ok(())
        }
        sparrow::cli::GatewayAction::Stop => {
            println!("Gateway stopped.");
            Ok(())
        }
    }
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
            if !profiles_dir.exists() {
                println!("No profiles yet. Create one with: sparrow profile create <name>");
                return Ok(());
            }
            println!("Profiles:");
            if let Ok(entries) = std::fs::read_dir(&profiles_dir) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        if let Some(name) = entry.file_name().to_str() {
                            println!("  - {}", name);
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
            println!("Switched to profile '{}'.", name);
            println!("Set SPARROW_PROFILE={} or use --profile {}", name, name);
        }
    }
    Ok(())
}

// ─── Import command ─────────────────────────────────────────────────────────────

fn handle_import(source: sparrow::cli::ImportSource) -> anyhow::Result<()> {
    match source {
        sparrow::cli::ImportSource::Openclaw { path } => {
            let src = path.unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_default()
                    .join(".openclaw")
            });
            println!("Importing from OpenClaw: {:?}", src);

            if !src.exists() {
                anyhow::bail!("OpenClaw directory not found at {:?}. Use --path to specify.", src);
            }

            // Agents
            let agents_src = src.join("agents");
            if agents_src.exists() {
                let dest = dirs::config_dir()
                    .unwrap_or_default()
                    .join("sparrow")
                    .join("agents");
                std::fs::create_dir_all(&dest)?;
                let mut count = 0;
                if let Ok(entries) = std::fs::read_dir(&agents_src) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().map(|e| e == "toml" || e == "yaml" || e == "yml").unwrap_or(false) {
                            let dest_file = dest.join(entry.file_name());
                            std::fs::copy(&path, &dest_file)?;
                            count += 1;
                        }
                    }
                }
                println!("  Agents: {} imported", count);
            }

            // Skills
            let skills_src = src.join("skills");
            if skills_src.exists() {
                let dest = dirs::config_dir()
                    .unwrap_or_default()
                    .join("sparrow")
                    .join("skills");
                copy_dir_recursive(&skills_src, &dest)?;
                println!("  Skills: imported");
            }

            // Cron jobs (simple JSON file)
            let cron_src = src.join("cron.json");
            if cron_src.exists() {
                let dest = dirs::state_dir()
                    .unwrap_or_default()
                    .join("sparrow")
                    .join("scheduler-jobs.json");
                std::fs::copy(&cron_src, &dest)?;
                println!("  Cron jobs: imported");
            }

            println!("Import complete. Run 'sparrow doctor' to verify.");
        }
    }
    Ok(())
}

fn copy_dir_recursive(src: &std::path::Path, dest: &std::path::Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dest)?;
    if let Ok(entries) = std::fs::read_dir(src) {
        for entry in entries.flatten() {
            let src_path = entry.path();
            let dest_path = dest.join(entry.file_name());
            if src_path.is_dir() {
                copy_dir_recursive(&src_path, &dest_path)?;
            } else {
                std::fs::copy(&src_path, &dest_path)?;
            }
        }
    }
    Ok(())
}

// ─── Setup command ──────────────────────────────────────────────────────────────

async fn handle_setup(config: &sparrow::config::Config) -> anyhow::Result<()> {
    use sparrow::tui::theme::boot_sequence;

    for line in boot_sequence() {
        println!("{}", line);
    }
    println!();
    println!("═══ SPARROW SETUP ═══");
    println!();
    println!("Sparrow uses a conversational setup to configure your environment.");
    println!();
    println!("Quick start:");
    println!("  1. Set API keys as environment variables:");
    println!("     ANTHROPIC_API_KEY, OPENAI_API_KEY, etc.");
    println!("  2. Or edit config directly: sparrow config --edit");
    println!("  3. Or run 'sparrow setup' again for guided setup.");
    println!();
    println!("Current configuration:");
    println!("  Config dir : {:?}", config.config_dir);
    println!("  State dir  : {:?}", config.state_dir);
    println!("  Autonomy   : {:?}", config.defaults.autonomy);
    println!("  Budget     : ${}/day, ${}/session", config.budget.daily_usd, config.budget.session_usd);
    println!();

    let providers = &config.providers;
    if providers.is_empty() {
        println!("No providers configured yet.");
        println!();
        println!("Example config.toml:");
        println!(r#"[providers.anthropic]"#);
        println!(r#"adapter = "anthropic-messages""#);
        println!(r#"models = ["claude-sonnet-4-6"]"#);
        println!();
        println!(r#"[providers.ollama]"#);
        println!(r#"adapter = "ollama""#);
        println!(r#"base_url = "http://localhost:11434/v1""#);
        println!(r#"models = ["qwen3.5:32b"]"#);
    } else {
        println!("Configured providers:");
        for (name, pconfig) in providers {
            println!("  {} (adapter: {})", name, pconfig.adapter);
            for model in &pconfig.models {
                println!("    - {}", model);
            }
        }
    }

    println!();
    println!("Run 'sparrow run \"hello\"' to test your setup.");

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
    if head.is_empty() {
        None
    } else {
        Some(head)
    }
}

fn redacted_config_snapshot(config: &sparrow::config::Config) -> serde_json::Value {
    fn has_secret_prefix(value: &str) -> bool {
        let trimmed = value.trim();
        trimmed.starts_with("sk-")
            || trimmed.starts_with("nvapi-")
            || trimmed.starts_with("gsk_")
            || trimmed.starts_with("sk-or-")
    }

    fn looks_secret_field_value(value: &str) -> bool {
        let trimmed = value.trim();
        has_secret_prefix(trimmed)
            || trimmed.len() > 40
                && !trimmed
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
    }

    fn redact(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, val) in map.iter_mut() {
                    let key_lc = key.to_lowercase();
                    if key_lc.contains("key")
                        || key_lc.contains("token")
                        || key_lc.contains("secret")
                        || key_lc.contains("password")
                    {
                        if val.as_str().map(looks_secret_field_value).unwrap_or(true) {
                            *val = serde_json::Value::String("<redacted>".into());
                            continue;
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
            serde_json::Value::String(s) if has_secret_prefix(s) => {
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

    let providers = build_provider_brains(config, false);

    let router = Arc::new(BasicRouter::new(config, providers));
    let engine = Engine::new(router, config.clone()).with_memory(memory).with_skills(skills);

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

    let task_obj = sparrow::engine::Task { description: task.to_string(), context: vec![] };
    let outcome = engine.drive(task_obj, tx).await?;
    print_handle.await?;

    if outcome.status.contains("error") {
        std::process::exit(1);
    }

    Ok(())
}

// ─── WebView console ───────────────────────────────────────────────────────────

async fn handle_webview(
    config: &sparrow::config::Config,
    memory: Arc<dyn Memory>,
    _scheduler: Arc<MemoryScheduler>,
    recorder: Arc<FsRecorder>,
    skills: Arc<dyn SkillLibrary>,
) -> anyhow::Result<()> {
    use sparrow::engine::Engine;
    use sparrow::router::BasicRouter;
    use std::net::SocketAddr;
    use std::sync::RwLock;

    let (event_tx, _) = tokio::sync::broadcast::channel::<sparrow::event::Event>(256);
    let (command_tx, mut command_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let shared_config = Arc::new(RwLock::new(config.clone()));
    let approvals = Arc::new(sparrow::console::WebApprovalBroker::new());

    let config_for_runs = shared_config.clone();
    let memory_for_runs = memory.clone();
    let skills_for_runs = skills.clone();
    let events_for_runs = event_tx.clone();
    let approvals_for_runs = approvals.clone();
    let recorder_for_runs = recorder.clone();
    tokio::spawn(async move {
        while let Some(task) = command_rx.recv().await {
            let current_config = config_for_runs
                .read()
                .expect("config lock poisoned")
                .clone();
            let task_for_recording = task.clone();
            let config_snapshot = redacted_config_snapshot(&current_config);
            let repo_head = current_repo_head();
            let providers = build_provider_brains(&current_config, false);
            let router = Arc::new(BasicRouter::new(&current_config, providers));
            let engine = Engine::new(router, current_config)
                .with_memory(memory_for_runs.clone())
                .with_skills(skills_for_runs.clone())
                .with_approval_handler(approvals_for_runs.clone());
            let task_obj = sparrow::engine::Task {
                description: task,
                context: vec![],
            };
            let (run_tx, mut run_rx) = tokio::sync::mpsc::unbounded_channel();
            let forward_tx = events_for_runs.clone();
            let recorder = recorder_for_runs.clone();
            let forward = tokio::spawn(async move {
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
                    recorder.record(&event);
                    if let sparrow::event::Event::RunFinished { run, .. } = &event {
                        let _ = recorder.finalize(&run.0);
                    }
                    let _ = forward_tx.send(event);
                }
            });

            if let Err(err) = engine.drive(task_obj, run_tx).await {
                let _ = events_for_runs.send(sparrow::event::Event::Error {
                    run: sparrow::event::RunId("webview".into()),
                    message: format!("run failed: {}", err),
                });
            }
            let _ = forward.await;
        }
    });

    let addr: SocketAddr = "127.0.0.1:9339".parse()?;
    println!("WebView console: http://{}", addr);
    println!("Open this URL in your browser.");
    println!("Press Ctrl+C to stop.\n");

    let server = WebViewServer::new(
        addr,
        event_tx,
        Some(command_tx),
        Some(shared_config),
        Some(approvals),
    );
    server.serve().await?;

    Ok(())
}
