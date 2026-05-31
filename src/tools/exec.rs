use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

use super::{Tool, ToolCtx, ToolResult};
use crate::event::RiskLevel;
use crate::sandbox::{Command, Limits, Sandbox};

pub struct Exec {
    sandbox: Arc<dyn Sandbox>,
}

impl Exec {
    pub fn new(sandbox: Arc<dyn Sandbox>) -> Self {
        Self { sandbox }
    }
}

#[async_trait]
impl Tool for Exec {
    fn name(&self) -> &str {
        "exec"
    }
    fn description(&self) -> &str {
        "Execute a shell command in the sandboxed workspace"
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "Shell command to execute" },
                "timeout_ms": { "type": "integer", "description": "Timeout in milliseconds (default 120000)" }
            },
            "required": ["command"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Exec
    }
    async fn call(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
    ) -> anyhow::Result<ToolResult> {
        let cmd_str = args["command"].as_str().unwrap_or("");
        let timeout_ms = args["timeout_ms"].as_u64().unwrap_or(120_000);

        let cmd = Command {
            program: if cfg!(windows) { "cmd" } else { "sh" }.to_string(),
            args: vec![
                if cfg!(windows) { "/c" } else { "-c" }.to_string(),
                cmd_str.to_string(),
            ],
            env: std::collections::HashMap::new(),
            workdir: ctx.workspace_root.clone(),
        };

        let limits = Limits {
            timeout_ms,
            max_output_bytes: 1024 * 1024, // 1 MB
        };

        let result = self.sandbox.exec(&cmd, &limits).await?;

        let output = if result.stdout.is_empty() && result.stderr.is_empty() {
            format!("(exit: {})", result.exit_code)
        } else if result.stderr.is_empty() {
            result.stdout
        } else if result.stdout.is_empty() {
            format!("stderr:\n{}", result.stderr)
        } else {
            format!("stdout:\n{}\nstderr:\n{}", result.stdout, result.stderr)
        };

        Ok(ToolResult::text(output))
    }
}
