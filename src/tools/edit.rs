use async_trait::async_trait;
use serde_json::json;
use std::fs;

use super::{resolve_workspace_path, Tool, ToolCtx, ToolResult};
use crate::event::{Block, RiskLevel};

pub struct Edit;

#[async_trait]
impl Tool for Edit {
    fn name(&self) -> &str {
        "edit"
    }
    fn description(&self) -> &str {
        "Edit a file by exact string replacement"
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Relative file path" },
                "old": { "type": "string", "description": "Exact text to replace" },
                "new": { "type": "string", "description": "Replacement text" },
                "replace_all": { "type": "boolean", "description": "Replace all occurrences (default false)" }
            },
            "required": ["path", "old", "new"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Mutating
    }
    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let path = args["path"].as_str().unwrap_or("");
        let old = args["old"].as_str().unwrap_or("");
        let new = args["new"].as_str().unwrap_or("");
        let replace_all = args["replace_all"].as_bool().unwrap_or(false);
        let full_path = resolve_workspace_path(&ctx.workspace_root, path)?;

        let content = fs::read_to_string(&full_path)?;

        if old.is_empty() {
            return Ok(ToolResult::error("old string cannot be empty"));
        }

        let count = content.matches(old).count();
        if count == 0 {
            return Ok(ToolResult::error(format!("Not found in {}: '{}'", path, old)));
        }
        if count > 1 && !replace_all {
            return Ok(ToolResult::error(format!(
                "Found {} matches in {}. Use replace_all: true or add more context to 'old'.",
                count, path
            )));
        }

        let new_content = if replace_all {
            content.replace(old, new)
        } else {
            content.replacen(old, new, 1)
        };

        let old_lines = old.lines().count() as u32;
        let new_lines = new.lines().count() as u32;

        fs::write(&full_path, &new_content)?;

        Ok(ToolResult::ok(vec![
            Block::Text(format!(
                "Edited {}: replaced {} occurrence(s)",
                path,
                if replace_all { count } else { 1 }
            )),
            Block::Diff {
                file: path.to_string(),
                patch: format!("@@ -1,{} +1,{} @@\n-{}\n+{}", old_lines, new_lines, old, new),
            },
        ]))
    }
}

pub struct MultiEdit;

#[async_trait]
impl Tool for MultiEdit {
    fn name(&self) -> &str {
        "multi_edit"
    }
    fn description(&self) -> &str {
        "Apply multiple edits to a file in one operation"
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "edits": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "old": { "type": "string" },
                            "new": { "type": "string" }
                        },
                        "required": ["old", "new"]
                    }
                }
            },
            "required": ["path", "edits"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Mutating
    }
    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let path = args["path"].as_str().unwrap_or("");
        let full_path = resolve_workspace_path(&ctx.workspace_root, path)?;
        let mut content = fs::read_to_string(&full_path)?;

        let edits = args["edits"].as_array().ok_or_else(|| {
            anyhow::anyhow!("edits must be an array")
        })?;

        let mut total_replacements = 0;
        for edit in edits {
            let old = edit["old"].as_str().unwrap_or("");
            let new = edit["new"].as_str().unwrap_or("");
            if old.is_empty() {
                continue;
            }
            let count = content.matches(old).count();
            if count == 1 {
                content = content.replace(old, new);
                total_replacements += 1;
            } else if count > 1 {
                return Ok(ToolResult::error(format!(
                    "Found {} matches for '{}'. Each edit must match exactly once.",
                    count,
                    if old.len() > 50 {
                        format!("{}...", &old[..50])
                    } else {
                        old.to_string()
                    }
                )));
            }
        }

        fs::write(&full_path, &content)?;
        Ok(ToolResult::text(format!(
            "Applied {} edits to {}",
            total_replacements, path
        )))
    }
}
