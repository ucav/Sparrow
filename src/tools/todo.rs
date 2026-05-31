use async_trait::async_trait;
use serde_json::json;

use super::{Tool, ToolCtx, ToolResult};
use crate::event::RiskLevel;

pub struct Todo;

#[async_trait]
impl Tool for Todo {
    fn name(&self) -> &str {
        "todo"
    }
    fn description(&self) -> &str {
        "Track tasks and sub-tasks for the current run"
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "update", "list", "complete"]
                },
                "id": { "type": "string" },
                "content": { "type": "string" },
                "status": { "type": "string", "enum": ["pending", "in_progress", "completed", "cancelled"] }
            },
            "required": ["action"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::ReadOnly
    }
    async fn call(&self, args: serde_json::Value, _ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let action = args["action"].as_str().unwrap_or("list");
        let content = args["content"].as_str().unwrap_or("");
        let status = args["status"].as_str().unwrap_or("pending");
        let id = args["id"].as_str().unwrap_or("");

        let msg = match action {
            "create" => format!("Created task: {} ({})", content, status),
            "update" => format!("Updated task {}: {} ({})", id, content, status),
            "complete" => format!("Completed task: {}", id),
            _ => "Todo list (memory-backed in M1)".to_string(),
        };

        Ok(ToolResult::text(msg))
    }
}
