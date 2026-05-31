use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;
use std::process::Command as StdCommand;

use super::{Tool, ToolCtx, ToolResult};
use crate::event::RiskLevel;

pub struct Git;

#[async_trait]
impl Tool for Git {
    fn name(&self) -> &str {
        "git"
    }
    fn description(&self) -> &str {
        "Git operations: status, diff, log, branch, checkout"
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["status", "diff", "log", "branch", "checkout", "add", "commit"]
                },
                "args": { "type": "array", "items": { "type": "string" } }
            },
            "required": ["action"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::ReadOnly
    }
    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let action = args["action"].as_str().unwrap_or("status");
        let extra_args: Vec<String> = args["args"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        // Update risk based on action
        let mut git_args = vec![action.to_string()];
        git_args.extend(extra_args);

        // Special handling for commit
        let mut message_file: Option<PathBuf> = None;
        if action == "commit" {
            let msg = args["message"].as_str().unwrap_or("auto-commit");
            let msg_file = ctx.workspace_root.join(".git").join("SPARROW_COMMIT_MSG");
            std::fs::write(&msg_file, msg)?;
            git_args.push("--file".into());
            git_args.push(msg_file.to_string_lossy().to_string());
            message_file = Some(msg_file);
        }

        let output = StdCommand::new("git")
            .args(&git_args)
            .current_dir(&ctx.workspace_root)
            .output()?;

        // Cleanup temp file
        if let Some(f) = message_file {
            let _ = std::fs::remove_file(f);
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let result = if stdout.is_empty() { stderr } else { stdout };

        Ok(ToolResult::text(result))
    }
}
