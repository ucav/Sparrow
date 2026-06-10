// src/cmd_handlers/handle_webview_cmd.rs — extracted from main.rs

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

// ─── Status command ────────────────────────────────────────────────────────────
