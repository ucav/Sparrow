// src/cmd_handlers/handle_daemon_cmd.rs — extracted from main.rs

async fn handle_daemon(
    config: &sparrow::config::Config,
    memory: Arc<dyn Memory>,
    scheduler: Arc<MemoryScheduler>,
    recorder: Arc<FsRecorder>,
    skills: Arc<dyn SkillLibrary>,
) -> anyhow::Result<()> {
    use sparrow::engine::Engine;
    use sparrow::router::BasicRouter;

    let providers = build_provider_brains(config, &memory, true);
    let router = Arc::new(BasicRouter::new(config, providers));
    let engine = Arc::new(
        Engine::new(router, config.clone())
            .with_memory(memory.clone())
            .with_skills(skills),
    );
    let event_bus = EventBus::new(256);
    let runtime = SparrowRuntime::new(
        engine,
        scheduler,
        recorder,
        event_bus,
        memory,
        config.clone(),
    );
    runtime.start().await?;
    println!("Sparrow daemon running. API on 127.0.0.1:9337. Ctrl+C to stop.");

    // Background update check — non-blocking, emits UpdateAvailable event
    // to all connected surfaces (WebView, TUI, gateway).
    {
        let bus = runtime.event_bus().clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            if let Some(info) = sparrow::update::check_update() {
                let _ = bus.publish(sparrow::event::Event::UpdateAvailable {
                    current: info.current,
                    latest: info.latest,
                    download_url: info.download_url,
                    crate_url: info.crate_url,
                    release_url: info.release_url,
                    install_cmd: info.install_cmd,
                });
            }
        });
    }

    tokio::signal::ctrl_c().await?;
    runtime.stop().await?;
    Ok(())
}

// ─── OAuth device-flow login — registry-driven ───────────────────────────────
//
// Any provider in the registry with `auth_flow: DeviceOAuth { .. }` is
// automatically supported.  The `client_id` can be passed on the CLI or
// read from the env var declared in the registry entry (`client_id_env`).

/// How `run_task` resolves the conversation context for this run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
