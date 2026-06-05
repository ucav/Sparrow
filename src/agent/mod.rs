use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

use crate::engine::Identity;
use crate::memory::Memory;

// ─── SOUL: persistent agent definition ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Soul {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub role: String,
    pub personality: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub rules: Vec<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub disallowed_tools: Vec<String>,
    #[serde(default)]
    pub default_model: Option<String>,
    #[serde(default)]
    pub default_autonomy: Option<String>,
    #[serde(default)]
    pub permission_mode: Option<String>,
    #[serde(default)]
    pub mcp_servers: Vec<String>,
    #[serde(default)]
    pub max_turns: Option<u32>,
    #[serde(default)]
    pub memory: Option<bool>,
    #[serde(default)]
    pub background: bool,
    #[serde(default)]
    pub isolation: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
}

impl Soul {
    pub fn to_identity(&self) -> Identity {
        Identity {
            name: self.name.clone(),
            role: self.role.clone(),
            personality: if self.prompt.trim().is_empty() {
                self.personality.clone()
            } else {
                format!("{}\n\n{}", self.personality, self.prompt)
            },
        }
    }

    pub fn to_toml(&self) -> anyhow::Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }

    pub fn from_toml(content: &str) -> anyhow::Result<Self> {
        Ok(toml::from_str(content)?)
    }

    pub fn from_markdown_frontmatter(content: &str) -> anyhow::Result<Self> {
        let Some(rest) = content.strip_prefix("---") else {
            anyhow::bail!("agent markdown is missing frontmatter");
        };
        let Some((frontmatter, body)) = rest.split_once("---") else {
            anyhow::bail!("agent markdown frontmatter is not closed");
        };
        let mut soul = Soul::default();
        for line in frontmatter.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim().trim_matches('"').trim_matches('\'');
            match key {
                "name" => soul.name = value.to_string(),
                "description" => soul.description = value.to_string(),
                "role" => soul.role = value.to_string(),
                "personality" => soul.personality = value.to_string(),
                "prompt" => soul.prompt = value.to_string(),
                "tools" => soul.tools = parse_list(value),
                "disallowed_tools" => soul.disallowed_tools = parse_list(value),
                "model" | "default_model" => soul.default_model = nonempty(value),
                "default_autonomy" => soul.default_autonomy = nonempty(value),
                "permission_mode" => soul.permission_mode = nonempty(value),
                "mcp_servers" => soul.mcp_servers = parse_list(value),
                "max_turns" => soul.max_turns = value.parse::<u32>().ok(),
                "memory" => soul.memory = parse_bool(value),
                "background" => soul.background = parse_bool(value).unwrap_or(false),
                "isolation" => soul.isolation = nonempty(value),
                "color" => soul.color = nonempty(value),
                _ => {}
            }
        }
        let body = body.trim();
        if !body.is_empty() {
            soul.prompt = if soul.prompt.trim().is_empty() {
                body.to_string()
            } else {
                format!("{}\n\n{}", soul.prompt, body)
            };
        }
        Ok(soul)
    }
}

impl Default for Soul {
    fn default() -> Self {
        Self {
            name: "sparrow".into(),
            description: String::new(),
            role: "senior software engineer".into(),
            personality: "concise, competent, direct. Prefers working code over explanation."
                .into(),
            prompt: String::new(),
            rules: vec![],
            tools: vec![],
            disallowed_tools: vec![],
            default_model: None,
            default_autonomy: Some("supervised".into()),
            permission_mode: None,
            mcp_servers: vec![],
            max_turns: None,
            memory: None,
            background: false,
            isolation: None,
            color: None,
        }
    }
}

fn nonempty(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_lowercase().as_str() {
        "true" | "yes" | "1" => Some(true),
        "false" | "no" | "0" => Some(false),
        _ => None,
    }
}

fn parse_list(value: &str) -> Vec<String> {
    let value = value.trim();
    let value = value
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .unwrap_or(value);
    value
        .split(',')
        .map(|item| item.trim().trim_matches('"').trim_matches('\''))
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

// ─── Agent store trait ──────────────────────────────────────────────────────────

pub trait AgentStore: Send + Sync {
    fn create(&self, soul: &Soul) -> anyhow::Result<()>;
    fn get(&self, name: &str) -> Option<Soul>;
    fn list(&self) -> Vec<Soul>;
    fn update(&self, name: &str, soul: &Soul) -> anyhow::Result<()>;
    fn remove(&self, name: &str) -> anyhow::Result<()>;
}

// ─── Filesystem-backed agent store (SOUL files as TOML) ─────────────────────────

pub struct FsAgentStore {
    agents_dir: PathBuf,
    memory: Option<Arc<dyn Memory>>,
}

impl FsAgentStore {
    pub fn new(agents_dir: PathBuf) -> Self {
        Self {
            agents_dir,
            memory: None,
        }
    }

    pub fn with_memory(mut self, memory: Arc<dyn Memory>) -> Self {
        self.memory = Some(memory);
        self
    }

    fn soul_path(&self, name: &str) -> PathBuf {
        self.agents_dir.join(format!(
            "{}.soul.toml",
            safe_agent_name(name).unwrap_or("invalid")
        ))
    }

    fn find_agent_path(&self, name: &str) -> Option<PathBuf> {
        let name = safe_agent_name(name).ok()?;
        let toml_path = self.soul_path(name);
        if toml_path.exists() {
            return Some(toml_path);
        }
        let md_path = self.agents_dir.join(format!("{}.agent.md", name));
        if md_path.exists() {
            return Some(md_path);
        }
        None
    }
}

impl AgentStore for FsAgentStore {
    fn create(&self, soul: &Soul) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.agents_dir)?;
        safe_agent_name(&soul.name)?;
        let path = self.soul_path(&soul.name);
        if path.exists() {
            anyhow::bail!(
                "Agent '{}' already exists. Use 'edit' to modify.",
                soul.name
            );
        }
        let content = soul.to_toml()?;
        std::fs::write(&path, content)?;

        // Persist to memory if available
        if let Some(mem) = &self.memory {
            mem.save_identity(&soul.name, &soul.to_identity())?;
        }
        Ok(())
    }

    fn get(&self, name: &str) -> Option<Soul> {
        let path = self.find_agent_path(name)?;
        read_soul_file(&path)
    }

    fn list(&self) -> Vec<Soul> {
        let mut souls = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.agents_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(soul) = read_soul_file(&path) {
                    souls.push(soul);
                }
            }
        }
        souls.sort_by(|a, b| a.name.cmp(&b.name));
        souls
    }

    fn update(&self, name: &str, soul: &Soul) -> anyhow::Result<()> {
        safe_agent_name(name)?;
        safe_agent_name(&soul.name)?;
        let path = self
            .find_agent_path(name)
            .ok_or_else(|| anyhow::anyhow!("Agent '{}' not found.", name))?;
        let content = soul.to_toml()?;
        std::fs::write(&path, content)?;

        if let Some(mem) = &self.memory {
            mem.save_identity(&soul.name, &soul.to_identity())?;
        }
        Ok(())
    }

    fn remove(&self, name: &str) -> anyhow::Result<()> {
        if let Some(path) = self.find_agent_path(name) {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }
}

fn read_soul_file(path: &std::path::Path) -> Option<Soul> {
    let content = std::fs::read_to_string(path).ok()?;
    match path.extension().and_then(|e| e.to_str()) {
        Some("toml") => Soul::from_toml(&content).ok(),
        Some("md") => Soul::from_markdown_frontmatter(&content).ok(),
        _ => None,
    }
}

fn safe_agent_name(name: &str) -> anyhow::Result<&str> {
    let trimmed = name.trim();
    if trimmed.is_empty()
        || trimmed.contains("..")
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains(':')
    {
        anyhow::bail!("invalid agent name '{}'", name);
    }
    Ok(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soul_toml_roundtrip_keeps_declarative_fields() {
        let soul = Soul {
            name: "reviewer".into(),
            description: "Adversarial reviewer".into(),
            role: "verifier".into(),
            personality: "strict".into(),
            tools: vec!["fs_read".into()],
            disallowed_tools: vec!["fs_write".into()],
            default_model: Some("nvidia:test".into()),
            permission_mode: Some("read-only".into()),
            max_turns: Some(8),
            background: true,
            color: Some("gold".into()),
            ..Soul::default()
        };
        let encoded = soul.to_toml().unwrap();
        let decoded = Soul::from_toml(&encoded).unwrap();
        assert_eq!(decoded.name, "reviewer");
        assert_eq!(decoded.disallowed_tools, vec!["fs_write"]);
        assert_eq!(decoded.permission_mode.as_deref(), Some("read-only"));
        assert_eq!(decoded.max_turns, Some(8));
    }

    #[test]
    fn markdown_frontmatter_agent_is_parsed() {
        let content = r#"---
name: verifier
description: Checks code
role: verifier
personality: adversarial
tools: [fs_read, search]
disallowed_tools: [fs_write, exec]
model: nvidia/test
permission_mode: read-only
max_turns: 5
background: true
color: gold
---
Review every claim against evidence.
"#;
        let soul = Soul::from_markdown_frontmatter(content).unwrap();
        assert_eq!(soul.name, "verifier");
        assert_eq!(soul.tools, vec!["fs_read", "search"]);
        assert_eq!(soul.disallowed_tools, vec!["fs_write", "exec"]);
        assert_eq!(soul.default_model.as_deref(), Some("nvidia/test"));
        assert_eq!(soul.permission_mode.as_deref(), Some("read-only"));
        assert_eq!(soul.max_turns, Some(5));
        assert!(soul.background);
        assert!(soul.prompt.contains("Review every claim"));
    }

    #[test]
    fn fs_agent_store_get_finds_agent_md() {
        let dir = std::env::temp_dir().join(format!(
            "sparrow-agent-md-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let md_content = "---\nname: scout\ndescription: Finds issues\nrole: verifier\npersonality: sharp\ntools: [fs_read]\n---\nBe thorough.";
        std::fs::write(dir.join("scout.agent.md"), md_content).unwrap();

        let store = FsAgentStore::new(dir.clone());

        let found = store.get("scout");
        assert!(found.is_some(), "get() should find .agent.md files");
        let soul = found.unwrap();
        assert_eq!(soul.name, "scout");
        assert_eq!(soul.tools, vec!["fs_read"]);

        let listed = store.list();
        assert!(listed.iter().any(|s| s.name == "scout"));

        store.remove("scout").unwrap();
        assert!(store.get("scout").is_none());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn fs_agent_store_rejects_path_traversal_names() {
        let dir = std::env::temp_dir().join(format!(
            "sparrow-agent-escape-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = FsAgentStore::new(dir.clone());
        let soul = Soul {
            name: "../outside".into(),
            ..Soul::default()
        };

        assert!(store.create(&soul).is_err());
        assert!(store.get("../outside").is_none());

        let _ = std::fs::remove_dir_all(dir);
    }
}
