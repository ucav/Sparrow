// src/cmd_handlers/handle_status_cmd.rs
use super::prelude::*;
pub fn handle_status(
    memory: &Arc<dyn Memory>,
    config: &sparrow::config::Config,
    scheduler: &Arc<sparrow::runtime::scheduler::MemoryScheduler>,
    recorder: &Arc<sparrow::runtime::recorder::FsRecorder>,
    state_dir: &std::path::PathBuf,
) -> anyhow::Result<()> {
    println!("Sparrow Status");
    println!("──────────────");

    // Budget & autonomy
    println!(
        "Budget     : ${:.2}/session  ${:.2}/day",
        config.budget.session_usd, config.budget.daily_usd
    );
    println!("Autonomy   : {:?}", config.defaults.autonomy);
    println!("Sandbox    : {}", config.defaults.sandbox);

    // Gateway up/down
    let gw_pid_path = state_dir.join("gateway.pid");
    let gw_ws_open = std::net::TcpStream::connect_timeout(
        &"127.0.0.1:9338".parse().unwrap(),
        std::time::Duration::from_millis(150),
    )
    .is_ok();
    let gw_pid_alive = gw_pid_path
        .exists()
        .then(|| {
            std::fs::read_to_string(&gw_pid_path)
                .ok()
                .and_then(|s| s.trim().parse::<u32>().ok())
        })
        .flatten()
        .map(|pid| {
            #[cfg(windows)]
            {
                std::process::Command::new("tasklist")
                    .args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV", "/NH"])
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
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
        })
        .unwrap_or(false);
    println!(
        "Gateway    : {}",
        if gw_ws_open || gw_pid_alive {
            "running"
        } else {
            "stopped  (start with: sparrow gateway start)"
        }
    );

    // Scheduled jobs
    let jobs = scheduler.list();
    if jobs.is_empty() {
        println!("Cron jobs  : none scheduled");
    } else {
        println!("Cron jobs  : {} scheduled", jobs.len());
        for j in &jobs {
            let st = if j.enabled { "active" } else { "paused" };
            let next = j.next_run.as_deref().unwrap_or("pending");
            println!("  [{}] {}  cron:{}  next:{}", st, j.id, j.cron, next);
        }
    }

    // Recent transcripts
    let transcripts = recorder.list_transcripts();
    println!("Transcripts: {} total", transcripts.len());
    for id in transcripts.iter().rev().take(3) {
        if let Some(tr) = recorder.load(id) {
            println!(
                "  {} | {} events | {}",
                id,
                tr.events.len(),
                tr.inputs.task.chars().take(50).collect::<String>()
            );
        }
    }

    // Memory & model cache
    let mem_stats = memory.memory_stats();
    println!(
        "Memory     : {} facts | MEMORY.md {}/{} | USER.md {}/{} chars",
        mem_stats.facts,
        mem_stats.memory_chars,
        mem_stats.memory_limit,
        mem_stats.user_chars,
        mem_stats.user_limit
    );
    let total_discovered: usize = sparrow::config::providers::provider_registry()
        .iter()
        .map(|p| {
            memory
                .get_discovered_models(&p.id)
                .into_iter()
                .filter(|model| sparrow::provider::discovery::is_chat_model_id(model))
                .count()
        })
        .sum();
    let static_count: usize = sparrow::config::providers::provider_registry()
        .iter()
        .map(|p| p.models.len())
        .sum();
    println!(
        "Models     : {} static + {} discovered (cached 24h)",
        static_count, total_discovered
    );

    println!("\nRun 'sparrow doctor' for full diagnostics.");
    Ok(())
}
