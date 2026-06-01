use async_trait::async_trait;
use serde_json::json;
use std::sync::Mutex;

use super::{Tool, ToolCtx, ToolResult};
use crate::event::RiskLevel;

pub struct Todo {
    db: Mutex<Option<rusqlite::Connection>>,
}

impl Todo {
    pub fn new() -> Self {
        Self {
            db: Mutex::new(None),
        }
    }

    fn get_conn(&self) -> anyhow::Result<std::sync::MutexGuard<'_, Option<rusqlite::Connection>>> {
        let mut guard = self
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("Todo DB lock poisoned"))?;
        if guard.is_none() {
            let state_dir = dirs::state_dir().unwrap_or_default().join("sparrow");
            std::fs::create_dir_all(&state_dir)?;
            let conn = rusqlite::Connection::open(state_dir.join("sparrow.db"))?;
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS todos (
                    id TEXT PRIMARY KEY,
                    content TEXT NOT NULL,
                    status TEXT NOT NULL DEFAULT 'pending',
                    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
                    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
                );",
            )?;
            *guard = Some(conn);
        }
        Ok(guard)
    }
}

#[async_trait]
impl Tool for Todo {
    fn name(&self) -> &str {
        "todo"
    }
    fn description(&self) -> &str {
        "Track tasks with persistent state across calls"
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["create", "update", "list", "complete", "delete", "clear_completed"] },
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

        let guard = self.get_conn()?;
        let conn = guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Todo DB not initialized"))?;

        match action {
            "create" => {
                let new_id = uuid::Uuid::new_v4().to_string();
                conn.execute(
                    "INSERT INTO todos (id, content, status) VALUES (?1, ?2, ?3)",
                    rusqlite::params![new_id, content, status],
                )?;
                Ok(ToolResult::text(format!(
                    "Created task {}: {} ({})",
                    new_id, content, status
                )))
            }
            "update" => {
                let rows = conn.execute(
                    "UPDATE todos SET content = ?1, status = ?2, updated_at = unixepoch() WHERE id = ?3",
                    rusqlite::params![content, status, id],
                )?;
                if rows == 0 {
                    Ok(ToolResult::error(format!("Task {} not found", id)))
                } else {
                    Ok(ToolResult::text(format!(
                        "Updated task {}: {} ({})",
                        id, content, status
                    )))
                }
            }
            "complete" => {
                let rows = conn.execute(
                    "UPDATE todos SET status = 'completed', updated_at = unixepoch() WHERE id = ?1",
                    rusqlite::params![id],
                )?;
                if rows == 0 {
                    Ok(ToolResult::error(format!("Task {} not found", id)))
                } else {
                    Ok(ToolResult::text(format!("Completed task: {}", id)))
                }
            }
            "delete" => {
                conn.execute("DELETE FROM todos WHERE id = ?1", rusqlite::params![id])?;
                Ok(ToolResult::text(format!("Deleted task: {}", id)))
            }
            "clear_completed" => {
                let count = conn.execute("DELETE FROM todos WHERE status = 'completed'", [])?;
                Ok(ToolResult::text(format!(
                    "Cleared {} completed tasks",
                    count
                )))
            }
            _ => {
                // list — optionally filtered by status
                let mut stmt = if !status.is_empty() && status != "pending" {
                    conn.prepare("SELECT id, content, status FROM todos WHERE status = ?1 ORDER BY created_at")?
                } else {
                    conn.prepare("SELECT id, content, status FROM todos ORDER BY created_at")?
                };
                let rows: Vec<String> = if !status.is_empty() && status != "pending" {
                    stmt.query_map(rusqlite::params![status], |row| {
                        Ok(format!(
                            "  {} [{}] {}",
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(1)?
                        ))
                    })?
                    .filter_map(|r| r.ok())
                    .collect()
                } else {
                    stmt.query_map([], |row| {
                        Ok(format!(
                            "  {} [{}] {}",
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(1)?
                        ))
                    })?
                    .filter_map(|r| r.ok())
                    .collect()
                };
                if rows.is_empty() {
                    Ok(ToolResult::text("No tasks."))
                } else {
                    Ok(ToolResult::text(rows.join("\n")))
                }
            }
        }
    }
}

impl Default for Todo {
    fn default() -> Self {
        Self::new()
    }
}
