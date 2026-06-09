use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::event::{Block, RiskLevel};

pub mod browser_sandbox;
pub mod builder_tools;
pub mod code_exec;
pub mod code_nav;
pub mod edit;
pub mod exec;
pub mod extras;
pub mod file_search;
pub mod fs;
pub mod git;
pub mod knowledge_graph;
pub mod media;
pub mod memory;
pub mod search_and_web;
pub mod stt;
pub mod subagent;
pub mod todo;
pub mod tts;
pub mod voice;
pub mod web_search;

// ─── Tool context ───────────────────────────────────────────────────────────────

pub struct ToolCtx {
    pub workspace_root: std::path::PathBuf,
    pub run_id: crate::event::RunId,
}

pub fn resolve_workspace_path(workspace_root: &Path, path: &str) -> anyhow::Result<PathBuf> {
    let root = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
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
        let parent = parent
            .canonicalize()
            .unwrap_or_else(|_| parent.to_path_buf());
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
    fn metadata(&self) -> ToolMetadata {
        metadata_for(self.name(), self.risk())
    }
    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> anyhow::Result<ToolResult>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolMetadata {
    pub name: String,
    pub toolset: String,
    pub risk: RiskLevel,
    pub requires_auth: bool,
    pub mutates_files: bool,
    pub network: bool,
    pub exec: bool,
}

pub const TOOLSETS: &[&str] = &[
    "safe",
    "web",
    "file",
    "terminal",
    "media",
    "debug",
    "skills",
    "memory",
    "session_search",
    "mcp",
    "gateway",
];

pub const KNOWN_TOOLS: &[(&str, RiskLevel)] = &[
    ("fs_read", RiskLevel::ReadOnly),
    ("fs_list", RiskLevel::ReadOnly),
    ("fs_write", RiskLevel::Mutating),
    ("edit", RiskLevel::Mutating),
    ("multi_edit", RiskLevel::Mutating),
    ("search", RiskLevel::Network),
    ("web_search", RiskLevel::Network),
    ("web_fetch", RiskLevel::Network),
    ("browser", RiskLevel::Network),
    ("computer", RiskLevel::Exec),
    ("git", RiskLevel::Exec),
    ("todo", RiskLevel::ReadOnly),
    ("exec", RiskLevel::Exec),
    ("image_generate", RiskLevel::Network),
    ("text_to_speech", RiskLevel::Network),
    ("transcribe", RiskLevel::Network),
    ("python_rpc", RiskLevel::Exec),
    ("lsp", RiskLevel::ReadOnly),
    ("glob", RiskLevel::ReadOnly),
    ("symbols", RiskLevel::ReadOnly),
    ("memory", RiskLevel::Mutating),
    ("knowledge_graph", RiskLevel::Mutating),
    ("subagent_spawn", RiskLevel::Exec),
];

pub fn known_tool_metadata(surface: Option<&str>) -> Vec<ToolMetadata> {
    KNOWN_TOOLS
        .iter()
        .map(|(name, risk)| metadata_for(name, risk.clone()))
        .filter(|meta| surface.map(|s| surface_allows(s, meta)).unwrap_or(true))
        .collect()
}

pub fn metadata_for(name: &str, risk: RiskLevel) -> ToolMetadata {
    let lower = name.to_ascii_lowercase();
    let toolset = if matches!(lower.as_str(), "fs_read" | "fs_list" | "glob" | "symbols") {
        "file"
    } else if matches!(lower.as_str(), "fs_write" | "edit" | "multi_edit") {
        "file"
    } else if matches!(
        lower.as_str(),
        "search" | "web_search" | "web_fetch" | "browser"
    ) {
        "web"
    } else if lower == "computer" {
        "terminal"
    } else if lower == "exec" || lower == "git" {
        "terminal"
    } else if matches!(
        lower.as_str(),
        "image_gen" | "image_generate" | "tts" | "text_to_speech" | "transcribe"
    ) {
        "media"
    } else if lower == "memory" || lower == "knowledge_graph" {
        "memory"
    } else if lower.contains("session") {
        "session_search"
    } else if lower == "python_rpc" {
        "terminal"
    } else if lower == "lsp" {
        "debug"
    } else if lower.contains("mcp") {
        "mcp"
    } else if lower.contains("subagent") {
        "skills"
    } else if lower == "todo" {
        "safe"
    } else {
        "safe"
    };
    ToolMetadata {
        name: name.to_string(),
        toolset: toolset.to_string(),
        requires_auth: matches!(toolset, "web" | "media" | "mcp" | "gateway"),
        mutates_files: matches!(risk, RiskLevel::Mutating | RiskLevel::Destructive)
            || matches!(lower.as_str(), "fs_write" | "edit" | "multi_edit"),
        network: matches!(risk, RiskLevel::Network) || matches!(toolset, "web" | "mcp" | "gateway"),
        exec: matches!(risk, RiskLevel::Exec) || toolset == "terminal",
        risk,
    }
}

pub fn surface_allows(surface: &str, metadata: &ToolMetadata) -> bool {
    match surface.trim().to_ascii_lowercase().as_str() {
        "gateway" => {
            !metadata.exec
                && !metadata.mutates_files
                && !matches!(metadata.risk, RiskLevel::Destructive)
                && !matches!(metadata.toolset.as_str(), "terminal" | "file")
        }
        "subagent" => !matches!(metadata.risk, RiskLevel::Destructive),
        "cli" | "tui" | "webview" | "" => true,
        _ => true,
    }
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

    pub fn metadata(&self) -> Vec<ToolMetadata> {
        self.tools.values().map(|tool| tool.metadata()).collect()
    }

    pub fn metadata_for_surface(&self, surface: &str) -> Vec<ToolMetadata> {
        self.metadata()
            .into_iter()
            .filter(|meta| surface_allows(surface, meta))
            .collect()
    }

    pub fn to_specs_for_surface(&self, surface: &str) -> Vec<super::provider::ToolSpec> {
        self.tools
            .values()
            .filter(|tool| surface_allows(surface, &tool.metadata()))
            .map(|t| super::provider::ToolSpec {
                name: t.name().to_string(),
                description: t.description().to_string(),
                input_schema: t.schema(),
            })
            .collect()
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
