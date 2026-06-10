use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::engine::{Engine, Task};
use crate::event::Event;
use crate::memory::Memory;

pub mod event_bus;
pub mod ratelimit;
pub mod recorder;
pub mod scheduler;
pub mod session;

use event_bus::EventBus;
use recorder::{FsRecorder, Recorder, RunInputs};
use scheduler::{MemoryScheduler, Scheduler};

// ─── Run request ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RunRequest {
    pub task: String,
    pub agent: Option<String>,
    pub autonomy: Option<crate::event::AutonomyLevel>,
    pub budget: Option<f64>,
}

// ─── THE RUNTIME TRAIT ──────────────────────────────────────────────────────────

#[async_trait::async_trait]
pub trait Runtime: Send + Sync {
    async fn submit(&self, req: RunRequest) -> anyhow::Result<String>;
    fn subscribe_all(&self) -> tokio::sync::broadcast::Receiver<Event>;
    async fn interrupt(&self, _run_id: &str, _msg: &str) -> anyhow::Result<()>;
    async fn start(&self) -> anyhow::Result<()>;
    async fn stop(&self) -> anyhow::Result<()>;
    fn is_running(&self) -> bool;
}

// ─── Headless daemon implementation ─────────────────────────────────────────────

pub struct SparrowRuntime {
    engine: Arc<Engine>,
    scheduler: Arc<MemoryScheduler>,
    recorder: Arc<FsRecorder>,
    event_bus: EventBus,
    _memory: Arc<dyn Memory>,
    config: Config,
    running: std::sync::atomic::AtomicBool,
    // Running tasks
    active_runs: tokio::sync::Mutex<std::collections::HashMap<String, tokio::task::JoinHandle<()>>>,
    cancellations: Arc<tokio::sync::Mutex<std::collections::HashMap<String, CancellationToken>>>,
    /// Per-run injection senders. Populated on submit(), consumed by redirect().
    injects:
        Arc<tokio::sync::Mutex<std::collections::HashMap<String, mpsc::UnboundedSender<String>>>>,
}

impl SparrowRuntime {
    pub fn new(
        engine: Arc<Engine>,
        scheduler: Arc<MemoryScheduler>,
        recorder: Arc<FsRecorder>,
        event_bus: EventBus,
        memory: Arc<dyn Memory>,
        config: Config,
    ) -> Self {
        Self {
            engine,
            scheduler,
            recorder,
            event_bus,
            _memory: memory,
            config,
            running: std::sync::atomic::AtomicBool::new(false),
            active_runs: tokio::sync::Mutex::new(std::collections::HashMap::new()),
            cancellations: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            injects: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Inject a user message into a running run mid-flight (§3.7).
    /// Returns an error if no run with that id is active.
    pub async fn redirect(&self, run_id: &str, msg: String) -> anyhow::Result<()> {
        let injects = self.injects.lock().await;
        match injects.get(run_id) {
            Some(tx) => {
                tx.send(msg)
                    .map_err(|e| anyhow::anyhow!("inject channel closed: {}", e))?;
                Ok(())
            }
            None => anyhow::bail!("No active run with id {}", run_id),
        }
    }

    /// Spawn the cron tick loop
    async fn cron_loop(&self) {
        let scheduler = self.scheduler.clone();
        let engine = self.engine.clone();
        let event_bus = self.event_bus.clone();
        let recorder = self.recorder.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;

                let due_jobs = scheduler.tick().await;
                for job in due_jobs {
                    tracing::info!("Running scheduled job: {} ({})", job.id, job.task);

                    let (tx, mut rx) = mpsc::unbounded_channel::<Event>();
                    let task = Task {
                        description: job.task.clone(),
                        context: vec![],
                    };

                    let run_id = uuid::Uuid::new_v4().to_string();
                    recorder.start_run(
                        run_id.clone(),
                        RunInputs {
                            task: job.task.clone(),
                            config_snapshot: serde_json::json!({}),
                            model_id: "scheduled".into(),
                            repo_head: None,
                            timestamp: chrono::Utc::now().to_rfc3339(),
                            agent: "scheduler".into(),
                        },
                    );

                    let event_bus_clone = event_bus.clone();
                    let recorder_clone = recorder.clone();
                    let run_id_clone = run_id.clone();
                    let engine_clone = engine.clone();

                    tokio::spawn(async move {
                        let engine_run_id = run_id_clone.clone();
                        let engine_handle = tokio::spawn(async move {
                            engine_clone
                                .drive_with_run_id(task, tx, crate::event::RunId(engine_run_id))
                                .await
                        });

                        while let Some(event) = rx.recv().await {
                            recorder_clone.record(&event);
                            event_bus_clone.publish(event);
                        }

                        if let Err(err) = engine_handle.await {
                            tracing::error!("scheduled engine task failed: {}", err);
                        }
                        let _ = recorder_clone.finalize(&run_id_clone);
                    });
                }
            }
        });
    }

    /// Start a TCP socket for local API access
    async fn serve_api(&self, addr: &str) -> anyhow::Result<()> {
        let listener = TcpListener::bind(addr).await?;
        tracing::info!("Runtime API listening on {}", addr);

        let event_bus = self.event_bus.clone();

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((mut stream, addr)) => {
                        tracing::debug!("API connection from {}", addr);
                        let mut rx = event_bus.subscribe_all();

                        tokio::spawn(async move {
                            use tokio::io::AsyncWriteExt;
                            loop {
                                match rx.recv().await {
                                    Ok(event) => {
                                        if !event.is_public() {
                                            continue;
                                        }
                                        if let Ok(json) = serde_json::to_string(&event) {
                                            let line = json + "\n";
                                            if stream.write_all(line.as_bytes()).await.is_err() {
                                                break;
                                            }
                                        }
                                    }
                                    Err(_) => break,
                                }
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("Accept error: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// Start a Unix domain socket for local API access (Linux/macOS).
    /// Clients can `socat - UNIX-CONNECT:/path/to/socket` to receive NDJSON events.
    #[cfg(unix)]
    async fn serve_unix_socket(&self, path: &str) -> anyhow::Result<()> {
        use tokio::net::UnixListener;

        // Remove stale socket file from a previous run
        let _ = std::fs::remove_file(path);

        let listener = UnixListener::bind(path)?;
        tracing::info!("Runtime Unix socket at {}", path);

        // Restrict to owner only (rwx------) for security
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }

        let event_bus = self.event_bus.clone();

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((mut stream, _)) => {
                        tracing::debug!("Unix socket connection");
                        let mut rx = event_bus.subscribe_all();
                        tokio::spawn(async move {
                            use tokio::io::AsyncWriteExt;
                            loop {
                                match rx.recv().await {
                                    Ok(event) => {
                                        if !event.is_public() {
                                            continue;
                                        }
                                        if let Ok(json) = serde_json::to_string(&event) {
                                            let line = json + "\n";
                                            if stream.write_all(line.as_bytes()).await.is_err() {
                                                break;
                                            }
                                        }
                                    }
                                    Err(_) => break,
                                }
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("Unix socket accept error: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// No-op stub on Windows / non-unix targets.
    #[cfg(not(unix))]
    async fn serve_unix_socket(&self, _path: &str) -> anyhow::Result<()> {
        tracing::debug!("Unix socket not available on this platform; skipping.");
        Ok(())
    }

    /// Public accessor so surfaces can publish events (e.g. update
    /// notifications). Lives on the concrete type, not on the `Runtime`
    /// trait — adding it to the trait would force every Runtime impl to
    /// expose its internal EventBus.
    pub fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }
}

#[async_trait::async_trait]
impl Runtime for SparrowRuntime {
    async fn submit(&self, req: RunRequest) -> anyhow::Result<String> {
        let run_id = uuid::Uuid::new_v4().to_string();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let cancel_token = CancellationToken::new();

        let task = Task {
            description: req.task.clone(),
            context: vec![],
        };

        self.recorder.start_run(
            run_id.clone(),
            RunInputs {
                task: req.task,
                config_snapshot: serde_json::json!({}),
                model_id: "runtime".into(),
                repo_head: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
                agent: req.agent.unwrap_or_else(|| "sparrow".into()),
            },
        );

        let engine = self.engine.clone();
        let event_bus = self.event_bus.clone();
        let recorder = self.recorder.clone();
        let rid = run_id.clone();
        let token = cancel_token.clone();
        let cancellations = self.cancellations.clone();

        // Inject channel: redirect() pushes user messages into the running engine
        let (inject_tx, inject_rx) = mpsc::unbounded_channel::<String>();
        self.injects.lock().await.insert(run_id.clone(), inject_tx);
        let injects_map = self.injects.clone();

        // RAII cleanup so that a panic anywhere inside the spawned task still
        // releases the run_id from `cancellations` / `injects` — preventing the
        // map from growing unboundedly with zombie keys.
        struct RunCleanup {
            rid: String,
            cancellations:
                Arc<tokio::sync::Mutex<std::collections::HashMap<String, CancellationToken>>>,
            injects: Arc<
                tokio::sync::Mutex<
                    std::collections::HashMap<String, mpsc::UnboundedSender<String>>,
                >,
            >,
            recorder: Arc<FsRecorder>,
        }
        impl Drop for RunCleanup {
            fn drop(&mut self) {
                // We can't .await in drop; use blocking_lock when not on a Tokio
                // worker thread, or spawn a task that owns the cleanup. The
                // cheapest correct option is to spawn a detached task — runtime
                // is guaranteed alive because submit() runs inside it.
                let rid = std::mem::take(&mut self.rid);
                let cancellations = self.cancellations.clone();
                let injects = self.injects.clone();
                let recorder = self.recorder.clone();
                tokio::spawn(async move {
                    cancellations.lock().await.remove(&rid);
                    injects.lock().await.remove(&rid);
                    let _ = recorder.finalize(&rid);
                });
            }
        }

        let handle = tokio::spawn(async move {
            let _guard = RunCleanup {
                rid: rid.clone(),
                cancellations,
                injects: injects_map,
                recorder: recorder.clone(),
            };
            let engine_rid = rid.clone();
            let cancel_rid = rid.clone();
            let cancel_tx = tx.clone();
            let engine_handle = tokio::spawn(async move {
                tokio::select! {
                    result = engine.drive_with_inject(
                        task,
                        tx,
                        crate::event::RunId(engine_rid),
                        Some(inject_rx),
                    ) => result,
                    _ = token.cancelled() => {
                        let _ = cancel_tx.send(Event::Error {
                            run: crate::event::RunId(cancel_rid),
                            message: "interrupted".into(),
                        });
                        Ok(crate::event::OutcomeSummary {
                            status: "interrupted".into(),
                            cost_usd: 0.0,
                            tokens: crate::event::TokenUsage {
                                input: 0,
                                output: 0,
                            },
                            diffs: vec![],
                            cost_comparison: String::new(),
                        })
                    }
                }
            });

            while let Some(event) = rx.recv().await {
                recorder.record(&event);
                event_bus.publish(event);
            }

            if let Err(err) = engine_handle.await {
                tracing::error!("runtime engine task failed: {}", err);
            }
            // `_guard` drops here on the happy path and on early returns / panics.
        });

        self.cancellations
            .lock()
            .await
            .insert(run_id.clone(), cancel_token);
        self.active_runs.lock().await.insert(run_id.clone(), handle);
        Ok(run_id)
    }

    fn subscribe_all(&self) -> tokio::sync::broadcast::Receiver<Event> {
        self.event_bus.subscribe_all()
    }

    async fn interrupt(&self, run_id: &str, msg: &str) -> anyhow::Result<()> {
        tracing::info!("Interrupt requested for run {}: {}", run_id, msg);
        if let Some(token) = self.cancellations.lock().await.get(run_id).cloned() {
            token.cancel();
            Ok(())
        } else {
            anyhow::bail!("No active run found for interrupt: {}", run_id)
        }
    }

    async fn start(&self) -> anyhow::Result<()> {
        if self.running.load(std::sync::atomic::Ordering::SeqCst) {
            anyhow::bail!("Runtime is already running");
        }
        self.running
            .store(true, std::sync::atomic::Ordering::SeqCst);

        let api_addr = "127.0.0.1:9337";
        self.cron_loop().await;
        self.serve_api(api_addr).await?;

        // Unix socket: resolves to ~/.local/state/sparrow/sparrow.sock (or XDG equivalent)
        let socket_path = self
            .config
            .state_dir
            .join("sparrow.sock")
            .to_string_lossy()
            .to_string();
        if let Err(e) = self.serve_unix_socket(&socket_path).await {
            tracing::warn!("Unix socket failed (non-fatal): {}", e);
        } else {
            tracing::info!("Runtime Unix socket at {}", socket_path);
        }

        tracing::info!("Runtime started. TCP API at {}", api_addr);
        tracing::info!("Scheduled jobs active.");

        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        self.running
            .store(false, std::sync::atomic::Ordering::SeqCst);
        tracing::info!("Runtime stopped.");
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::SeqCst)
    }
}
