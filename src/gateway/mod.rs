use std::sync::Arc;
use tokio::sync::mpsc;

use crate::engine::{Engine, Task};
use crate::event::Event;
use crate::runtime::recorder::{FsRecorder, Recorder, RunInputs};

pub mod discord;
pub mod email;
pub mod extra_transports;
pub mod slack;
pub mod telegram;
pub mod ws;

// ─── Gateway message types ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct GatewayMessage {
    pub surface: String,
    pub user_id: String,
    pub chat_id: String,
    pub text: String,
    pub message_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GatewayResponse {
    pub surface: String,
    pub chat_id: String,
    pub text: String,
    pub reply_to: Option<String>,
    pub buttons: Vec<Vec<String>>,
}

// ─── THE GATEWAY TRAIT ──────────────────────────────────────────────────────────

#[async_trait::async_trait]
pub trait GatewayTransport: Send + Sync {
    fn name(&self) -> &str;
    async fn start(&self, tx: mpsc::UnboundedSender<GatewayMessage>) -> anyhow::Result<()>;
    async fn send(&self, response: GatewayResponse) -> anyhow::Result<()>;
    async fn stop(&self) -> anyhow::Result<()>;
}

// ─── Message router: maps incoming messages to engine tasks ─────────────────────

pub struct MessageRouter {
    engine: Arc<Engine>,
    recorder: Arc<FsRecorder>,
    event_bus_tx: tokio::sync::broadcast::Sender<Event>,
    allowed_users: Vec<String>,
    /// Cross-surface session continuity (§8). Keyed by user identity so the same
    /// user resumes the same conversation/context regardless of surface.
    sessions: Option<Arc<crate::runtime::session::SessionStore>>,
}

impl MessageRouter {
    pub fn new(
        engine: Arc<Engine>,
        recorder: Arc<FsRecorder>,
        event_bus_tx: tokio::sync::broadcast::Sender<Event>,
        allowed_users: Vec<String>,
    ) -> Self {
        Self {
            engine,
            recorder,
            event_bus_tx,
            allowed_users,
            sessions: None,
        }
    }

    /// Attach a session store to enable cross-surface conversation continuity.
    pub fn with_sessions(mut self, sessions: Arc<crate::runtime::session::SessionStore>) -> Self {
        self.sessions = Some(sessions);
        self
    }

    /// Stable session key for a message. Keyed on user identity so the same user
    /// continues the same session across CLI/Telegram/Slack (§8). Falls back to
    /// surface:chat_id when no user id is present.
    fn session_key(msg_user_id: &str, surface: &str, chat_id: &str) -> String {
        if !msg_user_id.is_empty() {
            format!("user:{}", msg_user_id)
        } else {
            format!("{}:{}", surface, chat_id)
        }
    }

    /// Route an incoming message: parse command, submit to engine, return response
    pub async fn route(
        &self,
        msg: GatewayMessage,
        responses: &mpsc::UnboundedSender<GatewayResponse>,
    ) {
        // Check user authorization
        if !self.allowed_users.is_empty() && !self.allowed_users.contains(&msg.user_id) {
            let _ = responses.send(GatewayResponse {
                surface: msg.surface.clone(),
                chat_id: msg.chat_id.clone(),
                text: "Unauthorized. Ask the admin to add your user ID.".into(),
                reply_to: msg.message_id,
                buttons: vec![],
            });
            return;
        }

        let text = msg.text.trim();
        let surface = msg.surface.clone();
        let chat_id = msg.chat_id.clone();
        let user_id = msg.user_id.clone();
        let reply_to = msg.message_id.clone();

        if text.is_empty() {
            return;
        }

        // Command parsing
        if text.starts_with('/') {
            self.handle_command(text, surface, chat_id, user_id, reply_to, responses)
                .await;
        } else {
            self.handle_task(text, surface, chat_id, user_id, reply_to, responses)
                .await;
        }
    }

    async fn handle_command(
        &self,
        text: &str,
        surface: String,
        chat_id: String,
        user_id: String,
        reply_to: Option<String>,
        responses: &mpsc::UnboundedSender<GatewayResponse>,
    ) {
        let parts: Vec<&str> = text.splitn(2, ' ').collect();
        let cmd = parts[0].to_lowercase();
        let args = parts.get(1).unwrap_or(&"");

        match cmd.as_str() {
            "/start" | "/help" => {
                let _ = responses.send(GatewayResponse {
                    surface,
                    chat_id,
                    text: format!(
                        "Sparrow — one cli · grows with you\n\n\
                         Commands:\n\
                         /run <task> — Execute a task\n\
                         /status — Show engine status\n\
                         /models — List configured models\n\
                         /budget — Show budget status\n\
                         /help — This message\n\n\
                         Or just send a message to start a task."
                    ),
                    reply_to,
                    buttons: vec![vec!["/run ".into(), "/status".into()]],
                });
            }
            "/run" => {
                if args.is_empty() {
                    let _ = responses.send(GatewayResponse {
                        surface,
                        chat_id,
                        text: "Usage: /run <task description>".into(),
                        reply_to,
                        buttons: vec![],
                    });
                    return;
                }
                self.handle_task(args, surface, chat_id, user_id, reply_to, responses)
                    .await;
            }
            "/reset" => {
                // Clear the cross-surface session for this user
                if let Some(sessions) = &self.sessions {
                    let key = Self::session_key(&user_id, &surface, &chat_id);
                    let _ = sessions.delete(&key);
                }
                let _ = responses.send(GatewayResponse {
                    surface,
                    chat_id,
                    text: "Session cleared. Next message starts fresh.".into(),
                    reply_to,
                    buttons: vec![],
                });
            }
            "/status" => {
                let _ = responses.send(GatewayResponse {
                    surface,
                    chat_id,
                    text: "Engine: online\nMode: headless".into(),
                    reply_to,
                    buttons: vec![],
                });
            }
            "/models" => {
                let _ = responses.send(GatewayResponse {
                    surface,
                    chat_id,
                    text: "Use 'sparrow model --list' in CLI for model listing.".into(),
                    reply_to,
                    buttons: vec![],
                });
            }
            "/budget" => {
                let _ = responses.send(GatewayResponse {
                    surface,
                    chat_id,
                    text: "Budget: configured in ~/.config/sparrow/config.toml".into(),
                    reply_to,
                    buttons: vec![],
                });
            }
            _ => {
                let _ = responses.send(GatewayResponse {
                    surface,
                    chat_id,
                    text: format!("Unknown command: {}. Try /help", cmd),
                    reply_to,
                    buttons: vec![],
                });
            }
        }
    }

    async fn handle_task(
        &self,
        text: &str,
        surface: String,
        chat_id: String,
        user_id: String,
        reply_to: Option<String>,
        responses: &mpsc::UnboundedSender<GatewayResponse>,
    ) {
        let task_text = text.to_string();
        let resp_tx = responses.clone();
        let cid = chat_id.clone();
        let surface_for_done = surface.clone();

        // Clone for second spawn
        let resp_tx2 = resp_tx.clone();
        let cid2 = cid.clone();
        let surface_for_stream = surface.clone();
        let reply_to2 = reply_to.clone();

        // ── Session continuity (§8) ───────────────────────────────────────────
        // Load prior conversation for this user so context follows them across
        // surfaces and survives gateway restarts.
        let session_key = Self::session_key(&user_id, &surface, &chat_id);
        let prior_msgs: Vec<crate::provider::Msg> = self
            .sessions
            .as_ref()
            .and_then(|s| s.load(&session_key))
            .and_then(|sess| serde_json::from_str(&sess.messages_json).ok())
            .unwrap_or_default();
        let sessions_for_save = self.sessions.clone();
        let session_key_save = session_key.clone();
        let prior_for_save = prior_msgs.clone();

        // Create a one-shot event stream for this task
        let (task_tx, mut task_rx) = mpsc::unbounded_channel::<Event>();
        let event_bus = self.event_bus_tx.clone();
        let engine = self.engine.clone();
        let recorder = self.recorder.clone();

        // Send initial "thinking" response
        let _ = resp_tx.send(GatewayResponse {
            surface: surface.clone(),
            chat_id: cid.clone(),
            text: format!("Working on: {}", &task_text[..task_text.len().min(80)]),
            reply_to: reply_to.clone(),
            buttons: vec![],
        });

        // Start recording
        let run_id = uuid::Uuid::new_v4().to_string();
        recorder.start_run(
            run_id.clone(),
            RunInputs {
                task: task_text.clone(),
                config_snapshot: serde_json::json!({}),
                model_id: "gateway".into(),
                repo_head: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
                agent: "gateway".into(),
            },
        );

        tokio::spawn(async move {
            let task = Task {
                description: task_text.clone(),
                context: prior_msgs,
            };

            match engine.drive(task, task_tx.clone()).await {
                Ok(outcome) => {
                    let _ = event_bus.send(Event::RunFinished {
                        run: crate::event::RunId(run_id.clone()),
                        outcome: outcome.clone(),
                    });
                    let _ = recorder.finalize(&run_id);
                    let _ = resp_tx.send(GatewayResponse {
                        surface: surface_for_done,
                        chat_id: cid.clone(),
                        text: format!(
                            "Done.\nStatus: {}\nCost: ${:.4}\nFiles: {}",
                            outcome.status,
                            outcome.cost_usd,
                            outcome.diffs.len()
                        ),
                        reply_to: reply_to.clone(),
                        buttons: vec![],
                    });
                }
                Err(e) => {
                    let _ = resp_tx.send(GatewayResponse {
                        surface: surface_for_done,
                        chat_id: cid,
                        text: format!("Error: {}", e),
                        reply_to: reply_to2,
                        buttons: vec![],
                    });
                }
            }

            drop(task_tx);
        });

        // Stream intermediate updates
        let user_task_text = text.to_string();
        tokio::spawn(async move {
            let mut buffer = String::new();
            let mut full_reply = String::new();
            while let Some(event) = task_rx.recv().await {
                if let Event::ThinkingDelta { text, .. } = &event {
                    full_reply.push_str(text);
                }
                match &event {
                    Event::ThinkingDelta { text, .. } => {
                        buffer.push_str(text);
                        if buffer.len() > 500 || buffer.contains('\n') {
                            let _ = resp_tx2.send(GatewayResponse {
                                surface: surface_for_stream.clone(),
                                chat_id: cid2.clone(),
                                text: buffer.clone(),
                                reply_to: None,
                                buttons: vec![],
                            });
                            buffer.clear();
                        }
                    }
                    Event::ToolUseProposed { name, .. } => {
                        if !buffer.is_empty() {
                            let _ = resp_tx2.send(GatewayResponse {
                                surface: surface_for_stream.clone(),
                                chat_id: cid2.clone(),
                                text: buffer.clone(),
                                reply_to: None,
                                buttons: vec![],
                            });
                            buffer.clear();
                        }
                        let _ = resp_tx2.send(GatewayResponse {
                            surface: surface_for_stream.clone(),
                            chat_id: cid2.clone(),
                            text: format!("[Tool: {}]", name),
                            reply_to: None,
                            buttons: vec![],
                        });
                    }
                    Event::ModelSwitched {
                        from, to, reason, ..
                    } => {
                        if !buffer.is_empty() {
                            let _ = resp_tx2.send(GatewayResponse {
                                surface: surface_for_stream.clone(),
                                chat_id: cid2.clone(),
                                text: buffer.clone(),
                                reply_to: None,
                                buttons: vec![],
                            });
                            buffer.clear();
                        }
                        let clean = crate::event::friendly_model_switch_reason(reason);
                        let text = if crate::event::is_local_model_unavailable(reason) {
                            format!("modèle local indisponible → routage modèle cloud ({})", to)
                        } else {
                            format!("fallback: {} → {} ({})", from, to, clean)
                        };
                        let _ = resp_tx2.send(GatewayResponse {
                            surface: surface_for_stream.clone(),
                            chat_id: cid2.clone(),
                            text,
                            reply_to: None,
                            buttons: vec![],
                        });
                    }
                    Event::ApprovalRequested { summary, .. } => {
                        let _ = resp_tx2.send(GatewayResponse {
                            surface: surface_for_stream.clone(),
                            chat_id: cid2.clone(),
                            text: format!("Approval needed: {}", summary),
                            reply_to: None,
                            buttons: vec![vec!["/approve".into(), "/deny".into()]],
                        });
                    }
                    _ => {}
                }
            }
            if !buffer.is_empty() {
                let _ = resp_tx2.send(GatewayResponse {
                    surface: surface_for_stream,
                    chat_id: cid2.clone(),
                    text: buffer,
                    reply_to: None,
                    buttons: vec![],
                });
            }

            // ── Persist the turn to the session (§8) ──────────────────────────
            // Append the user message and the assistant reply so the next message
            // — on any surface — resumes with full context.
            if let Some(sessions) = &sessions_for_save {
                let mut updated = prior_for_save;
                updated.push(crate::provider::Msg {
                    role: "user".into(),
                    content: vec![crate::provider::ContentBlock::Text {
                        text: user_task_text,
                    }],
                });
                if !full_reply.trim().is_empty() {
                    updated.push(crate::provider::Msg {
                        role: "assistant".into(),
                        content: vec![crate::provider::ContentBlock::Text { text: full_reply }],
                    });
                }
                // Cap session history to the last 40 messages to bound growth.
                let len = updated.len();
                if len > 40 {
                    updated.drain(..len - 40);
                }
                let _ = sessions.save(&session_key_save, &updated, None);
            }
        });
    }
}

// ─── Event formatter: Event → human-readable message ────────────────────────────

pub fn format_event(event: &Event) -> Option<String> {
    match event {
        Event::RunStarted { task, agent, .. } => {
            Some(format!("Started: {} (agent: {})", task, agent))
        }
        Event::RunFinished { outcome, .. } => Some(format!(
            "Finished: {} | Cost: ${:.4} | Files: {}",
            outcome.status,
            outcome.cost_usd,
            outcome.diffs.len()
        )),
        Event::ThinkingDelta { text, .. } => Some(text.clone()),
        Event::ModelSwitched {
            from, to, reason, ..
        } => {
            let clean = crate::event::friendly_model_switch_reason(reason);
            if crate::event::is_local_model_unavailable(reason) {
                Some(format!(
                    "modèle local indisponible → routage modèle cloud ({})",
                    to
                ))
            } else {
                Some(format!("Fallback: {} → {} ({})", from, to, clean))
            }
        }
        Event::ToolUseProposed { name, .. } => Some(format!("[{}]", name)),
        Event::ApprovalRequested { summary, .. } => Some(format!("Approve: {}", summary)),
        Event::Error { message, .. } => {
            if crate::event::is_local_model_unavailable(message) {
                None
            } else {
                Some(format!("Error: {}", message))
            }
        }
        Event::CostUpdate { usd, .. } => Some(format!("Cost: ${:.4}", usd)),
        Event::CheckpointCreated { label, .. } => Some(format!("Checkpoint: {}", label)),
        _ => None,
    }
}
