use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

use crate::engine::Identity;
use crate::memory::Memory;

// ─── SOUL: persistent agent definition ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Soul {
    pub name: String,
    pub role: String,
    pub personality: String,
    #[serde(default)]
    pub rules: Vec<String>,
    #[serde(default)]
    pub default_model: Option<String>,
    #[serde(default)]
    pub default_autonomy: Option<String>,
}

impl Soul {
    pub fn to_identity(&self) -> Identity {
        Identity {
            name: self.name.clone(),
            role: self.role.clone(),
            personality: self.personality.clone(),
        }
    }

    pub fn to_toml(&self) -> anyhow::Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }

    pub fn from_toml(content: &str) -> anyhow::Result<Self> {
        Ok(toml::from_str(content)?)
    }
}

impl Default for Soul {
    fn default() -> Self {
        Self {
            name: "sparrow".into(),
            role: "senior software engineer".into(),
            personality: "concise, competent, direct. Prefers working code over explanation.".into(),
            rules: vec![],
            default_model: None,
            default_autonomy: Some("supervised".into()),
        }
    }
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
        self.agents_dir.join(format!("{}.soul.toml", name))
    }
}

impl AgentStore for FsAgentStore {
    fn create(&self, soul: &Soul) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.agents_dir)?;
        let path = self.soul_path(&soul.name);
        if path.exists() {
            anyhow::bail!("Agent '{}' already exists. Use 'edit' to modify.", soul.name);
        }
        let content = soul.to_toml()?;
        std::fs::write(&path, content)?;

        // Persist to memory if available
        if let Some(mem) = &self.memory {
            mem.save_identity(&soul.to_identity())?;
        }
        Ok(())
    }

    fn get(&self, name: &str) -> Option<Soul> {
        let path = self.soul_path(name);
        if !path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(&path).ok()?;
        Soul::from_toml(&content).ok()
    }

    fn list(&self) -> Vec<Soul> {
        let mut souls = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.agents_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "toml").unwrap_or(false) {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(soul) = Soul::from_toml(&content) {
                            souls.push(soul);
                        }
                    }
                }
            }
        }
        souls.sort_by(|a, b| a.name.cmp(&b.name));
        souls
    }

    fn update(&self, name: &str, soul: &Soul) -> anyhow::Result<()> {
        let path = self.soul_path(name);
        if !path.exists() {
            anyhow::bail!("Agent '{}' not found.", name);
        }
        let content = soul.to_toml()?;
        std::fs::write(&path, content)?;

        // Update in memory
        if let Some(mem) = &self.memory {
            mem.save_identity(&soul.to_identity())?;
        }
        Ok(())
    }

    fn remove(&self, name: &str) -> anyhow::Result<()> {
        let path = self.soul_path(name);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }
}
