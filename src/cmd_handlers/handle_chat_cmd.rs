// src/cmd_handlers/handle_chat_cmd.rs — extracted from main.rs

async fn handle_chat(
    config: &sparrow::config::Config,
    memory: Arc<dyn Memory>,
) -> anyhow::Result<()> {
    use sparrow::engine::Engine;
    use sparrow::router::BasicRouter;

    let providers = build_provider_brains(config, &memory, true);

    let router = Arc::new(BasicRouter::new(config, providers));
    let engine = Arc::new(Engine::new(router, config.clone()).with_memory(memory));
    let mut session = ChatSession::new(engine);
    session.run_interactive().await
}

// ─── Gateway command ────────────────────────────────────────────────────────────

