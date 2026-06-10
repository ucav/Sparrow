// src/cmd_handlers/handle_permissions_cmd.rs — extracted from main.rs

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
