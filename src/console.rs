use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use tokio::sync::{broadcast, mpsc};

use crate::auth::{AuthStore, Credential};
use crate::config::{Config, ConfigStore, FsConfigStore, ProviderConfig};
use crate::event::Event;

// ─── Embedded HTML ─────────────────────────────────────────────────────────────

const CONSOLE_HTML: &str = include_str!("../console.html");

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
}

impl WebViewServer {
    pub fn new(
        addr: SocketAddr,
        event_tx: broadcast::Sender<Event>,
        command_tx: Option<mpsc::UnboundedSender<String>>,
        config: Option<Arc<RwLock<Config>>>,
    ) -> Self {
        Self { addr, event_tx, command_tx, config }
    }

    pub async fn serve(&self) -> anyhow::Result<()> {
        use axum::{
            extract::{
                ws::WebSocketUpgrade,
                State,
            },
            response::Html,
            routing::{get, post},
            Router,
        };

        let event_tx = self.event_tx.clone();
        let state = Arc::new(AppState {
            event_tx: event_tx.clone(),
            command_tx: self.command_tx.clone(),
            config: self.config.clone(),
        });

        let app = Router::new()
            .route("/", get(|| async { Html(CONSOLE_HTML) }))
            .route("/run", post(run_task))
            .route("/config", get(get_config).post(save_provider))
            .route("/ws", get(move |ws: WebSocketUpgrade, State(state): State<Arc<AppState>>| async move {
                let rx = state.event_tx.subscribe();
                ws.on_upgrade(move |socket| handle_ws(socket, rx))
            }))
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
}

#[derive(serde::Deserialize)]
struct RunRequest {
    task: String,
}

#[derive(serde::Serialize)]
struct RunResponse {
    ok: bool,
    message: String,
}

#[derive(serde::Serialize)]
struct ProviderView {
    name: String,
    adapter: String,
    base_url: Option<String>,
    models: Vec<String>,
    api_key_env: Option<String>,
    has_credential: bool,
}

#[derive(serde::Serialize)]
struct ConfigResponse {
    ok: bool,
    message: String,
    autonomy: String,
    sandbox: String,
    providers: Vec<ProviderView>,
}

#[derive(serde::Deserialize)]
struct ProviderRequest {
    name: String,
    adapter: String,
    base_url: Option<String>,
    models: Vec<String>,
    api_key_env: Option<String>,
    api_key: Option<String>,
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

    match &state.command_tx {
        Some(tx) if tx.send(task).is_ok() => axum::extract::Json(RunResponse {
            ok: true,
            message: "queued".into(),
        }),
        _ => axum::extract::Json(RunResponse {
            ok: false,
            message: "console command channel unavailable".into(),
        }),
    }
}

async fn get_config(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> axum::extract::Json<ConfigResponse> {
    let Some(shared) = &state.config else {
        return axum::extract::Json(ConfigResponse {
            ok: false,
            message: "config unavailable".into(),
            autonomy: String::new(),
            sandbox: String::new(),
            providers: vec![],
        });
    };

    let cfg = shared.read().expect("config lock poisoned").clone();
    let auth = crate::auth::store::ChainedAuthStore::new(cfg.config_dir.clone());
    let mut providers = cfg
        .providers
        .iter()
        .map(|(name, p)| ProviderView {
            name: name.clone(),
            adapter: p.adapter.clone(),
            base_url: p.base_url.clone(),
            models: p.models.clone(),
            api_key_env: p
                .api_key_env
                .as_ref()
                .filter(|value| !looks_like_api_key(value))
                .cloned(),
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
        })
        .collect::<Vec<_>>();
    providers.sort_by(|a, b| a.name.cmp(&b.name));

    axum::extract::Json(ConfigResponse {
        ok: true,
        message: "loaded".into(),
        autonomy: format!("{:?}", cfg.defaults.autonomy),
        sandbox: cfg.defaults.sandbox,
        providers,
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

    let name = req.name.trim().to_lowercase();
    if name.is_empty() {
        return axum::extract::Json(RunResponse {
            ok: false,
            message: "provider name required".into(),
        });
    }

    let mut cfg = shared.write().expect("config lock poisoned");
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

async fn handle_ws(mut socket: axum::extract::ws::WebSocket, mut event_rx: tokio::sync::broadcast::Receiver<Event>) {
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
