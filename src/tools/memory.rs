use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

use super::{Tool, ToolCtx, ToolResult};
use crate::event::RiskLevel;
use crate::memory::{Fact, Memory, MemoryDocKind};

pub struct MemoryTool {
    memory: Arc<dyn Memory>,
}

impl MemoryTool {
    pub fn new(memory: Arc<dyn Memory>) -> Self {
        Self { memory }
    }
}

#[async_trait]
impl Tool for MemoryTool {
    fn name(&self) -> &str {
        "memory"
    }

    fn description(&self) -> &str {
        "Read and manage Sparrow persistent memory: facts, bounded MEMORY.md/USER.md docs, recall, and consolidation."
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "recall", "add", "replace", "remove", "consolidate", "docs", "stats"]
                },
                "id": { "type": "string", "description": "Fact id for remove/replace" },
                "key": { "type": "string", "description": "Fact key or doc kind (memory|user)" },
                "value": { "type": "string", "description": "Fact value or doc content" },
                "query": { "type": "string", "description": "Recall query" },
                "limit": { "type": "integer", "description": "Maximum rows to return" }
            },
            "required": ["action"]
        })
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::Mutating
    }

    async fn call(&self, args: serde_json::Value, _ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let action = args["action"].as_str().unwrap_or("list");
        let limit = args["limit"].as_u64().unwrap_or(10).min(50) as usize;
        match action {
            "list" => {
                let facts = self.memory.all_facts();
                if facts.is_empty() {
                    Ok(ToolResult::text("No stored facts."))
                } else {
                    Ok(ToolResult::text(
                        facts
                            .iter()
                            .take(limit)
                            .map(|fact| format!("{}  {}: {}", fact.id, fact.key, fact.value))
                            .collect::<Vec<_>>()
                            .join("\n"),
                    ))
                }
            }
            "recall" => {
                let query = args["query"].as_str().unwrap_or("");
                if query.trim().is_empty() {
                    return Ok(ToolResult::error("memory recall requires query"));
                }
                let facts = self.memory.recall(query, limit);
                if facts.is_empty() {
                    Ok(ToolResult::text("No matching facts."))
                } else {
                    Ok(ToolResult::text(
                        facts
                            .iter()
                            .map(|fact| format!("{}  {}: {}", fact.id, fact.key, fact.value))
                            .collect::<Vec<_>>()
                            .join("\n"),
                    ))
                }
            }
            "add" | "replace" => {
                let key = args["key"].as_str().unwrap_or("").trim();
                let value = args["value"].as_str().unwrap_or("").trim();
                if key.is_empty() || value.is_empty() {
                    return Ok(ToolResult::error(
                        "memory add/replace requires key and value",
                    ));
                }
                if let Some(kind) = MemoryDocKind::parse(key) {
                    self.memory.upsert_memory_doc(kind, value)?;
                    return Ok(ToolResult::text(format!("Updated {}", kind.as_str())));
                }
                let id = if action == "replace" {
                    args["id"]
                        .as_str()
                        .filter(|id| !id.trim().is_empty())
                        .map(|id| id.to_string())
                        .or_else(|| {
                            self.memory
                                .all_facts()
                                .into_iter()
                                .find(|f| f.key == key)
                                .map(|f| f.id)
                        })
                } else {
                    Some(uuid::Uuid::new_v4().to_string())
                };
                let id = id.unwrap_or_else(|| key.to_string());
                self.memory.remember(Fact {
                    id: id.clone(),
                    key: key.to_string(),
                    value: value.to_string(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                    updated_at: chrono::Utc::now().to_rfc3339(),
                })?;
                Ok(ToolResult::text(format!("Stored fact {}", id)))
            }
            "remove" => {
                let id = args["id"].as_str().unwrap_or("").trim();
                let key = args["key"].as_str().unwrap_or("").trim();
                if let Some(kind) = MemoryDocKind::parse(key) {
                    self.memory.remove_memory_doc(kind)?;
                    return Ok(ToolResult::text(format!("Removed {}", kind.as_str())));
                }
                if id.is_empty() {
                    return Ok(ToolResult::error("memory remove requires id"));
                }
                self.memory.forget(id)?;
                Ok(ToolResult::text(format!("Removed fact {}", id)))
            }
            "consolidate" => {
                self.memory.consolidate_memory()?;
                Ok(ToolResult::text("Memory consolidated into bounded docs."))
            }
            "docs" => {
                let mut out = Vec::new();
                for kind in [MemoryDocKind::Memory, MemoryDocKind::User] {
                    if let Some(doc) = self.memory.memory_doc(kind) {
                        out.push(format!("## {}\n{}", kind.as_str(), doc.content));
                    }
                }
                if out.is_empty() {
                    Ok(ToolResult::text("No MEMORY.md/USER.md docs stored."))
                } else {
                    Ok(ToolResult::text(out.join("\n\n")))
                }
            }
            "stats" => {
                let stats = self.memory.memory_stats();
                Ok(ToolResult::text(format!(
                    "facts={} MEMORY.md={}/{} chars USER.md={}/{} chars",
                    stats.facts,
                    stats.memory_chars,
                    stats.memory_limit,
                    stats.user_chars,
                    stats.user_limit
                )))
            }
            _ => Ok(ToolResult::error("unknown memory action")),
        }
    }
}
