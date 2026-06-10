// src/cmd_handlers/handle_run_task_cmd.rs
use super::prelude::*;

/// Snapshot the relevant parts of the config for the recorder, with any
/// inline secrets in provider entries replaced by "<redacted>". Lives here
/// (rather than in main.rs) so any handler can reach it via the prelude.
pub fn redacted_config_snapshot(config: &sparrow::config::Config) -> serde_json::Value {
    let looks_like_inline_secret = super::handle_agent_cmd::looks_like_inline_secret;
    serde_json::json!({
        "theme": config.theme,
        "autonomy": config.defaults.autonomy,
        "sandbox": config.defaults.sandbox,
        "budget": {
            "daily": config.budget.daily_usd,
            "session": config.budget.session_usd
        },
        "routing": {
            "free_first": config.routing.free_first,
            "policy": config.routing.policy,
            "preferred_provider": config.routing.preferred_provider
        },
        "providers": config.providers.iter().map(|(k, v)| {
            let api_key = match &v.api_key_env {
                Some(env) if looks_like_inline_secret(env) => Some("<redacted>".to_string()),
                Some(env) => Some(env.clone()),
                None => None,
            };
            (k.clone(), serde_json::json!({
                "adapter": v.adapter,
                "models": v.models,
                "api_key_env": api_key,
                "has_key": v.api_key_env.as_ref()
                    .map(|env| std::env::var(env).map(|v| !v.trim().is_empty()).unwrap_or(false))
                    .unwrap_or(false)
            }))
        }).collect::<serde_json::Map<_, _>>()
    })
}

pub async fn run_task(
    task: &str,
    config: &sparrow::config::Config,
    memory: Arc<dyn Memory>,
    skills: Arc<dyn SkillLibrary>,
    recorder: Arc<FsRecorder>,
    soul: Option<Soul>,
    flags: RunFlags,
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
    let session_key = match flags.session_mode {
        SessionMode::ContinueLast => {
            // `sparrow --continue`: pick up the most recently updated session
            // from ANY surface (CLI, console, gateway threads).
            sessions
                .as_ref()
                .and_then(|s| s.list().into_iter().next())
                .map(|s| s.id)
                .unwrap_or_else(|| {
                    std::env::var("SPARROW_SESSION").unwrap_or_else(|_| {
                        format!(
                            "cli:{}",
                            std::env::current_dir()
                                .map(|p| p.display().to_string())
                                .unwrap_or_else(|_| "default".into())
                        )
                    })
                })
        }
        _ => std::env::var("SPARROW_SESSION").unwrap_or_else(|_| {
            format!(
                "cli:{}",
                std::env::current_dir()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| "default".into())
            )
        }),
    };
    let prior_msgs: Vec<sparrow::provider::Msg> = if flags.session_mode == SessionMode::Fresh {
        Vec::new()
    } else {
        sessions
            .as_ref()
            .and_then(|s| s.load(&session_key))
            .and_then(|sess| match serde_json::from_str(&sess.messages_json) {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("session '{}' deserialize failed: {}", session_key, e);
                    None
                }
            })
            .unwrap_or_default()
    };
    // Make continuity VISIBLE — silently carrying context surprises users.
    if !prior_msgs.is_empty() {
        eprintln!(
            "\x1b[2m↩ continuing session ({} prior messages) — use --fresh to start clean\x1b[0m",
            prior_msgs.len()
        );
    }

    // ── Pre-run quote (estimate, then confirm) ─────────────────────────────
    // Nobody else quotes BEFORE executing. Skipped with --yes or when stdin
    // is not a TTY (CI, pipes) so automation never blocks.
    let interactive = std::io::IsTerminal::is_terminal(&std::io::stdin());
    if !flags.assume_yes && interactive {
        let pf = engine.preflight(task);
        let chain_disp = sparrow::engine::summarize_model_chain(&pf.chain, 3);
        eprintln!(
            "  plan: tier {} · est. {}–{}k tok · est. ${:.2}–${:.2} · route: {}",
            pf.tier.as_str(),
            (pf.est_input_range.0 + pf.est_output_range.0) / 1_000,
            (pf.est_input_range.1 + pf.est_output_range.1) / 1_000,
            pf.est_cost_range.0,
            pf.est_cost_range.1,
            chain_disp
        );
        let proceed = dialoguer::Confirm::new()
            .with_prompt("  proceed?")
            .default(true)
            .interact()
            .unwrap_or(true);
        if !proceed {
            eprintln!("  aborted — nothing was run, nothing was spent.");
            return Ok(());
        }
    }

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
                    if outcome.status == "completed" {
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
                    } else {
                        println!(
                            "\nRun {}. Cost so far: ${:.4}, Tokens: {} in / {} out",
                            outcome.status,
                            outcome.cost_usd,
                            outcome.tokens.input,
                            outcome.tokens.output,
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
