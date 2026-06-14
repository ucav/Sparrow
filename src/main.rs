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
use sparrow::runtime::scheduler::MemoryScheduler;
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
    // Box::pinned. `Cli` itself is also a large type (clap builds the full
    // command tree), so even `Cli::parse()` must run on the roomy stack. Run
    // everything on a worker thread with an explicit 16 MB stack.
    let worker = std::thread::Builder::new()
        .name("sparrow-main".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(move || -> anyhow::Result<()> {
            // Parse args BEFORE building the tokio runtime. Clap renders
            // `--version`/`--help` (and usage errors) and exits right here — so
            // those hot paths never pay for a multi-threaded tokio runtime (one
            // worker per core + IO/timer drivers) or tracing init. That runtime
            // spin-up was the dominant wasted cost in `sparrow --version` /
            // `help` startup (see artifacts/perf-report.md, Plan B). Parsing on
            // the 16 MB worker stack avoids the main-thread overflow.
            // Natural-language front door: if clap can't parse the input as a
            // known command (unknown subcommand, or an alias given extra words
            // like `montre la console`), don't error — treat the WHOLE input as
            // natural language and route it. The user never has to learn a
            // command. `--help`/`--version`/usage still behave normally.
            let cli = Cli::try_parse().unwrap_or_else(|e| {
                use clap::error::ErrorKind;
                if matches!(
                    e.kind(),
                    ErrorKind::DisplayHelp
                        | ErrorKind::DisplayVersion
                        | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
                ) {
                    e.exit();
                }
                let raw: Vec<String> = std::env::args().skip(1).collect();
                if raw.is_empty() {
                    e.exit();
                }
                // Re-parse through the `do` front door, which collects the words
                // and routes them. If that also fails, surface the original error.
                let mut argv = vec!["sparrow".to_string(), "do".to_string()];
                argv.extend(raw);
                Cli::try_parse_from(argv).unwrap_or_else(|_| e.exit())
            });
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            runtime.block_on(Box::pin(async_main(cli)))
        })?;
    worker
        .join()
        .map_err(|_| anyhow::anyhow!("sparrow main thread panicked"))?
}

/// Whether a command consults a model catalogue and therefore benefits from the
/// boot-time background model discovery (Ollama probe + per-provider discovery).
///
/// Read-only / local-only commands (listing auth, memory, checkpoints, config,
/// running the doctor, …) never touch a catalogue, so kicking off discovery for
/// them just wastes startup and — worse for a trust-sensitive tool — opens a
/// stray network connection (e.g. an Ollama probe on `localhost:11434`) that the
/// user never asked for. This is a conservative DENYLIST: anything not listed
/// keeps the previous behaviour (discovery on), so no model-consuming command
/// can silently regress to an empty catalogue.
fn command_wants_model_discovery(cmd: &Option<Commands>) -> bool {
    use Commands::*;
    match cmd {
        // Bare `sparrow` opens the cockpit → wants the catalogue.
        None => true,
        Some(
            Auth { .. }
            | Memory { .. }
            | Checkpoint { .. }
            | Rewind { .. }
            | Replay { .. }
            | Sessions { .. }
            | Permissions { .. }
            | Profile { .. }
            | Config { .. }
            | Skills { .. }
            | Plugins { .. }
            | Tools { .. }
            | Security { .. }
            | Hook { .. }
            | Mcp { .. }
            | Import { .. }
            | Whatis { .. }
            | Budget { .. }
            | Mode { .. }
            | Do { .. }
            | Natural(_)
            | Doctor
            | Setup
            | Init
            | Status
            | Update
            | Share,
        ) => false,
        // Everything else (run, chat, plan, model, route, console, launch,
        // gateway, agent, swarm, …) keeps boot discovery.
        Some(_) => true,
    }
}

/// One-time, non-blocking informed-consent notice shown on first launch so the
/// user knows what autonomy level Sparrow will run at before granting it any
/// tools. The default is `Trusted` (auto-runs exec/network with notification);
/// this surfaces that plainly and tells the user how to dial it down. Reads the
/// real configured level so it stays accurate if the user has changed it.
fn autonomy_consent_notice(config: &sparrow::config::Config) -> String {
    use sparrow::event::AutonomyLevel;
    let (level, detail) = match config.defaults.autonomy {
        AutonomyLevel::Supervised => (
            "Supervised",
            "Sparrow asks before every shell command, file change, and network call.",
        ),
        AutonomyLevel::Trusted => (
            "Trusted",
            "Sparrow runs shell commands and network tools automatically — you are \
             notified, not asked. Destructive actions always ask first, and a Git \
             checkpoint is taken before changes (`sparrow rewind` undoes them).",
        ),
        AutonomyLevel::Autonomous => (
            "Autonomous",
            "Sparrow runs shell commands and network tools without prompting.",
        ),
    };
    format!(
        "\n  ⚙  Autonomy: {level}. {detail}\n\
         \x20    Change it: run with `--autonomy supervised`, or set `defaults.autonomy` in config.toml.\n\
         \x20    Safety & sandbox details: SECURITY.md\n"
    )
}

async fn async_main(cli: Cli) -> anyhow::Result<()> {
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
            experience: Default::default(),
            skills: Default::default(),
            intel: Default::default(),
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
    let fast_console = matches!(&cli.command, Some(Commands::Console { fast: true, .. }));

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
    if !fast_console && command_wants_model_discovery(&cli.command) {
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

    // ── First-launch detection (§16 / v0.9) ──────────────────────────
    // Default path is now zero-question: create a free-first config and get to
    // the user's prompt. The older setup agent remains behind `launch --pro`.
    let is_first_launch = !active_config_dir.join("config.toml").exists();
    if is_first_launch && cli.command.is_none() {
        config = sparrow::onboarding::zero_question::prepare_default_launch(&config, &config_store)
            .await?;
        config.config_dir = active_config_dir.clone();
        config.state_dir = active_state_dir.clone();
        println!("{}", sparrow::onboarding::zero_question::ready_message());
    }

    // Informed consent (#10a): on first launch, on the interactive entry points,
    // tell the user plainly what autonomy level they're about to grant. Printed
    // once (config.toml doesn't exist yet on first run) and never blocks.
    if is_first_launch
        && matches!(
            cli.command,
            None | Some(Commands::Launch { .. })
                | Some(Commands::Tui)
                | Some(Commands::Console { .. })
        )
    {
        println!("{}", autonomy_consent_notice(&config));
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
                    false,
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
        Some(Commands::Launch { port, tui, pro }) => {
            if !active_config_dir.join("config.toml").exists() {
                if pro {
                    println!("Mode expert - configuration détaillée...\n");
                    let setup_result = sparrow::onboarding::setup_agent::run_setup_agent(
                        &config,
                        &config_store,
                        memory.clone(),
                        build_provider_brains,
                    )
                    .await;
                    if let Err(err) = setup_result {
                        eprintln!("Setup Agent: {} - falling back to interactive setup.", err);
                        sparrow::cmd_handlers::setup_cmd::handle_setup(&config, &config_store)
                            .await?;
                    }
                    if let Ok(fresh) = config_store.load() {
                        config = fresh;
                        config.config_dir = active_config_dir.clone();
                        config.state_dir = active_state_dir.clone();
                    }
                } else {
                    config = sparrow::onboarding::zero_question::prepare_default_launch(
                        &config,
                        &config_store,
                    )
                    .await?;
                    config.config_dir = active_config_dir.clone();
                    config.state_dir = active_state_dir.clone();
                    println!("{}", sparrow::onboarding::zero_question::ready_message());
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
                    false,
                )
                .await?;
            }
        }
        Some(Commands::Console { port, fast }) => {
            handle_webview(
                &config,
                memory.clone(),
                scheduler.clone(),
                recorder.clone(),
                skill_library.clone(),
                Some(agent_store.clone()),
                port,
                cli.bind.clone(),
                fast,
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
        // v0.9 Pilier 1 — « entrée par le problème ». `fix` and `explique`
        // are human-language front doors that frame the task for a diagnosis-
        // first, jargon-free run, then reuse the exact same engine path as
        // `run` (nothing is lost vs pro mode).
        Some(Commands::Fix { ref problem }) => {
            let joined = problem.join(" ");
            let task = if joined.trim().is_empty() {
                "Inspecte le dossier courant et trouve le problème le plus évident \
                 (erreur de build, test cassé, conflit git, dépendances manquantes). \
                 Explique en UNE phrase simple, sans jargon, ce qui ne va pas et \
                 pourquoi. Propose la correction, et applique-la seulement après mon \
                 accord. Réponds dans la langue de l'utilisateur."
                    .to_string()
            } else {
                format!(
                    "J'ai ce problème : « {joined} ». Diagnostique la cause réelle \
                     en lisant les fichiers/erreurs concernés avant de conclure. \
                     Explique en UNE phrase simple, sans jargon, ce qui ne va pas. \
                     Propose la correction et applique-la seulement après mon accord. \
                     Réponds dans la langue de l'utilisateur."
                )
            };
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
                &task,
                &config,
                memory.clone(),
                skill_library.clone(),
                recorder.clone(),
                None,
                flags,
            )
            .await?;
        }
        Some(Commands::Explique { ref target }) => {
            let joined = target.join(" ");
            if joined.trim().is_empty() {
                eprintln!(
                    "Dis-moi quoi expliquer : un fichier, une erreur, ou un mot.\n\
                     Exemple : sparrow explique src/main.rs"
                );
            } else {
                let task = format!(
                    "Explique « {joined} » en langage simple, comme à quelqu'un qui \
                     débute. Si c'est un chemin de fichier, lis-le d'abord. Si c'est \
                     une erreur, explique ce qu'elle signifie et d'où elle vient. \
                     Sois bref, concret, sans jargon inutile. Ne modifie aucun \
                     fichier. Réponds dans la langue de l'utilisateur."
                );
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
                    &task,
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
        Some(Commands::Run {
            ref task,
            json: _json,
            plan_first,
            dry_run,
            patch,
        }) => {
            {
                if plan_first {
                    sparrow::cmd_handlers::handle_plan_cmd::handle_plan(
                        task,
                        &config,
                        skill_library.clone(),
                        false,
                    )?;
                    if !cli.yes {
                        anyhow::bail!(
                            "`--plan-first` stops before execution. Re-run with `--yes` to execute this plan."
                        );
                    }
                }
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
                let mut run_config;
                let config_ref = if dry_run || patch {
                    run_config = config.clone();
                    run_config.permissions.mode = sparrow::permissions::PermissionMode::ReadOnly;
                    &run_config
                } else {
                    &config
                };
                let task_with_mode;
                let task_ref = if patch {
                    task_with_mode = format!(
                        "PATCH MODE: do not modify files. Produce a unified diff only, with enough context for `git apply`, for this task:\n\n{}",
                        task
                    );
                    task_with_mode.as_str()
                } else if dry_run {
                    task_with_mode = format!(
                        "DRY RUN: inspect and propose changes only. Do not modify files or execute mutating tools. Task:\n\n{}",
                        task
                    );
                    task_with_mode.as_str()
                } else {
                    task.as_str()
                };
                sparrow::cmd_handlers::handle_run_task_cmd::run_task(
                    task_ref,
                    config_ref,
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
        Some(Commands::Audit { json }) => {
            let root = std::env::current_dir()?;
            let audit = sparrow::repo_audit::run_repo_audit(&root)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&audit)?);
            } else {
                let path = sparrow::repo_audit::write_audit_markdown(&root, &audit)?;
                println!("Audit written: {}", path.display());
                println!(
                    "Files: {} · Rust: {} · stubs: {} · TODO/FIXME: {}",
                    audit.files_total,
                    audit.rust_files,
                    audit.production_stubs.len(),
                    audit.todo_comments.len()
                );
            }
        }
        Some(Commands::Test { fix, json }) => {
            let root = std::env::current_dir()?;
            let Some(runner) = sparrow::project_test::detect_test_runner(&root) else {
                anyhow::bail!(
                    "No test runner detected. Expected Cargo.toml, package.json, pyproject.toml, pytest.ini, or setup.cfg."
                );
            };
            let output = std::process::Command::new(&runner.command)
                .args(&runner.args)
                .current_dir(&root)
                .output()?;
            let success = output.status.success();
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "ok": success,
                        "runner": runner,
                        "status": output.status.code(),
                        "stdout": stdout,
                        "stderr": stderr,
                    }))?
                );
            } else {
                println!("Runner: {}", runner.display_command());
                if !stdout.trim().is_empty() {
                    println!("{stdout}");
                }
                if !stderr.trim().is_empty() {
                    eprintln!("{stderr}");
                }
            }
            if !success {
                if fix {
                    let task = format!(
                        "The project test runner `{}` failed. Read the failure output below, fix the root cause with minimal edits, then rerun the same test command until it passes or after 3 bounded attempts. Output:\n\nSTDOUT:\n{}\n\nSTDERR:\n{}",
                        runner.display_command(),
                        stdout,
                        stderr
                    );
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
                        &task,
                        &config,
                        memory.clone(),
                        skill_library.clone(),
                        recorder.clone(),
                        None,
                        flags,
                    )
                    .await?;
                } else {
                    anyhow::bail!("test runner failed: {}", runner.display_command());
                }
            }
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
        Some(Commands::Commit {
            ref message,
            dry_run,
        }) => {
            let stat = std::process::Command::new("git")
                .args(["diff", "--cached", "--stat"])
                .output()?;
            let staged_stat = String::from_utf8_lossy(&stat.stdout).to_string();
            if staged_stat.trim().is_empty() {
                if dry_run {
                    println!("No staged changes to commit.");
                    println!("Dry run: no commit created.");
                    return Ok(());
                }
                anyhow::bail!(
                    "No staged changes to commit. Stage files first, then run `sparrow commit`."
                );
            }
            let diff = std::process::Command::new("git")
                .args(["diff", "--cached"])
                .output()?;
            let staged_diff = String::from_utf8_lossy(&diff.stdout);
            let plan = sparrow::git_workflow::build_commit_plan(
                message.clone(),
                staged_stat,
                &staged_diff,
            );
            if !plan.secret_findings.is_empty() {
                println!("Secret-like patterns found in staged diff:");
                for finding in &plan.secret_findings {
                    println!("- {finding}");
                }
                anyhow::bail!("Commit refused until the staged diff is clean.");
            }
            println!("Commit message: {}", plan.message);
            println!("{}", plan.staged_stat);
            if dry_run {
                println!("Dry run: no commit created.");
            } else {
                let msg_path = std::env::temp_dir().join(format!(
                    "sparrow-commit-{}.txt",
                    chrono::Local::now().format("%Y%m%d%H%M%S")
                ));
                std::fs::write(&msg_path, &plan.message)?;
                let status = std::process::Command::new("git")
                    .arg("commit")
                    .arg("-F")
                    .arg(&msg_path)
                    .status()?;
                let _ = std::fs::remove_file(&msg_path);
                if !status.success() {
                    anyhow::bail!("git commit failed with status {status}");
                }
            }
        }
        Some(Commands::Release { action }) => match action {
            sparrow::cli::ReleaseAction::Prep { dry_run } => {
                let root = std::env::current_dir()?;
                let files = if dry_run {
                    sparrow::release_prep::planned_release_doc_paths(&root)
                } else {
                    sparrow::release_prep::prepare_release_docs(&root)?
                };
                for file in files {
                    println!("{}", file.display());
                }
            }
        },
        Some(Commands::Intel { action }) => {
            sparrow::intel_cli::handle_intel_action(action, &config).await?;
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
        Some(Commands::Reason { task }) => {
            sparrow::cmd_handlers::handle_reason_cmd::handle_reason(&config, memory.clone(), &task)
                .await?;
        }
        Some(Commands::Do { request, dry_run }) => {
            sparrow::cmd_handlers::handle_do_cmd::handle_do(
                &config,
                memory.clone(),
                &request,
                dry_run,
            )
            .await?;
        }
        // The bare-text front door: `sparrow corrige le build` (no command word)
        // arrives here as an unknown subcommand and is routed automatically.
        Some(Commands::Natural(words)) => {
            sparrow::cmd_handlers::handle_do_cmd::dispatch_natural_language(
                &config,
                memory.clone(),
                &words.join(" "),
                false,
            )
            .await?;
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
                    if let Some(pin) = &updated.routing.preferred_provider {
                        println!("🔒 Manual mode active. Current pin: {}", pin);
                    } else {
                        println!("🔒 Manual mode active. Choose a provider/model with:");
                        println!("  sparrow route set <provider>");
                        println!("  sparrow route set <provider>/<model>");
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
        // v0.9 Pilier 1 — l'accueil chaleureux + détection de contexte.
        Some(Commands::Bonjour) => {
            let lang = config.experience.lang();
            let cwd = std::env::current_dir().unwrap_or_default();
            println!("{}", sparrow::welcome::welcome_text(&cwd, lang));
        }
        // v0.9 Pilier 4 — budget en langage humain. Show or set the per-session
        // spend cap; accepts "2€", "$0.50" or a bare number.
        Some(Commands::Budget { amount }) => {
            match amount {
                None => {
                    println!(
                        "Plafond actuel : {:.2} par session, {:.2} par jour.",
                        config.budget.session_usd, config.budget.daily_usd
                    );
                    println!(
                        "Je m'arrête tout seul avant de dépasser. Pour changer : sparrow budget 2€"
                    );
                }
                Some(raw) => {
                    // Strip currency symbols/spaces, accept comma decimals.
                    let cleaned: String = raw
                        .chars()
                        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
                        .collect::<String>()
                        .replace(',', ".");
                    match cleaned.parse::<f64>() {
                        Ok(value) if value > 0.0 => {
                            let mut updated = config.clone();
                            updated.budget.session_usd = value;
                            match config_store.save(&updated) {
                                Ok(()) => println!(
                                    "C'est noté : je m'arrête tout seul à {:.2} par session.",
                                    value
                                ),
                                Err(e) => eprintln!("Je n'ai pas pu enregistrer : {e}"),
                            }
                        }
                        _ => eprintln!(
                            "Je n'ai pas compris « {raw} ». Donne-moi un montant, par exemple : 2€ ou $0.50."
                        ),
                    }
                }
            }
        }
        // v0.9 Pilier 6 — la galerie des possibles. Browse ready-to-run
        // recipes by persona or keyword; each printed prompt is the tutorial.
        Some(Commands::Idees { filter }) => {
            let lang = config.experience.lang();
            let raw = filter.join(" ");
            let trimmed = raw.trim();
            // A single word that matches a persona slug filters by persona;
            // otherwise the whole input is a free-text query.
            let known_personas = sparrow::gallery::personas();
            let (persona, query): (Option<&str>, Option<&str>) = if !trimmed.is_empty()
                && known_personas
                    .iter()
                    .any(|p| p.starts_with(&trimmed.to_lowercase()))
            {
                (Some(trimmed), None)
            } else if trimmed.is_empty() {
                (None, None)
            } else {
                (None, Some(trimmed))
            };
            let results = sparrow::gallery::search(persona, query);
            if results.is_empty() {
                println!("Aucune idée ne correspond à « {trimmed} ».");
                println!("Profils disponibles : {}", known_personas.join(" · "));
            } else {
                if trimmed.is_empty() {
                    println!("🐦  Voici ce que tu peux faire avec Sparrow :\n");
                } else {
                    println!("🐦  Idées pour « {trimmed} » :\n");
                }
                let mut current = "";
                for r in results {
                    let group = r.persona.label(lang);
                    if group != current {
                        println!("  {group}");
                        current = group;
                    }
                    println!("    · {}  ({})", r.title(lang), r.est);
                    println!("      → {}", r.prompt(lang));
                }
                println!(
                    "\nCopie une de ces phrases après « sparrow run », ou tape\n\
                     « sparrow fix » / « sparrow explique » pour ton propre besoin."
                );
            }
        }
        // v0.9 Pilier 2 — le glossaire vivant. Instant, offline definition of
        // Sparrow's own jargon; unknown terms point at `sparrow explique`.
        Some(Commands::Whatis { term }) => {
            let lang = config.experience.lang();
            let joined = term.join(" ");
            if joined.trim().is_empty() {
                println!("Les mots que je peux définir tout de suite :");
                let mut terms = sparrow::glossary::terms();
                terms.sort_unstable();
                println!("  {}", terms.join(" · "));
                println!("\nExemple : sparrow whatis checkpoint");
                println!("Pour autre chose : sparrow explique « ton sujet »");
            } else if let Some(def) = sparrow::glossary::lookup(&joined, lang) {
                println!("{def}");
            } else {
                println!(
                    "Je n'ai pas « {joined} » dans mon glossaire.\n\
                     Essaie : sparrow explique « {joined} » — je te l'explique pour de vrai."
                );
            }
        }
        // v0.9 Pilier 2 — the depth switch. `sparrow mode simple|builder|pro|auto`
        // sets how Sparrow talks to you; no argument prints the current mode.
        Some(Commands::Mode { mode }) => match mode {
            None => {
                let m = config.experience.mode.to_lowercase();
                let human = match m.as_str() {
                    "pro" => "détaillé (pro) — sortie technique complète",
                    "builder" => "builder — run, test, refactor, git, debug, replay",
                    "simple" => "simple — langage clair, sans jargon",
                    _ => "auto — simple par défaut, bascule possible",
                };
                println!("Mode actuel : {human}.");
                println!(
                    "Pour changer : sparrow mode simple · sparrow mode builder · sparrow mode pro"
                );
            }
            Some(requested) => {
                let normalized = requested.trim().to_lowercase();
                if !matches!(normalized.as_str(), "simple" | "builder" | "pro" | "auto") {
                    eprintln!(
                        "Mode inconnu : « {requested} ». Choisis : simple, builder, pro ou auto."
                    );
                } else {
                    let mut updated = config.clone();
                    updated.experience.mode = normalized.clone();
                    match config_store.save(&updated) {
                        Ok(()) => {
                            let confirm = match normalized.as_str() {
                                "pro" => {
                                    "Mode détaillé activé — tu verras tout (route, tokens, coût)."
                                }
                                "simple" => {
                                    "Mode simple activé — je te parle en clair, sans jargon."
                                }
                                "builder" => {
                                    "Mode builder activé — menus Run/Test/Refactor/Git/Debug."
                                }
                                _ => "Mode auto activé — simple par défaut.",
                            };
                            println!("{confirm}");
                        }
                        Err(e) => eprintln!("Je n'ai pas pu enregistrer ce réglage : {e}"),
                    }
                }
            }
        },
        // v0.9 Pilier 4 — « le filet de sécurité ». `annule` is the one-word
        // undo: with no id it resolves the most recent checkpoint (or the
        // oldest with --tout), confirms, and reports what was restored in
        // plain language. It reuses the same git-backed rewind as `rewind`.
        Some(Commands::Annule { id, tout }) => {
            let cwd = std::env::current_dir().unwrap_or_default();
            // Resolve the target checkpoint. Explicit id wins; otherwise pick
            // the newest (default) or oldest (--tout) by creation date.
            let resolve = |sort: &str| -> Option<(String, String)> {
                let out = std::process::Command::new("git")
                    .args([
                        "for-each-ref",
                        "--sort",
                        sort,
                        "--count=1",
                        "refs/sparrow/checkpoints",
                        "--format=%(refname:short)|%(creatordate:format:%H:%M)",
                    ])
                    .current_dir(&cwd)
                    .output()
                    .ok()?;
                let text = String::from_utf8_lossy(&out.stdout);
                let line = text.lines().next()?.trim();
                if line.is_empty() {
                    return None;
                }
                let (refname, when) = line.split_once('|').unwrap_or((line, ""));
                let short = refname.rsplit('/').next().unwrap_or(refname).to_string();
                Some((short, when.to_string()))
            };

            let target = match id {
                Some(explicit) => Some((explicit, String::new())),
                None if tout => resolve("creatordate"),
                None => resolve("-creatordate"),
            };

            let Some((target_id, when)) = target else {
                println!(
                    "Il n'y a encore aucun point de sauvegarde à annuler.\n\
                     Sparrow en crée un automatiquement avant chaque modification de fichier."
                );
                return Ok(());
            };

            if !cli.yes {
                let what = if tout {
                    "tout remettre comme au début de la session".to_string()
                } else {
                    "annuler la dernière modification".to_string()
                };
                eprint!(
                    "Je vais {} (point de sauvegarde {}{}). \
                     Les fichiers actuels non sauvegardés seront perdus. On y va ? [o/N] ",
                    what,
                    target_id,
                    if when.is_empty() {
                        String::new()
                    } else {
                        format!(" · {}", when)
                    }
                );
                let _ = std::io::stdout().flush();
                let mut input = String::new();
                let ok = std::io::stdin().read_line(&mut input).is_ok()
                    && matches!(
                        input.trim().to_lowercase().as_str(),
                        "o" | "oui" | "y" | "yes"
                    );
                if !ok {
                    println!("Annulation abandonnée — rien n'a changé.");
                    return Ok(());
                }
            }

            let checkpoints = GitCheckpoints::new(cwd);
            match checkpoints.rewind(sparrow::event::CheckpointId(target_id.clone())) {
                Ok(()) => {
                    if when.is_empty() {
                        println!(
                            "C'est fait — tes fichiers sont revenus au point {}.",
                            target_id
                        );
                    } else {
                        println!(
                            "C'est fait — tes fichiers sont revenus comme ils étaient à {}.",
                            when
                        );
                    }
                }
                Err(e) => eprintln!(
                    "Je n'ai pas réussi à revenir en arrière : {}.\n\
                     Tape `sparrow doctor` si le problème persiste.",
                    e
                ),
            }
        }
        Some(Commands::Doctor) => {
            println!("Diagnostic Sparrow");
            println!("==================");
            println!(
                "✅ Configuration : prête. Budget ${:.2}/session, ${:.2}/jour.",
                config.budget.session_usd, config.budget.daily_usd
            );

            let auth = sparrow::auth::store::ChainedAuthStore::new(config.config_dir.clone());
            let stored = auth.list();
            let providers = sparrow::config::effective_provider_configs(&config);
            let has_local = providers.contains_key("ollama") || providers.contains_key("local");
            let provider_icon = if providers.is_empty() { "❌" } else { "✅" };
            let provider_phrase = if providers.is_empty() {
                "aucun moteur détecté. Lance `sparrow launch` pour préparer le secours local."
                    .to_string()
            } else if has_local {
                format!(
                    "{} moteur(s) détecté(s), avec secours local gratuit.",
                    providers.len()
                )
            } else {
                format!(
                    "{} moteur(s) détecté(s). Ajoute Ollama pour un secours gratuit.",
                    providers.len()
                )
            };
            println!("{provider_icon} Moteurs : {provider_phrase}");
            println!(
                "✅ Clés : {} entrée(s) dans le coffre Sparrow.",
                stored.len()
            );

            let git_ok = std::process::Command::new("git")
                .arg("--version")
                .output()
                .is_ok();
            println!(
                "{} Git : {}",
                if git_ok { "✅" } else { "⚠️" },
                if git_ok {
                    "disponible pour les points de sauvegarde."
                } else {
                    "absent. Installe Git pour mieux annuler les changements."
                }
            );

            #[cfg(not(target_os = "linux"))]
            let sandbox_phrase = if config.defaults.sandbox == "local-hardened" {
                "actif avec les protections disponibles sur cette plateforme."
            } else {
                "réglé dans la configuration."
            };
            #[cfg(target_os = "linux")]
            let sandbox_phrase = "actif avec isolation Linux quand disponible.";
            println!("✅ Sécurité : {sandbox_phrase}");

            println!(
                "✅ Mémoire : {} souvenir(s), {} agent(s), {} compétence(s).",
                memory.all_facts().len(),
                agent_store.list().len(),
                skill_library.all().len()
            );

            if let Ok(Some(update)) =
                tokio::task::spawn_blocking(sparrow::update::check_update).await
            {
                println!("⚠️ Mise à jour : {update}. Lance `sparrow update` quand tu veux.");
            } else {
                println!("✅ Mise à jour : rien d'urgent détecté.");
            }

            println!();
            println!("Réparer automatiquement : `sparrow fix \"ce qui bloque\"`.");
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
    fast_start: bool,
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
    let agent_store_for_runs = agent_store.clone();
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
            // v0.9.1: the console run path previously omitted agent_store and
            // hooks_config, so from the WebView the agent could NOT spawn
            // sub-agents (the soul tells it to) and NO lifecycle hooks fired.
            // Wire them so the console behaves exactly like the CLI in production.
            let hooks_for_engine = current_config.hooks.clone();
            let mut engine = Engine::new(router, current_config)
                .with_memory(memory_for_runs.clone())
                .with_skills(skills_for_runs.clone())
                .with_approval_handler(approvals_for_runs.clone())
                .with_hooks_config(hooks_for_engine);
            // agent_store is optional at the console layer; wire it when present
            // so sub-agent spawning works from the WebView like it does in CLI.
            if let Some(store) = &agent_store_for_runs {
                engine = engine.with_agent_store(store.clone());
            }
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
    let url = if fast_start {
        format!("http://127.0.0.1:{}?boot=0&fast=1", port)
    } else {
        format!("http://127.0.0.1:{}", port)
    };
    println!("WebView console: {}", url);
    if bind_target.is_public {
        println!(
            "⚠️  Sparrow écoute sur {} — accessible depuis le réseau local.\n\
             N'utilise ça que sur un réseau de confiance (clés, fichiers et agents y sont exposés).",
            addr
        );
    }
    println!("Press Ctrl+C to stop.\n");

    // Auto-open the browser for the normal local WebView cockpit. `--fast`
    // leaves this to the caller so the server can bind immediately and scripts
    // can measure /healthz without paying the OS browser-launch cost.
    if !fast_start {
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

#[cfg(test)]
mod consent_tests {
    use super::autonomy_consent_notice;
    use sparrow::event::AutonomyLevel;

    fn config_with(level: AutonomyLevel) -> sparrow::config::Config {
        let mut c = sparrow::config::Config::default();
        c.defaults.autonomy = level;
        c
    }

    #[test]
    fn trusted_notice_is_honest_about_auto_exec() {
        let notice = autonomy_consent_notice(&config_with(AutonomyLevel::Trusted));
        assert!(notice.contains("Trusted"));
        // Must state that exec/network run automatically, and how to dial down.
        assert!(notice.contains("automatically"));
        assert!(notice.contains("--autonomy supervised"));
        assert!(notice.contains("SECURITY.md"));
    }

    #[test]
    fn supervised_notice_says_it_asks() {
        let notice = autonomy_consent_notice(&config_with(AutonomyLevel::Supervised));
        assert!(notice.contains("Supervised"));
        assert!(notice.contains("asks before"));
    }

    #[test]
    fn default_autonomy_is_trusted_so_consent_is_warranted() {
        // If the default ever changes, this notice's framing must be revisited.
        let notice = autonomy_consent_notice(&sparrow::config::Config::default());
        assert!(notice.contains("Trusted"));
    }
}
