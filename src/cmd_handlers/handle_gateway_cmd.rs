// src/cmd_handlers/handle_gateway_cmd.rs
use super::prelude::*;
pub async fn handle_gateway(
    action: sparrow::cli::GatewayAction,
    state_dir: &std::path::PathBuf,
    config: &sparrow::config::Config,
    memory: Arc<dyn Memory>,
    scheduler: Arc<MemoryScheduler>,
    recorder: Arc<FsRecorder>,
) -> anyhow::Result<()> {
    match action {
        sparrow::cli::GatewayAction::Start => {
            println!("Starting gateway daemon...");
            write_gateway_pid(state_dir)?;

            use sparrow::engine::Engine;
            use sparrow::router::BasicRouter;

            let providers = build_provider_brains(config, &memory, true);

            let router = Arc::new(BasicRouter::new(config, providers));
            let engine = Arc::new(Engine::new(router, config.clone()).with_memory(memory.clone()));
            let _cron_handle = scheduler.start_cron_loop(engine.clone(), recorder.clone());

            // Event bus for pub/sub
            let (event_bus_tx, _) = tokio::sync::broadcast::channel::<sparrow::event::Event>(256);

            // Session store for cross-surface continuity (§8)
            let session_store = std::sync::Arc::new(sparrow::runtime::session::SessionStore::open(
                &config.state_dir.join("sessions.db"),
            )?);

            // Message router
            let router_handler = Arc::new(
                MessageRouter::new(engine, recorder.clone(), event_bus_tx, vec![])
                    .with_sessions(session_store),
            );

            // Channel: transports → router
            let (msg_tx, mut msg_rx) = tokio::sync::mpsc::unbounded_channel::<GatewayMessage>();
            // Channel: router → transports
            let (resp_tx, mut resp_rx) = tokio::sync::mpsc::unbounded_channel::<GatewayResponse>();

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

            // Email (outbound only, behind `email` feature)
            if let Some(ref em) = config.surfaces.email {
                if em.enabled {
                    let user = std::env::var(&em.username_env).unwrap_or_default();
                    let pass = std::env::var(&em.password_env).unwrap_or_default();
                    if !user.is_empty() && !pass.is_empty() {
                        println!(
                            "  Email    : enabled (SMTP {}:{})",
                            em.smtp_host, em.smtp_port
                        );
                        let mut email_transport = sparrow::gateway::email::EmailTransport::new(
                            em.from.clone(),
                            em.smtp_host.clone(),
                            em.smtp_port,
                            user,
                            pass,
                            em.allowed_to.clone(),
                        );
                        if let Some(imap_host) = &em.imap_host {
                            email_transport =
                                email_transport.with_imap(imap_host.clone(), em.imap_port);
                            println!("             + IMAP inbound {}:{}", imap_host, em.imap_port);
                        }
                        transports.push(Box::new(email_transport));
                    } else {
                        println!(
                            "  Email    : no credentials (set {} + {})",
                            em.username_env, em.password_env
                        );
                    }
                }
            }

            // Always start WebSocket API
            println!("  WS API   : ws://127.0.0.1:9338");
            let ws_api = WebSocketApi::new("127.0.0.1:9338");
            transports.push(Box::new(ws_api));

            println!(
                "  Extra    : WhatsApp/Signal/Feishu/WeCom/QQ/Teams adapters present, not started without credentials"
            );

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

            // Abort poller: watch `<state>/gateway-abort/<run>.abort` files
            // written by `sparrow gateway abort <run>` and cancel the matching
            // in-process run via the registry.
            {
                let registry = router_handler.run_registry.clone();
                let abort_dir = state_dir.join("gateway-abort");
                std::fs::create_dir_all(&abort_dir).ok();
                tokio::spawn(async move {
                    loop {
                        if let Ok(entries) = std::fs::read_dir(&abort_dir) {
                            for entry in entries.flatten() {
                                let path = entry.path();
                                let run_id = path
                                    .file_stem()
                                    .map(|s| s.to_string_lossy().to_string())
                                    .unwrap_or_default();
                                if run_id.is_empty() {
                                    continue;
                                }
                                let aborted = registry.abort(&run_id);
                                let _ = std::fs::remove_file(&path);
                                if aborted {
                                    eprintln!("[gateway] aborted run {}", run_id);
                                }
                            }
                        }
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    }
                });
            }

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
                    Some(response) = resp_rx.recv() => {
                        let surface = response.surface.clone();
                        let mut delivered = false;
                        for transport in &transports {
                            if transport.name() == surface {
                                delivered = true;
                                if let Err(e) = transport.send(response.clone()).await {
                                    eprintln!("Failed to send {} response: {}", surface, e);
                                }
                                break;
                            }
                        }
                        if !delivered {
                            eprintln!("No gateway transport for surface: {}", surface);
                        }
                    }
                    _ = tokio::signal::ctrl_c() => {
                        println!("\nStopping gateway...");
                        for transport in &transports {
                            let _ = transport.stop().await;
                        }
                        let _ = remove_gateway_pid(state_dir);
                        println!("Gateway stopped.");
                        break;
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(60)) => {
                        // Keep-alive
                    }
                }
            }
            Ok(())
        }
        sparrow::cli::GatewayAction::Status => {
            let pid = read_gateway_pid(state_dir);
            let pid_running = pid.is_some_and(process_is_running);
            let ws_open = gateway_ws_port_open();
            if pid_running || ws_open {
                println!("Gateway status: running");
                if let Some(pid) = read_gateway_pid(state_dir) {
                    println!("PID: {}", pid);
                }
                println!(
                    "WS API: {}",
                    if ws_open {
                        "online on ws://127.0.0.1:9338"
                    } else {
                        "not reachable"
                    }
                );
            } else {
                println!("Gateway status: not running");
                println!("Start with: sparrow gateway start");
            }
            Ok(())
        }
        sparrow::cli::GatewayAction::Health => {
            let pid = read_gateway_pid(state_dir);
            let pid_running = pid.is_some_and(process_is_running);
            let ws_open = gateway_ws_port_open();
            println!("Gateway health");
            println!(
                "  pid_file : {}",
                if pid.is_some() { "present" } else { "absent" }
            );
            println!(
                "  process  : {}",
                if pid_running { "running" } else { "stopped" }
            );
            println!(
                "  ws       : {}",
                if ws_open { "online" } else { "offline" }
            );
            println!(
                "  sessions : {}",
                config.state_dir.join("sessions.db").display()
            );
            if pid.is_some() && !pid_running && !ws_open {
                println!("  warning  : stale gateway pid file");
            }
            Ok(())
        }
        sparrow::cli::GatewayAction::Abort { run } => {
            let abort_dir = state_dir.join("gateway-abort");
            std::fs::create_dir_all(&abort_dir)?;
            std::fs::write(
                abort_dir.join(format!("{}.abort", sanitize_file_component(&run))),
                chrono::Utc::now().to_rfc3339(),
            )?;
            println!("Gateway abort requested for run '{}'.", run);
            println!("Abort signal: {}", abort_dir.display());
            Ok(())
        }
        sparrow::cli::GatewayAction::Stop => {
            match read_gateway_pid(state_dir) {
                Some(pid) if process_is_running(pid) => {
                    stop_gateway_process(pid)?;
                    let _ = remove_gateway_pid(state_dir);
                    println!("Gateway stop requested for PID {}.", pid);
                }
                Some(pid) => {
                    let _ = remove_gateway_pid(state_dir);
                    println!("Gateway PID {} was stale; cleaned status file.", pid);
                }
                None => {
                    println!("Gateway status: not running");
                }
            }
            Ok(())
        }
    }
}

pub fn gateway_pid_path(state_dir: &std::path::Path) -> std::path::PathBuf {
    state_dir.join("gateway.pid")
}

pub fn write_gateway_pid(state_dir: &std::path::Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(state_dir)?;
    std::fs::write(gateway_pid_path(state_dir), std::process::id().to_string())?;
    Ok(())
}

pub fn read_gateway_pid(state_dir: &std::path::Path) -> Option<u32> {
    std::fs::read_to_string(gateway_pid_path(state_dir))
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
}

pub fn remove_gateway_pid(state_dir: &std::path::Path) -> std::io::Result<()> {
    let path = gateway_pid_path(state_dir);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

pub fn sanitize_file_component(value: &str) -> String {
    let cleaned = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    if cleaned.is_empty() {
        "run".into()
    } else {
        cleaned
    }
}

pub fn gateway_ws_port_open() -> bool {
    std::net::TcpStream::connect_timeout(
        &"127.0.0.1:9338".parse().expect("valid socket address"),
        std::time::Duration::from_millis(250),
    )
    .is_ok()
}

pub fn process_is_running(pid: u32) -> bool {
    #[cfg(windows)]
    {
        std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV", "/NH"])
            .output()
            .map(|out| {
                let stdout = String::from_utf8_lossy(&out.stdout);
                out.status.success() && stdout.contains(&pid.to_string())
            })
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

pub fn stop_gateway_process(pid: u32) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        let status = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status()?;
        if !status.success() {
            anyhow::bail!("taskkill failed for PID {}", pid);
        }
    }
    #[cfg(not(windows))]
    {
        let status = std::process::Command::new("kill")
            .arg(pid.to_string())
            .status()?;
        if !status.success() {
            anyhow::bail!("kill failed for PID {}", pid);
        }
    }
    Ok(())
}
