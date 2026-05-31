use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::mpsc;

use super::{Tool, ToolCtx, ToolResult};
use crate::event::{Block, Event, RiskLevel};
use crate::engine::{Engine, Task};

// ─── Subagent spawn ─────────────────────────────────────────────────────────────

/// Delegates a subtask to a child AgentRun with its own conversation and sandbox.
/// §15: "Each subagent gets its own conversation, terminal, and a Python RPC channel."
pub struct SubagentSpawn {
    engine: Arc<Engine>,
}

impl SubagentSpawn {
    pub fn new(engine: Arc<Engine>) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl Tool for SubagentSpawn {
    fn name(&self) -> &str {
        "subagent_spawn"
    }
    fn description(&self) -> &str {
        "Spawn a child agent to handle a subtask independently"
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "task": { "type": "string", "description": "Subtask description" },
                "role": { "type": "string", "description": "Role for the subagent (e.g. tester, researcher, reviewer)" },
                "model": { "type": "string", "description": "Optional: model to use for the subagent" }
            },
            "required": ["task"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Exec
    }
    async fn call(
        &self,
        args: serde_json::Value,
        _ctx: &ToolCtx,
    ) -> anyhow::Result<ToolResult> {
        let task_desc = args["task"].as_str().unwrap_or("");
        let role = args["role"].as_str().unwrap_or("helper");

        let (tx, mut rx) = mpsc::unbounded_channel();

        let task = Task {
            description: task_desc.to_string(),
            context: vec![],
        };

        let engine = self.engine.clone();

        let handle = tokio::spawn(async move {
            match engine.drive(task, tx).await {
                Ok(outcome) => outcome,
                Err(e) => crate::event::OutcomeSummary {
                    status: format!("error: {}", e),
                    diffs: vec![],
                    cost_usd: 0.0,
                    tokens: crate::event::TokenUsage { input: 0, output: 0 },
                },
            }
        });

        // Collect subagent output
        let mut output = String::new();
        while let Some(event) = rx.recv().await {
            match &event {
                Event::ThinkingDelta { text, .. } => {
                    output.push_str(text);
                }
                Event::AgentStatus { note, .. } => {
                    output.push_str(&format!("\n[{}]", note));
                }
                Event::RunFinished { outcome, .. } => {
                    output.push_str(&format!(
                        "\n[Subagent done: {} | ${:.4}]",
                        outcome.status, outcome.cost_usd
                    ));
                }
                Event::Error { message, .. } => {
                    output.push_str(&format!("\n[Error: {}]", message));
                }
                _ => {}
            }
        }

        let outcome = handle.await.unwrap_or_else(|e| crate::event::OutcomeSummary {
            status: format!("subagent panicked: {}", e),
            diffs: vec![],
            cost_usd: 0.0,
            tokens: crate::event::TokenUsage { input: 0, output: 0 },
        });

        Ok(ToolResult::ok(vec![
            Block::Text(format!(
                "Subagent '{}' completed.\nStatus: {}\nOutput:\n{}",
                role, outcome.status, output
            )),
        ]))
    }
}

// ─── Python RPC channel (stub) ──────────────────────────────────────────────────

pub struct PythonRpc;

#[async_trait]
impl Tool for PythonRpc {
    fn name(&self) -> &str {
        "python_rpc"
    }
    fn description(&self) -> &str {
        "Execute Python code via an RPC channel to a persistent Python kernel"
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "code": { "type": "string", "description": "Python code to execute" },
                "timeout_ms": { "type": "integer", "description": "Timeout in milliseconds" }
            },
            "required": ["code"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Exec
    }
    async fn call(
        &self,
        args: serde_json::Value,
        _ctx: &ToolCtx,
    ) -> anyhow::Result<ToolResult> {
        let code = args["code"].as_str().unwrap_or("");
        let timeout_ms = args["timeout_ms"].as_u64().unwrap_or(30_000);

        // Execute via python3 subprocess (M6 stub — full RPC in v2)
        use std::process::Command as StdCommand;
        use std::time::Instant;

        let mut child = StdCommand::new("python3")
            .arg("-c")
            .arg(code)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let start = Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);

        loop {
            match child.try_wait()? {
                Some(_status) => {
                    let output = child.wait_with_output()?;
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    let result = if stderr.is_empty() { stdout } else { stderr };
                    return Ok(ToolResult::text(result));
                }
                None => {
                    if start.elapsed() > timeout {
                        let _ = child.kill();
                        return Ok(ToolResult::error("Python execution timed out"));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        }
    }
}
