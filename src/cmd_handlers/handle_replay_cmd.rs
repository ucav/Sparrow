// src/cmd_handlers/handle_replay_cmd.rs
use super::prelude::*;
pub async fn handle_replay(
    run_id: &str,
    recorder: &Arc<FsRecorder>,
    config: &sparrow::config::Config,
    memory: Arc<dyn Memory>,
) -> anyhow::Result<()> {
    match recorder.load(run_id) {
        Some(transcript) => {
            println!("═══ REPLAY: {} ═══", run_id);
            println!("Task  : {}", transcript.inputs.task);
            println!("Agent : {}", transcript.inputs.agent);
            println!("Model : {}", transcript.inputs.model_id);
            println!("Events: {}", transcript.events.len());
            println!();

            for event in &transcript.events {
                match event {
                    sparrow::event::Event::ThinkingDelta { text, .. } => {
                        print!("{}", text);
                    }
                    sparrow::event::Event::ToolUseProposed { name, .. } => {
                        println!("\n[Tool: {}]", name);
                    }
                    sparrow::event::Event::RunFinished { outcome, .. } => {
                        println!(
                            "\n--- Done: {} | Cost: ${:.4} {}---",
                            outcome.status,
                            outcome.cost_usd,
                            sparrow::cost::format_comparison_oneliner(
                                outcome.cost_usd,
                                &outcome.tokens
                            )
                        );
                    }
                    sparrow::event::Event::Error { message, .. }
                        if !sparrow::event::is_local_model_unavailable(message) =>
                    {
                        eprintln!("\n[Error: {}]", message);
                    }
                    _ => {}
                }
            }

            println!("\n═══ Re-execute? (y/N) ═══");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if input.trim().to_lowercase() == "y" {
                use sparrow::engine::Engine;
                use sparrow::router::BasicRouter;
                let providers = build_provider_brains(config, &memory, true);
                let router = Arc::new(BasicRouter::new(config, providers));
                let engine = Arc::new(Engine::new(router, config.clone()).with_memory(memory));
                let re_executer = ReExecuter::new(engine);
                println!("Re-executing against current model...");
                match re_executer.re_execute(&transcript).await {
                    Ok(outcome) => println!(
                        "Re-execute done: {} | ${:.4}",
                        outcome.status, outcome.cost_usd
                    ),
                    Err(e) => eprintln!("Re-execute failed: {}", e),
                }
            }
            println!("\n═══ End of replay ═══");
        }
        None => {
            let transcripts = recorder.list_transcripts();
            if transcripts.is_empty() {
                println!("No transcripts found.");
            } else {
                println!("Transcript not found: {}", run_id);
                println!("\nAvailable:");
                for t in &transcripts {
                    if let Some(tr) = recorder.load(t) {
                        println!(
                            "  {} | {} events | {}",
                            t,
                            tr.events.len(),
                            tr.inputs.task.chars().take(60).collect::<String>()
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

// ─── Chat command ───────────────────────────────────────────────────────────────
