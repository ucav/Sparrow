use async_trait::async_trait;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

use super::{resolve_workspace_path, Tool, ToolCtx, ToolResult};
use crate::event::RiskLevel;

pub struct FsRead;

#[async_trait]
impl Tool for FsRead {
    fn name(&self) -> &str {
        "fs_read"
    }
    fn description(&self) -> &str {
        "Read a file from the workspace"
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Relative path to the file" },
                "offset": { "type": "integer", "description": "Line number to start from (1-indexed)" },
                "limit": { "type": "integer", "description": "Max lines to read" }
            },
            "required": ["path"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::ReadOnly
    }
    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let path = args["path"].as_str().unwrap_or("");
        let full_path = resolve_workspace_path(&ctx.workspace_root, path)?;

        let content = fs::read_to_string(&full_path)?;
        let lines: Vec<&str> = content.lines().collect();

        let offset = args["offset"].as_u64().unwrap_or(1).max(1) as usize - 1;
        let limit = args["limit"].as_u64().unwrap_or(lines.len() as u64) as usize;

        let start = offset.min(lines.len());
        let end = (start + limit).min(lines.len());

        let result: Vec<String> = lines[start..end]
            .iter()
            .enumerate()
            .map(|(i, l)| format!("{:>6}: {}", start + i + 1, l))
            .collect();

        Ok(ToolResult::text(result.join("\n")))
    }
}

pub struct FsList;

#[async_trait]
impl Tool for FsList {
    fn name(&self) -> &str {
        "fs_list"
    }
    fn description(&self) -> &str {
        "List files and directories in the workspace"
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Relative directory to list" },
                "depth": { "type": "integer", "description": "Recursion depth (default 1)" }
            },
            "required": []
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::ReadOnly
    }
    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let path = args["path"].as_str().unwrap_or(".");
        let depth = args["depth"].as_u64().unwrap_or(1) as usize;
        let full_path = resolve_workspace_path(&ctx.workspace_root, path)?;

        let entries = list_dir(&full_path, depth, 0)?;
        Ok(ToolResult::text(entries.join("\n")))
    }
}

fn list_dir(path: &PathBuf, max_depth: usize, current_depth: usize) -> anyhow::Result<Vec<String>> {
    let mut result = Vec::new();
    if current_depth > max_depth {
        return Ok(result);
    }
    let prefix = "  ".repeat(current_depth);

    let mut entries: Vec<_> = fs::read_dir(path)?.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in &entries {
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if is_dir {
            result.push(format!("{}{}/", prefix, name));
            result.extend(list_dir(&entry.path(), max_depth, current_depth + 1)?);
        } else {
            result.push(format!("{}{}", prefix, name));
        }
    }
    Ok(result)
}

pub struct FsWrite;

#[async_trait]
impl Tool for FsWrite {
    fn name(&self) -> &str {
        "fs_write"
    }
    fn description(&self) -> &str {
        "Write or overwrite a file in the workspace"
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Relative path to write" },
                "content": { "type": "string", "description": "File content" }
            },
            "required": ["path", "content"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Mutating
    }
    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let path = args["path"].as_str().unwrap_or("");
        let content = args["content"].as_str().unwrap_or("");
        let full_path = resolve_workspace_path(&ctx.workspace_root, path)?;

        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&full_path, content)?;

        Ok(ToolResult::text(format!("Written: {}", path)))
    }
}
