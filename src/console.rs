use secrecy::ExposeSecret;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use tokio::sync::{Mutex, broadcast, mpsc, oneshot};

use crate::auth::{AuthStore, Credential};
use crate::capabilities::SkillLibrary;
use crate::config::{Config, ConfigStore, FsConfigStore, ProviderConfig};
use crate::engine::{ApprovalHandler, ApprovalRequest};
use crate::event::{Decision, Event};
use crate::memory::{Memory, MemoryDocKind};
use crate::plan::ReadOnlyPlan;

// ─── Embedded HTML ─────────────────────────────────────────────────────────────
//
// console.html is `include_str!`d into the binary so a release build ships as
// a single file. The drawback: any edit to console.html requires a fresh
// `cargo build` — a plain WebView reload (Ctrl+R) re-fetches the same baked-in
// bytes. To make the WebView dev loop tight, set the env var
// `SPARROW_CONSOLE_HTML` to a path on disk and that file is served instead.

const CONSOLE_HTML_EMBEDDED: &str = include_str!("../console.html");

fn console_html() -> std::borrow::Cow<'static, str> {
    if let Ok(path) = std::env::var("SPARROW_CONSOLE_HTML") {
        if !path.trim().is_empty() {
            match std::fs::read_to_string(&path) {
                Ok(contents) => return std::borrow::Cow::Owned(contents),
                Err(e) => {
                    tracing::warn!(
                        "SPARROW_CONSOLE_HTML={} unreadable ({}); falling back to embedded HTML",
                        path,
                        e
                    );
                }
            }
        }
    }
    std::borrow::Cow::Borrowed(CONSOLE_HTML_EMBEDDED)
}

fn looks_like_api_key(value: &str) -> bool {
    let value = value.trim();
    value.starts_with("sk-")
        || value.starts_with("nvapi-")
        || value.starts_with("gsk_")
        || value.starts_with("sk-or-")
        || value.len() > 40 && !value.chars().all(|c| c.is_ascii_uppercase() || c == '_')
}

// ─── WebView server ────────────────────────────────────────────────────────────

pub struct WebViewServer {
    addr: SocketAddr,
    event_tx: broadcast::Sender<Event>,
    command_tx: Option<mpsc::UnboundedSender<String>>,
    config: Option<Arc<RwLock<Config>>>,
    approvals: Option<Arc<WebApprovalBroker>>,
    skills: Option<Arc<dyn SkillLibrary>>,
    memory: Option<Arc<dyn Memory>>,
}

impl WebViewServer {
    pub fn new(
        addr: SocketAddr,
        event_tx: broadcast::Sender<Event>,
        command_tx: Option<mpsc::UnboundedSender<String>>,
        config: Option<Arc<RwLock<Config>>>,
        approvals: Option<Arc<WebApprovalBroker>>,
        skills: Option<Arc<dyn SkillLibrary>>,
        memory: Option<Arc<dyn Memory>>,
    ) -> Self {
        Self {
            addr,
            event_tx,
            command_tx,
            config,
            approvals,
            skills,
            memory,
        }
    }

    pub async fn serve(&self) -> anyhow::Result<()> {
        use axum::{
            Router,
            extract::{State, ws::WebSocketUpgrade},
            response::Html,
            routing::{get, post},
        };

        let event_tx = self.event_tx.clone();
        let state = Arc::new(AppState {
            event_tx: event_tx.clone(),
            command_tx: self.command_tx.clone(),
            config: self.config.clone(),
            approvals: self.approvals.clone(),
            skills: self.skills.clone(),
            memory: self.memory.clone(),
        });

        let app = Router::new()
            // Reads from disk if `SPARROW_CONSOLE_HTML` is set (live-reload
            // friendly: Ctrl+R picks up edits without recompile). Otherwise
            // serves the include_str!()'d copy baked at compile time.
            .route("/", get(|| async { Html(console_html().into_owned()) }))
            .route("/run", post(run_task))
            .route("/plan", post(plan_task))
            .route("/commands", get(get_commands))
            .route("/memory", get(get_memory))
            .route("/plugins", get(get_plugins))
            .route("/tools", get(get_tools))
            .route("/models", get(list_models))
            .route("/status", get(get_status))
            .route("/file", get(read_file))
            .route("/conversation/reset", post(reset_conversation))
            .route("/stop", post(stop_run))
            .route("/approval", post(resolve_approval))
            .route("/config", get(get_config).post(save_provider))
            .route("/permissions", get(get_permissions).post(save_permissions))
            .route("/security", get(get_security))
            .route("/sessions", get(list_sessions))
            .route("/sessions/load", post(load_session))
            .route("/history", get(get_history))
            .route("/agents", get(list_agents))
            .route("/upload", post(upload_attachment))
            .route("/artifacts", get(list_artifacts))
            .route("/providers/scan", post(scan_provider_models))
            .route("/routing", get(get_routing).post(save_routing))
            .route(
                "/ws",
                get(
                    move |ws: WebSocketUpgrade, State(state): State<Arc<AppState>>| async move {
                        let rx = state.event_tx.subscribe();
                        ws.on_upgrade(move |socket| handle_ws(socket, rx))
                    },
                ),
            )
            .with_state(state);

        let listener = tokio::net::TcpListener::bind(self.addr).await?;
        tracing::info!("WebView console: http://{}", self.addr);

        axum::serve(listener, app).await?;
        Ok(())
    }
}

#[derive(Clone)]
struct AppState {
    event_tx: broadcast::Sender<Event>,
    command_tx: Option<mpsc::UnboundedSender<String>>,
    config: Option<Arc<RwLock<Config>>>,
    approvals: Option<Arc<WebApprovalBroker>>,
    skills: Option<Arc<dyn SkillLibrary>>,
    memory: Option<Arc<dyn Memory>>,
}

#[derive(Default)]
pub struct WebApprovalBroker {
    pending: Mutex<HashMap<String, oneshot::Sender<Decision>>>,
}

impl WebApprovalBroker {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn resolve(&self, id: &str, decision: Decision) -> bool {
        let mut pending = self.pending.lock().await;
        pending
            .remove(id)
            .map(|tx| tx.send(decision).is_ok())
            .unwrap_or(false)
    }
}

#[async_trait::async_trait]
impl ApprovalHandler for WebApprovalBroker {
    async fn request_approval(&self, request: ApprovalRequest) -> Decision {
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(request.id, tx);
        }
        rx.await.unwrap_or(Decision::Deny)
    }
}

#[derive(serde::Deserialize)]
struct RunRequest {
    task: String,
    #[serde(default)]
    model_override: Option<String>,
}

#[derive(serde::Serialize)]
struct RunResponse {
    ok: bool,
    message: String,
}

#[derive(serde::Serialize)]
struct PlanResponse {
    ok: bool,
    message: String,
    plan: Option<ReadOnlyPlan>,
}

#[derive(serde::Serialize)]
struct CommandView {
    name: String,
    description: String,
    source: String,
}

#[derive(serde::Serialize)]
struct CommandsResponse {
    ok: bool,
    message: String,
    commands: Vec<CommandView>,
}

#[derive(serde::Deserialize)]
struct ApprovalResponseRequest {
    id: String,
    decision: String,
}

#[derive(serde::Serialize)]
struct ProviderView {
    name: String,
    label: String,
    adapter: String,
    base_url: Option<String>,
    models: Vec<String>,
    tags: Vec<String>,
    notes: String,
    api_key_env: Option<String>,
    has_credential: bool,
    configured: bool,
}

#[derive(serde::Serialize)]
struct BudgetView {
    session_usd: f64,
    daily_usd: f64,
}

#[derive(serde::Serialize)]
struct ConfigResponse {
    ok: bool,
    message: String,
    autonomy: String,
    sandbox: String,
    providers: Vec<ProviderView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    budget: Option<BudgetView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    workdir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skills_count: Option<usize>,
}

#[derive(serde::Serialize)]
struct PermissionsResponse {
    ok: bool,
    message: String,
    permissions: Option<crate::permissions::PermissionConfig>,
}

#[derive(serde::Deserialize)]
struct PermissionsRequest {
    mode: Option<String>,
}

#[derive(serde::Serialize)]
struct MemoryDocView {
    kind: String,
    chars: usize,
    limit: usize,
    updated_at: String,
    content: String,
}

#[derive(serde::Serialize)]
struct MemoryFactView {
    id: String,
    key: String,
    value: String,
    updated_at: String,
}

#[derive(serde::Serialize)]
struct MemoryResponse {
    ok: bool,
    message: String,
    stats: Option<crate::memory::MemoryStats>,
    docs: Vec<MemoryDocView>,
    facts: Vec<MemoryFactView>,
}

#[derive(serde::Serialize)]
struct PluginView {
    name: String,
    version: String,
    description: String,
    commands: usize,
    skills: usize,
    hooks: usize,
    allowed: bool,
    warnings: Vec<String>,
}

#[derive(serde::Serialize)]
struct PluginsResponse {
    ok: bool,
    message: String,
    plugins: Vec<PluginView>,
}

#[derive(serde::Serialize)]
struct ToolsResponse {
    ok: bool,
    message: String,
    toolsets: Vec<String>,
    tools: Vec<crate::tools::ToolMetadata>,
}

#[derive(serde::Deserialize)]
struct HistoryQuery {
    limit: Option<usize>,
}

#[derive(serde::Serialize)]
struct HistoryResponse {
    ok: bool,
    message: String,
    inputs: Vec<String>,
}

#[derive(serde::Deserialize)]
struct ProviderRequest {
    #[serde(default)]
    name: String,
    #[serde(default)]
    adapter: String,
    base_url: Option<String>,
    #[serde(default)]
    models: Vec<String>,
    api_key_env: Option<String>,
    api_key: Option<String>,
    autonomy: Option<String>,
    sandbox: Option<String>,
}

async fn run_task(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    axum::extract::Json(req): axum::extract::Json<RunRequest>,
) -> axum::extract::Json<RunResponse> {
    let task = req.task.trim().to_string();
    if task.is_empty() {
        return axum::extract::Json(RunResponse {
            ok: false,
            message: "empty task".into(),
        });
    }

    // Prepend model override hint so the engine can parse it.
    // The frontend sends "provider:model" — strip the provider prefix to
    // match brain.id() which returns just the model name.
    let dispatch = if let Some(m) = req.model_override.filter(|s| !s.is_empty()) {
        let model_only = m.rsplit(':').next().unwrap_or(&m);
        format!("__model:{model_only}__ {task}")
    } else {
        task
    };
    match &state.command_tx {
        Some(tx) if tx.send(dispatch).is_ok() => axum::extract::Json(RunResponse {
            ok: true,
            message: "queued".into(),
        }),
        _ => axum::extract::Json(RunResponse {
            ok: false,
            message: "console command channel unavailable".into(),
        }),
    }
}

async fn plan_task(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    axum::extract::Json(req): axum::extract::Json<RunRequest>,
) -> axum::extract::Json<PlanResponse> {
    let task = req.task.trim().to_string();
    if task.is_empty() {
        return axum::extract::Json(PlanResponse {
            ok: false,
            message: "empty task".into(),
            plan: None,
        });
    }
    let commands = commands_for_state(&state);
    let plan = crate::plan::build_read_only_plan(&task, &commands);
    axum::extract::Json(PlanResponse {
        ok: true,
        message: "planned".into(),
        plan: Some(plan),
    })
}

async fn get_commands(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> axum::extract::Json<CommandsResponse> {
    let commands = commands_for_state(&state)
        .into_iter()
        .map(|cmd| CommandView {
            name: format!("/{}", cmd.name),
            description: cmd.description,
            source: match cmd.source {
                crate::commands::SlashCommandSource::Builtin => "builtin".into(),
                crate::commands::SlashCommandSource::Project(path) => {
                    format!("project:{}", path.display())
                }
                crate::commands::SlashCommandSource::User(path) => {
                    format!("user:{}", path.display())
                }
                crate::commands::SlashCommandSource::Skill(name) => format!("skill:{}", name),
                crate::commands::SlashCommandSource::Plugin(name) => format!("plugin:{}", name),
            },
        })
        .collect();
    axum::extract::Json(CommandsResponse {
        ok: true,
        message: "commands loaded".into(),
        commands,
    })
}

fn commands_for_state(state: &AppState) -> Vec<crate::commands::SlashCommand> {
    let project_root = std::env::current_dir().unwrap_or_default();
    let config_dir = state
        .config
        .as_ref()
        .and_then(|cfg| cfg.read().ok().map(|cfg| cfg.config_dir.clone()))
        .unwrap_or_else(|| {
            dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("sparrow")
        });
    crate::commands::all_commands(&project_root, &config_dir, state.skills.as_deref())
}

async fn get_memory(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> axum::extract::Json<MemoryResponse> {
    let Some(memory) = &state.memory else {
        return axum::extract::Json(MemoryResponse {
            ok: false,
            message: "memory unavailable".into(),
            stats: None,
            docs: Vec::new(),
            facts: Vec::new(),
        });
    };
    let stats = memory.memory_stats();
    let docs = [MemoryDocKind::Memory, MemoryDocKind::User]
        .into_iter()
        .filter_map(|kind| {
            memory.memory_doc(kind).map(|doc| MemoryDocView {
                kind: kind.as_str().to_string(),
                chars: doc.content.chars().count(),
                limit: kind.limit(),
                updated_at: doc.updated_at,
                content: doc.content,
            })
        })
        .collect();
    let facts = memory
        .all_facts()
        .into_iter()
        .take(25)
        .map(|fact| MemoryFactView {
            id: fact.id,
            key: fact.key,
            value: fact.value,
            updated_at: fact.updated_at,
        })
        .collect();
    axum::extract::Json(MemoryResponse {
        ok: true,
        message: "loaded".into(),
        stats: Some(stats),
        docs,
        facts,
    })
}

async fn get_plugins(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> axum::extract::Json<PluginsResponse> {
    let config_dir = state
        .config
        .as_ref()
        .and_then(|cfg| cfg.read().ok().map(|cfg| cfg.config_dir.clone()))
        .unwrap_or_else(|| {
            dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("sparrow")
        });
    let dirs = [
        std::env::current_dir()
            .unwrap_or_default()
            .join(".sparrow")
            .join("plugins"),
        config_dir.join("plugins"),
    ];
    let mut plugins = Vec::new();
    for dir in dirs {
        let registry = crate::capabilities::plugin::PluginRegistry::new(dir);
        for plugin in registry.scan() {
            let audit = registry.audit(&plugin);
            plugins.push(PluginView {
                name: plugin.manifest.name,
                version: plugin.manifest.version,
                description: plugin.manifest.description,
                commands: plugin.manifest.commands.len(),
                skills: plugin.manifest.skills.len(),
                hooks: plugin.manifest.hooks.len(),
                allowed: audit.allowed,
                warnings: audit.warnings,
            });
        }
    }
    axum::extract::Json(PluginsResponse {
        ok: true,
        message: "loaded".into(),
        plugins,
    })
}

async fn get_tools() -> axum::extract::Json<ToolsResponse> {
    axum::extract::Json(ToolsResponse {
        ok: true,
        message: "loaded".into(),
        toolsets: crate::tools::TOOLSETS
            .iter()
            .map(|set| set.to_string())
            .collect(),
        tools: crate::tools::known_tool_metadata(None),
    })
}

async fn list_models(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> axum::extract::Json<serde_json::Value> {
    use crate::config::providers::provider_registry;
    let providers = provider_registry();
    let out: Vec<serde_json::Value> = providers
        .iter()
        .map(|p| {
            // Curated models from the static registry.
            let mut models: Vec<serde_json::Value> = p
                .models
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "name": m.name,
                        "label": m.label,
                        "tags": m.tags,
                        "context_window": m.context_window,
                        "cost_in": m.cost_input_per_mtok,
                        "cost_out": m.cost_output_per_mtok,
                        "recommended": m.recommended,
                        "source": "registry",
                    })
                })
                .collect();
            // Merge live-discovered models from the SQLite cache (e.g. the 92
            // NVIDIA models) so the picker shows everything, not just curated.
            // For each discovered model, infer per-model caps from the name so
            // the WebView context-window meter adapts (DeepSeek V4 Pro = 1M,
            // not the previous hard-coded 128k default).
            if let Some(mem) = &state.memory {
                let curated: std::collections::HashSet<String> =
                    p.models.iter().map(|m| m.name.clone()).collect();
                for name in mem.get_discovered_models(&p.id) {
                    if !curated.contains(&name) {
                        let caps = crate::config::providers::model_caps(&p.id, &name);
                        models.push(serde_json::json!({
                            "name": name,
                            "label": name,
                            "tags": [],
                            "context_window": caps.context_window,
                            "max_output": caps.max_output,
                            "cost_in": caps.cost_input_per_mtok,
                            "cost_out": caps.cost_output_per_mtok,
                            "recommended": false,
                            "source": "discovered",
                        }));
                    }
                }
            }
            serde_json::json!({
                "id": p.id,
                "label": p.label,
                "tags": p.tags,
                "model_count": models.len(),
                "models": models,
            })
        })
        .collect();
    axum::extract::Json(serde_json::json!({ "ok": true, "providers": out }))
}

async fn stop_run(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> axum::extract::Json<RunResponse> {
    match &state.command_tx {
        Some(tx) if tx.send("__stop__".to_string()).is_ok() => axum::extract::Json(RunResponse {
            ok: true,
            message: "stop requested".into(),
        }),
        _ => axum::extract::Json(RunResponse {
            ok: false,
            message: "console command channel unavailable".into(),
        }),
    }
}

async fn reset_conversation(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> axum::extract::Json<RunResponse> {
    // Send a sentinel string through the command channel; main.rs interprets
    // __reset_conversation__ as a request to drain conv_history.
    match &state.command_tx {
        Some(tx) if tx.send("__reset_conversation__".to_string()).is_ok() => {
            axum::extract::Json(RunResponse {
                ok: true,
                message: "conversation cleared".into(),
            })
        }
        _ => axum::extract::Json(RunResponse {
            ok: false,
            message: "console command channel unavailable".into(),
        }),
    }
}

async fn get_status() -> axum::extract::Json<serde_json::Value> {
    use crate::config::providers::provider_registry;
    let providers = provider_registry();
    axum::extract::Json(serde_json::json!({
        "ok": true,
        "version": env!("CARGO_PKG_VERSION"),
        "providers_total": providers.len(),
        "workdir": std::env::current_dir().ok().map(|p| p.to_string_lossy().to_string()),
    }))
}

#[derive(serde::Deserialize)]
struct FileQuery {
    path: String,
}

async fn read_file(
    axum::extract::Query(q): axum::extract::Query<FileQuery>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    // Sandbox: only allow paths inside cwd.
    let cwd = match std::env::current_dir() {
        Ok(d) => d,
        Err(_) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "cwd unavailable",
            )
                .into_response();
        }
    };
    // Canonicalize cwd too so UNC prefixes match on Windows.
    let cwd_canon = cwd.canonicalize().unwrap_or(cwd.clone());
    let requested = std::path::Path::new(&q.path);
    let canonical = match cwd.join(requested).canonicalize() {
        Ok(p) => p,
        Err(_) => return (axum::http::StatusCode::NOT_FOUND, "file not found").into_response(),
    };
    if !canonical.starts_with(&cwd_canon) {
        return (axum::http::StatusCode::FORBIDDEN, "path outside workdir").into_response();
    }
    match std::fs::read_to_string(&canonical) {
        Ok(content) => {
            let ext = canonical.extension().and_then(|e| e.to_str()).unwrap_or("");
            let lang = match ext {
                "rs" => "rust",
                "js" | "ts" | "jsx" | "tsx" => "javascript",
                "py" => "python",
                "toml" => "toml",
                "md" => "markdown",
                "html" => "html",
                "css" => "css",
                "json" => "json",
                _ => "text",
            };
            axum::extract::Json(serde_json::json!({
                "ok": true, "path": q.path, "lang": lang,
                "lines": content.lines().count(),
                "content": content,
            }))
            .into_response()
        }
        Err(_) => (axum::http::StatusCode::NOT_FOUND, "cannot read file").into_response(),
    }
}

async fn resolve_approval(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    axum::extract::Json(req): axum::extract::Json<ApprovalResponseRequest>,
) -> axum::extract::Json<RunResponse> {
    let Some(approvals) = &state.approvals else {
        return axum::extract::Json(RunResponse {
            ok: false,
            message: "approval channel unavailable".into(),
        });
    };
    let decision = match req.decision.trim().to_lowercase().as_str() {
        "allow" | "approve" | "approved" => Decision::Allow,
        "deny" | "reject" | "rejected" => Decision::Deny,
        _ => {
            return axum::extract::Json(RunResponse {
                ok: false,
                message: "decision must be approve or deny".into(),
            });
        }
    };
    if approvals.resolve(req.id.trim(), decision).await {
        axum::extract::Json(RunResponse {
            ok: true,
            message: "approval resolved".into(),
        })
    } else {
        axum::extract::Json(RunResponse {
            ok: false,
            message: "approval not found or already resolved".into(),
        })
    }
}

async fn get_config(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> axum::extract::Json<ConfigResponse> {
    let Some(shared) = &state.config else {
        return axum::extract::Json(ConfigResponse {
            ok: false,
            message: "config unavailable".into(),
            budget: None,
            workdir: None,
            skills_count: None,
            autonomy: String::new(),
            sandbox: String::new(),
            providers: vec![],
        });
    };

    let cfg = shared.read().expect("config lock poisoned").clone();
    let auth = crate::auth::store::ChainedAuthStore::new(cfg.config_dir.clone());
    let mut providers = crate::config::providers::onboarding_providers()
        .into_iter()
        .map(|def| {
            let configured = cfg.providers.get(&def.id);
            let api_key_env = configured
                .and_then(|p| {
                    p.api_key_env
                        .as_ref()
                        .filter(|value| !looks_like_api_key(value))
                        .cloned()
                })
                .or_else(|| def.api_key_env.clone());
            let has_credential = auth.get(&def.id).is_some()
                || configured
                    .and_then(|p| p.api_key_env.as_ref())
                    .map(|value| {
                        looks_like_api_key(value)
                            || std::env::var(value)
                                .map(|env_value| !env_value.is_empty())
                                .unwrap_or(false)
                    })
                    .unwrap_or(false)
                || api_key_env
                    .as_ref()
                    .map(|value| {
                        std::env::var(value)
                            .map(|env_value| !env_value.is_empty())
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);

            // Merge configured + curated + discovered into a single sorted
            // unique list so the config panel's "X models" count survives
            // across sessions (was only counting the static registry, hiding
            // the 60+ models the user had just scanned).
            let mut models: Vec<String> = configured
                .map(|p| {
                    if p.models.is_empty() {
                        def.models.iter().map(|m| m.name.clone()).collect()
                    } else {
                        p.models.clone()
                    }
                })
                .unwrap_or_else(|| def.models.iter().map(|m| m.name.clone()).collect());
            if let Some(mem) = &state.memory {
                let known: std::collections::HashSet<String> = models.iter().cloned().collect();
                for name in mem.get_discovered_models(&def.id) {
                    if !known.contains(&name) {
                        models.push(name);
                    }
                }
            }
            ProviderView {
                name: def.id,
                label: def.label,
                adapter: configured.map(|p| p.adapter.clone()).unwrap_or(def.adapter),
                base_url: configured
                    .and_then(|p| p.base_url.clone())
                    .or(Some(def.base_url)),
                models,
                tags: def.tags,
                notes: def.notes,
                api_key_env,
                has_credential,
                configured: configured.is_some(),
            }
        })
        .collect::<Vec<_>>();

    for (name, p) in &cfg.providers {
        if providers.iter().any(|view| &view.name == name) {
            continue;
        }
        let api_key_env = p
            .api_key_env
            .as_ref()
            .filter(|value| !looks_like_api_key(value))
            .cloned();
        providers.push(ProviderView {
            name: name.clone(),
            label: name.clone(),
            adapter: p.adapter.clone(),
            base_url: p.base_url.clone(),
            models: p.models.clone(),
            tags: vec!["custom".into()],
            notes: "Custom configured provider.".into(),
            api_key_env: api_key_env.clone(),
            has_credential: auth.get(name).is_some()
                || p.api_key_env
                    .as_ref()
                    .map(|value| {
                        looks_like_api_key(value)
                            || std::env::var(value)
                                .map(|env_value| !env_value.is_empty())
                                .unwrap_or(false)
                    })
                    .unwrap_or(false),
            configured: true,
        });
    }
    providers.sort_by(|a, b| a.name.cmp(&b.name));

    axum::extract::Json(ConfigResponse {
        ok: true,
        message: "loaded".into(),
        autonomy: format!("{:?}", cfg.defaults.autonomy),
        sandbox: cfg.defaults.sandbox,
        providers,
        budget: Some(BudgetView {
            session_usd: cfg.budget.session_usd,
            daily_usd: cfg.budget.daily_usd,
        }),
        workdir: std::env::current_dir()
            .ok()
            .map(|p| p.to_string_lossy().to_string()),
        skills_count: None,
    })
}

async fn save_provider(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    axum::extract::Json(req): axum::extract::Json<ProviderRequest>,
) -> axum::extract::Json<RunResponse> {
    let Some(shared) = &state.config else {
        return axum::extract::Json(RunResponse {
            ok: false,
            message: "config unavailable".into(),
        });
    };

    let mut cfg = shared.write().expect("config lock poisoned");
    if let Some(level) = parse_autonomy(req.autonomy.as_deref()) {
        cfg.defaults.autonomy = level;
    }
    if let Some(sandbox) = req
        .sandbox
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        cfg.defaults.sandbox = sandbox;
    }

    let name = req.name.trim().to_lowercase();
    if name.is_empty() {
        let saved = cfg.clone();
        let store = FsConfigStore::new(saved.config_dir.clone());
        if let Err(err) = store.save(&saved) {
            return axum::extract::Json(RunResponse {
                ok: false,
                message: format!("config save failed: {}", err),
            });
        }
        return axum::extract::Json(RunResponse {
            ok: true,
            message: "runtime preferences saved".into(),
        });
    }

    let raw_api_key_env = req
        .api_key_env
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let api_key_env = raw_api_key_env
        .as_ref()
        .filter(|value| !looks_like_api_key(value))
        .cloned();
    let api_key_from_env_field = raw_api_key_env
        .as_ref()
        .filter(|value| looks_like_api_key(value))
        .cloned();

    cfg.providers.insert(
        name.clone(),
        ProviderConfig {
            adapter: req.adapter.trim().to_string(),
            base_url: req
                .base_url
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            models: req
                .models
                .into_iter()
                .map(|m| m.trim().to_string())
                .filter(|m| !m.is_empty())
                .collect(),
            api_key_env,
        },
    );

    let saved = cfg.clone();
    let store = FsConfigStore::new(saved.config_dir.clone());
    if let Err(err) = store.save(&saved) {
        return axum::extract::Json(RunResponse {
            ok: false,
            message: format!("config save failed: {}", err),
        });
    }

    if let Some(key) = req
        .api_key
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
        .or(api_key_from_env_field)
    {
        let auth = crate::auth::store::ChainedAuthStore::new(saved.config_dir);
        if let Err(err) = auth.set(&name, Credential::api_key(key)) {
            return axum::extract::Json(RunResponse {
                ok: false,
                message: format!("credential save failed: {}", err),
            });
        }
    }

    axum::extract::Json(RunResponse {
        ok: true,
        message: format!("provider '{}' saved", name),
    })
}

/// Hard cap for WebView attachments (10 MB).
pub const MAX_ATTACHMENT_BYTES: usize = 10 * 1024 * 1024;

/// Where attachments are stored, relative to the current working directory.
pub fn attachments_dir() -> std::path::PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".sparrow")
        .join("attachments")
}

#[derive(serde::Serialize)]
pub struct AttachmentMetadata {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub mime: String,
    pub kind: &'static str,
}

pub fn classify_attachment(mime: &str, ext: &str) -> &'static str {
    let ext = ext.to_ascii_lowercase();
    if mime.starts_with("image/")
        || matches!(
            ext.as_str(),
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
        )
    {
        "image"
    } else if mime.starts_with("audio/")
        || matches!(ext.as_str(), "mp3" | "wav" | "m4a" | "ogg" | "flac")
    {
        "audio"
    } else if mime == "application/pdf" || ext == "pdf" {
        "pdf"
    } else if mime.starts_with("text/")
        || matches!(
            ext.as_str(),
            "md" | "txt" | "csv" | "json" | "toml" | "yml" | "yaml"
        )
    {
        "text"
    } else {
        "file"
    }
}

async fn upload_attachment(
    mut multipart: axum::extract::Multipart,
) -> axum::extract::Json<serde_json::Value> {
    let dir = attachments_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return axum::extract::Json(serde_json::json!({
            "ok": false,
            "message": format!("could not create attachments dir: {}", e),
        }));
    }
    let mut accepted: Vec<AttachmentMetadata> = Vec::new();
    let mut rejected: Vec<serde_json::Value> = Vec::new();
    while let Ok(Some(field)) = multipart.next_field().await {
        let original = field
            .file_name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "upload.bin".into());
        let content_type = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();
        let data = match field.bytes().await {
            Ok(b) => b,
            Err(e) => {
                rejected.push(
                    serde_json::json!({"name": original, "reason": format!("read error: {}", e)}),
                );
                continue;
            }
        };
        if data.len() > MAX_ATTACHMENT_BYTES {
            rejected.push(serde_json::json!({
                "name": original,
                "reason": format!("too large: {} bytes > limit {}", data.len(), MAX_ATTACHMENT_BYTES),
            }));
            continue;
        }
        // Sanitize filename: strip directory components.
        let safe = std::path::Path::new(&original)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "upload.bin".into());
        let dest = dir.join(&safe);
        if let Err(e) = std::fs::write(&dest, &data) {
            rejected
                .push(serde_json::json!({"name": safe, "reason": format!("write error: {}", e)}));
            continue;
        }
        let ext = std::path::Path::new(&safe)
            .extension()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let kind = classify_attachment(&content_type, &ext);
        accepted.push(AttachmentMetadata {
            name: safe.clone(),
            path: dest.to_string_lossy().to_string(),
            size: data.len() as u64,
            mime: content_type,
            kind,
        });
    }

    axum::extract::Json(serde_json::json!({
        "ok": !accepted.is_empty(),
        "accepted": accepted,
        "rejected": rejected,
        "limit_bytes": MAX_ATTACHMENT_BYTES,
    }))
}

async fn list_artifacts() -> axum::extract::Json<serde_json::Value> {
    let dir = attachments_dir();
    let mut items: Vec<AttachmentMetadata> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let ext = path
                .extension()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let mime = mime_guess::from_path(&path)
                .first_or_octet_stream()
                .to_string();
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let kind = classify_attachment(&mime, &ext);
            items.push(AttachmentMetadata {
                name,
                path: path.to_string_lossy().to_string(),
                size,
                mime,
                kind,
            });
        }
    }
    axum::extract::Json(serde_json::json!({
        "ok": true,
        "items": items,
        "dir": dir.to_string_lossy().to_string(),
    }))
}

/// `GET /agents` — return every installed agent so the WebView swarm row and
/// the composer's `@<name>` picker can render the real list (Sprint 1, item
/// dynamic WebView swarm row. Status is "idle" by default; the runtime updates a
/// shared `Arc<Mutex<AgentRuntimeState>>` later — for v0.3.0 we ship the
/// listing as a cold view backed by `FsAgentStore::list()`.
async fn list_agents() -> axum::extract::Json<serde_json::Value> {
    use crate::agent::{AgentStore, FsAgentStore};

    let agents_dir = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("sparrow")
        .join("agents");

    // Collect souls from user config dir + local repo `agents/` dir + `.sparrow/agents/`.
    let extra_dirs: Vec<std::path::PathBuf> = [
        std::env::current_dir().ok().map(|d| d.join("agents")),
        std::env::current_dir()
            .ok()
            .map(|d| d.join(".sparrow").join("agents")),
    ]
    .into_iter()
    .flatten()
    .filter(|p| p.is_dir())
    .collect();

    let store = FsAgentStore::new(agents_dir.clone());
    let mut souls = store.list();
    let mut seen: std::collections::HashSet<String> =
        souls.iter().map(|s| s.name.clone()).collect();
    for dir in &extra_dirs {
        let extra = FsAgentStore::new(dir.clone()).list();
        for s in extra {
            if seen.insert(s.name.clone()) {
                souls.push(s);
            }
        }
    }

    let items: Vec<serde_json::Value> = souls
        .into_iter()
        .map(|s| {
            // Pick a colour key the WebView already knows about; falls back to
            // the canonical triad if the agent uses one of those role names.
            let color_key = match s.role.to_lowercase().as_str() {
                "planner" => "planner",
                "coder" => "coder",
                "verifier" => "verifier",
                _ => s
                    .color
                    .as_deref()
                    .map(classify_agent_color)
                    .unwrap_or("steel"),
            };
            serde_json::json!({
                "name": s.name,
                "role": s.role,
                "description": s.description,
                "status": "idle",
                "msg": "",
                "color_key": color_key,
            })
        })
        .collect();

    axum::extract::Json(serde_json::json!({
        "ok": true,
        "dir": agents_dir.to_string_lossy(),
        "agents": items,
    }))
}

/// Maps the optional `color` field of a `Soul` to one of the known WebView
/// theme tokens. Unknown values fall back to `steel`.
pub fn classify_agent_color(raw: &str) -> &'static str {
    match raw.trim().to_lowercase().as_str() {
        "planner" | "blue" => "planner",
        "coder" | "teal" | "agent" => "coder",
        "verifier" | "sand" => "verifier",
        "gold" | "yellow" => "gold",
        "coral" | "red" => "coral",
        _ => "steel",
    }
}

#[derive(serde::Deserialize)]
struct LoadSessionRequest {
    id: String,
}

async fn load_session(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    axum::extract::Json(req): axum::extract::Json<LoadSessionRequest>,
) -> axum::extract::Json<RunResponse> {
    let id = req.id.trim();
    if id.is_empty() {
        return axum::extract::Json(RunResponse {
            ok: false,
            message: "empty session id".into(),
        });
    }
    // Reuse the same command channel sentinel mechanism that powers
    // __reset_conversation__ — main.rs interprets __load_session__:<id> and
    // swaps the live `conv_history` with the loaded session's messages.
    let sentinel = format!("__load_session__:{}", id);
    match &state.command_tx {
        Some(tx) if tx.send(sentinel).is_ok() => axum::extract::Json(RunResponse {
            ok: true,
            message: "session load requested".into(),
        }),
        _ => axum::extract::Json(RunResponse {
            ok: false,
            message: "console command channel unavailable".into(),
        }),
    }
}

async fn list_sessions() -> axum::extract::Json<serde_json::Value> {
    // Resolve the same DB path the CLI uses. Failures degrade to an empty list
    // rather than 500'ing the WebView panel.
    let db_path = session_db_path();
    let store = match crate::runtime::session::SessionStore::open(&db_path) {
        Ok(s) => s,
        Err(e) => {
            return axum::extract::Json(serde_json::json!({
                "ok": false,
                "message": format!("could not open session db: {}", e),
                "db_path": db_path.to_string_lossy(),
                "sessions": [],
            }));
        }
    };
    let sessions = store.list();
    axum::extract::Json(serde_json::json!({
        "ok": true,
        "db_path": db_path.to_string_lossy(),
        "sessions": sessions,
    }))
}

async fn get_history(
    axum::extract::Query(query): axum::extract::Query<HistoryQuery>,
) -> axum::extract::Json<HistoryResponse> {
    let db_path = session_db_path();
    let store = match crate::runtime::session::SessionStore::open(&db_path) {
        Ok(s) => s,
        Err(e) => {
            return axum::extract::Json(HistoryResponse {
                ok: false,
                message: format!("could not open session db: {}", e),
                inputs: Vec::new(),
            });
        }
    };
    axum::extract::Json(HistoryResponse {
        ok: true,
        message: "loaded".into(),
        inputs: store.recent_inputs(query.limit.unwrap_or(50)),
    })
}

fn session_db_path() -> std::path::PathBuf {
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .or_else(dirs::data_dir)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("sparrow")
        .join("sessions.db")
}

async fn get_security(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> axum::extract::Json<serde_json::Value> {
    let Some(shared) = &state.config else {
        return axum::extract::Json(serde_json::json!({
            "ok": false,
            "message": "config unavailable",
        }));
    };
    let cfg = shared.read().expect("config lock poisoned").clone();
    let audit = crate::security::SecurityAudit::run(&cfg, &cfg.hooks);
    axum::extract::Json(serde_json::json!({
        "ok": true,
        "audit": audit,
    }))
}

async fn get_permissions(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> axum::extract::Json<PermissionsResponse> {
    let Some(shared) = &state.config else {
        return axum::extract::Json(PermissionsResponse {
            ok: false,
            message: "config unavailable".into(),
            permissions: None,
        });
    };
    let cfg = shared.read().expect("config lock poisoned").clone();
    axum::extract::Json(PermissionsResponse {
        ok: true,
        message: "loaded".into(),
        permissions: Some(cfg.permissions),
    })
}

async fn save_permissions(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    axum::extract::Json(req): axum::extract::Json<PermissionsRequest>,
) -> axum::extract::Json<RunResponse> {
    let Some(shared) = &state.config else {
        return axum::extract::Json(RunResponse {
            ok: false,
            message: "config unavailable".into(),
        });
    };
    let mut cfg = shared.write().expect("config lock poisoned");
    if let Some(mode) = req.mode.as_deref() {
        let Some(mode) = crate::permissions::PermissionMode::parse(mode) else {
            return axum::extract::Json(RunResponse {
                ok: false,
                message: "unknown permission mode".into(),
            });
        };
        cfg.defaults.autonomy = mode.autonomy_level();
        cfg.permissions.mode = mode;
    }
    let saved = cfg.clone();
    let store = FsConfigStore::new(saved.config_dir.clone());
    if let Err(err) = store.save(&saved) {
        return axum::extract::Json(RunResponse {
            ok: false,
            message: format!("permissions save failed: {}", err),
        });
    }
    axum::extract::Json(RunResponse {
        ok: true,
        message: "permissions saved".into(),
    })
}

fn parse_autonomy(value: Option<&str>) -> Option<crate::event::AutonomyLevel> {
    match value.map(|s| s.trim().to_lowercase()).as_deref() {
        Some("supervised") => Some(crate::event::AutonomyLevel::Supervised),
        Some("trusted") => Some(crate::event::AutonomyLevel::Trusted),
        Some("autonomous") => Some(crate::event::AutonomyLevel::Autonomous),
        _ => None,
    }
}

// ─── Provider model scan ──────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct ScanRequest {
    provider: String,
}

#[derive(serde::Serialize)]
struct ScanResponse {
    ok: bool,
    message: String,
    models: Vec<String>,
}

async fn scan_provider_models(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    axum::extract::Json(req): axum::extract::Json<ScanRequest>,
) -> axum::extract::Json<ScanResponse> {
    use crate::config::providers::find_provider;

    let provider_id = req.provider.trim().to_string();

    let Some(def) = find_provider(&provider_id) else {
        return axum::extract::Json(ScanResponse {
            ok: false,
            message: format!("Unknown provider: {}", provider_id),
            models: vec![],
        });
    };

    // Resolve API key: auth store -> env var
    let api_key = {
        let key_from_store = state.config.as_ref().and_then(|cfg| {
            let c = cfg.read().ok()?;
            let auth = crate::auth::store::ChainedAuthStore::new(c.config_dir.clone());
            match auth.get(&provider_id) {
                Some(crate::auth::Credential::ApiKey(k)) => Some(k.expose_secret().to_string()),
                _ => None,
            }
        });
        let key_from_env = def
            .api_key_env
            .as_deref()
            .and_then(|env| std::env::var(env).ok());
        key_from_store.or(key_from_env).unwrap_or_default()
    };

    match crate::provider::discovery::discover_models(&def.adapter, &def.base_url, &api_key).await {
        Ok(models) => {
            let count = models.len();
            axum::extract::Json(ScanResponse {
                ok: true,
                message: format!("Found {} model(s) for {}", count, def.label),
                models,
            })
        }
        Err(err) => axum::extract::Json(ScanResponse {
            ok: false,
            message: format!("Scan failed: {}", err),
            models: vec![],
        }),
    }
}

// ─── Routing config get/set ───────────────────────────────────────────────────

#[derive(serde::Serialize)]
struct RoutingResponse {
    ok: bool,
    preferred_provider: Option<String>,
    auto_discover: bool,
    all_providers: Vec<String>,
}

#[derive(serde::Deserialize)]
struct RoutingRequest {
    /// Set to "" or null to clear the preference.
    preferred_provider: Option<String>,
    auto_discover: Option<bool>,
}

async fn get_routing(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> axum::extract::Json<RoutingResponse> {
    use crate::config::providers::provider_registry;

    let all_providers: Vec<String> = provider_registry().iter().map(|p| p.id.clone()).collect();

    let Some(shared) = &state.config else {
        return axum::extract::Json(RoutingResponse {
            ok: false,
            preferred_provider: None,
            auto_discover: true,
            all_providers,
        });
    };

    let cfg = shared.read().expect("config lock poisoned");
    axum::extract::Json(RoutingResponse {
        ok: true,
        preferred_provider: cfg.routing.preferred_provider.clone(),
        auto_discover: cfg.routing.auto_discover,
        all_providers,
    })
}

async fn save_routing(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    axum::extract::Json(req): axum::extract::Json<RoutingRequest>,
) -> axum::extract::Json<RunResponse> {
    let Some(shared) = &state.config else {
        return axum::extract::Json(RunResponse {
            ok: false,
            message: "config unavailable".into(),
        });
    };

    {
        let mut cfg = shared.write().expect("config lock poisoned");

        // preferred_provider: empty string or missing = clear
        cfg.routing.preferred_provider = req
            .preferred_provider
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        if let Some(ad) = req.auto_discover {
            cfg.routing.auto_discover = ad;
        }

        let saved = cfg.clone();
        let store = FsConfigStore::new(saved.config_dir.clone());
        if let Err(err) = store.save(&saved) {
            return axum::extract::Json(RunResponse {
                ok: false,
                message: format!("save failed: {}", err),
            });
        }
    }

    axum::extract::Json(RunResponse {
        ok: true,
        message: "Routing preferences saved.".into(),
    })
}

async fn handle_ws(
    mut socket: axum::extract::ws::WebSocket,
    mut event_rx: tokio::sync::broadcast::Receiver<Event>,
) {
    loop {
        tokio::select! {
            result = event_rx.recv() => {
                match result {
                    Ok(event) => {
                        if let Ok(json) = serde_json::to_string(&event) {
                            use axum::extract::ws::Message;
                            if socket.send(Message::Text(json.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {
                // Ping keep-alive
                use axum::extract::ws::Message;
                if socket.send(Message::Ping(vec![])).await.is_err() {
                    break;
                }
            }
        }
    }
}
