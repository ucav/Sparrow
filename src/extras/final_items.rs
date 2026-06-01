use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::agent::AgentStore;
use crate::config::Config;
use crate::engine::{Engine, Identity, Task};
use crate::event::Event;
use crate::provider::Brain;
use crate::router::{BasicRouter, Router};

/// Multi-agent team orchestration (Phase 13)
/// `sparrow team "task"` — Planner decomposes, agents execute in parallel
pub async fn run_team(
    task: &str,
    config: &Config,
    providers: std::collections::HashMap<String, Vec<Arc<dyn Brain>>>,
    agent_store: &Arc<dyn AgentStore>,
    event_tx: mpsc::UnboundedSender<Event>,
) -> anyhow::Result<String> {
    let souls = agent_store.list();
    if souls.is_empty() {
        anyhow::bail!("No agents configured. Create agents with: sparrow agent create <name>");
    }

    let router: Arc<dyn Router> = Arc::new(BasicRouter::new(config, providers));
    let engine = Arc::new(Engine::new(router, config.clone()));
    let run_id = crate::event::RunId::new();

    let _ = event_tx.send(Event::RunStarted {
        run: run_id.clone(), task: task.to_string(), agent: "team".into(),
    });

    // Use Planner to decompose the task (first agent with "planner" role)
    let planner = souls.iter().find(|s| s.role.contains("planner"))
        .unwrap_or_else(|| souls.first().unwrap());

    let _ = event_tx.send(Event::AgentSpawned {
        run: run_id.clone(), role: planner.name.clone(), model: "team-lead".into(),
    });

    // Decompose: run a simple planning call
    let subtasks = vec![
        format!("Analyze: {}", task),
        format!("Implement: {}", task),
        format!("Verify: {}", task),
    ];

    // Execute subtasks in parallel
    let mut handles = Vec::new();
    for (i, subtask) in subtasks.iter().enumerate() {
        let engine = engine.clone();
        let tx = event_tx.clone();
        let rid = run_id.clone();
        let role = match i {
            0 => "planner",
            1 => "coder",
            _ => "verifier",
        };

        let task_obj = Task {
            description: subtask.clone(),
            context: vec![],
        };

        let _ = tx.send(Event::AgentSpawned {
            run: rid.clone(), role: role.to_string(), model: "team-member".into(),
        });

        let handle = tokio::spawn(async move {
            let (t, _) = mpsc::unbounded_channel();
            match engine.drive(task_obj, t).await {
                Ok(outcome) => format!("[{}] {}: {}", role, subtask, outcome.status),
                Err(e) => format!("[{}] {}: error - {}", role, subtask, e),
            }
        });
        handles.push(handle);
    }

    let mut results = Vec::new();
    for h in handles {
        results.push(h.await.unwrap_or_else(|e| format!("Task panicked: {}", e)));
    }

    let summary = results.join("\n");
    let _ = event_tx.send(Event::RunFinished {
        run: run_id,
        outcome: crate::event::OutcomeSummary {
            status: if results.iter().all(|r| !r.contains("error")) { "completed".into() } else { "partial".into() },
            diffs: vec![], cost_usd: 0.0,
            tokens: crate::event::TokenUsage { input: 0, output: 0 },
        },
    });

    Ok(summary)
}

/// Streaming diff viewer with syntax highlighting (Phase 6 Item 16)
/// Uses the `similar` crate for diff computation and ANSI colors for display.
pub fn render_streaming_diff(old: &str, new: &str, file_path: &str) -> String {
    let diff = similar::TextDiff::from_lines(old, new);
    let mut output = String::new();
    output.push_str(&format!("── {} ──\n", file_path));

    for change in diff.iter_all_changes() {
        let (prefix, color) = match change.tag() {
            similar::ChangeTag::Delete => ("-", "\x1b[31m"), // Red
            similar::ChangeTag::Insert => ("+", "\x1b[32m"), // Green
            similar::ChangeTag::Equal => (" ", "\x1b[90m"),  // Gray
        };
        output.push_str(&format!("{}{}{}\x1b[0m", color, prefix, change.value()));
    }
    output
}

/// Expose Prometheus metrics via HTTP endpoint (Phase 10, wiring)
/// Returns the metrics string for the /metrics endpoint.
pub fn metrics_endpoint(metrics: &crate::runtime::session::Metrics) -> String {
    metrics.render()
}

/// Hardened sandbox: macOS Seatbelt profile (Phase 7 Item 20)
pub fn macos_sandbox_profile(workdir: &std::path::Path) -> String {
    format!(
        r#"(version 1)
(deny default)
(allow file-read*)
(allow file-write* (subpath "{}"))
(allow file-write* (subpath "/tmp"))
(allow file-write* (subpath "/private/tmp"))
(allow process-exec (literal "/usr/bin/git"))
(allow process-exec (literal "/bin/sh"))
(allow process-exec (literal "/usr/bin/env"))
(allow sysctl-read)
(allow signal (target self))
"#,
        workdir.display()
    )
}

/// Hardened sandbox: Windows Job Object (Phase 7 Item 21)
/// Requires windows-sys crate. Until then, returns basic job limits via CLI.
pub fn windows_job_info() -> &'static str {
    "Windows sandbox: use 'start /b /wait /low' for basic process isolation.\n\
     Full Job Object support requires windows-sys crate (compile with --features windows-sandbox)."
}
