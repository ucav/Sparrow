use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::event::{Block, RiskLevel};

pub mod fs;
pub mod edit;
pub mod exec;
pub mod git;
pub mod todo;
pub mod search_and_web;
pub mod subagent;
pub mod extras;
pub mod builder_tools;

// ─── Tool context ───────────────────────────────────────────────────────────────

pub struct ToolCtx {
    pub workspace_root: std::path::PathBuf,
    pub run_id: crate::event::RunId,
}

pub fn resolve_workspace_path(workspace_root: &Path, path: &str) -> anyhow::Result<PathBuf> {
    let root = workspace_root.canonicalize().unwrap_or_else(|_| workspace_root.to_path_buf());
    let candidate = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        root.join(path)
    };

    let check_target = if candidate.exists() {
        candidate.canonicalize()?
    } else {
        let parent = candidate
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Invalid path: {}", path))?;
        let parent = parent.canonicalize().unwrap_or_else(|_| parent.to_path_buf());
        parent.join(
            candidate
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Invalid path: {}", path))?,
        )
    };

    if !check_target.starts_with(&root) {
        anyhow::bail!("Path escapes workspace: {}", path);
    }

    Ok(check_target)
}

// ─── Tool result ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: Vec<Block>,
    pub is_error: bool,
}

impl ToolResult {
    pub fn ok(content: Vec<Block>) -> Self {
        Self {
            content,
            is_error: false,
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            content: vec![Block::Text(msg.into())],
            is_error: true,
        }
    }

    pub fn text(msg: impl Into<String>) -> Self {
        Self {
            content: vec![Block::Text(msg.into())],
            is_error: false,
        }
    }
}

// ─── THE TOOL TRAIT ─────────────────────────────────────────────────────────────

/// What an agent can do. Every tool declares a JSON schema and a risk level
/// used by the autonomy gate.
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> serde_json::Value;
    fn risk(&self) -> RiskLevel;
    async fn call(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
    ) -> anyhow::Result<ToolResult>;
}

// ─── Tool registry (ToolSet) ────────────────────────────────────────────────────

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn all(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.values().cloned().collect()
    }

    pub fn names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    pub fn to_specs(&self) -> Vec<super::provider::ToolSpec> {
        self.tools
            .values()
            .map(|t| super::provider::ToolSpec {
                name: t.name().to_string(),
                description: t.description().to_string(),
                input_schema: t.schema(),
            })
            .collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
