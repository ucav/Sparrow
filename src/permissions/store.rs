// ─── Permission persistence store ──────────────────────────────────────────────
//
// Persists per-tool permission decisions to disk so "allow always" survives
// across sessions. Stored as JSON in ~/.config/sparrow/permissions.json.
//
// Format:
// {
//   "tools": {
//     "web_search": "allow_always",
//     "fs_write": "allow_session",
//     "code_exec": "ask_user"
//   }
// }

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::event::Decision;

// ─── Store ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionStore {
    /// Per-tool decisions, keyed by tool name.
    pub tools: HashMap<String, PersistedDecision>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PersistedDecision {
    /// Always ask for confirmation.
    AskUser,
    /// Allow once — not persisted (handled in-memory), but accepted for API symmetry.
    AllowOnce,
    /// Allow for the remainder of the current session.
    AllowSession,
    /// Allow permanently.
    AllowAlways,
    /// Deny this tool.
    Deny,
}

impl PersistedDecision {
    pub fn to_decision(&self) -> Decision {
        match self {
            PersistedDecision::AskUser => Decision::AskUser,
            PersistedDecision::AllowOnce => Decision::AllowOnce,
            PersistedDecision::AllowSession => Decision::AllowSession,
            PersistedDecision::AllowAlways => Decision::AllowAlways,
            PersistedDecision::Deny => Decision::Deny,
        }
    }

    pub fn from_decision(d: &Decision) -> Option<Self> {
        match d {
            Decision::AskUser => Some(PersistedDecision::AskUser),
            Decision::AllowOnce => Some(PersistedDecision::AllowOnce),
            Decision::AllowSession => Some(PersistedDecision::AllowSession),
            Decision::AllowAlways => Some(PersistedDecision::AllowAlways),
            Decision::Deny => Some(PersistedDecision::Deny),
            // Plain Allow is not persisted — it's a one-time runtime decision
            Decision::Allow => None,
        }
    }

    /// Whether this decision means the tool should be allowed without asking.
    pub fn is_allowed(&self) -> bool {
        matches!(
            self,
            PersistedDecision::AllowOnce
                | PersistedDecision::AllowSession
                | PersistedDecision::AllowAlways
        )
    }

    /// Whether this decision is durable (survives sessions).
    pub fn is_durable(&self) -> bool {
        matches!(
            self,
            PersistedDecision::AllowAlways | PersistedDecision::Deny
        )
    }
}

impl PermissionStore {
    /// Load from disk, or return an empty store if the file doesn't exist.
    pub fn load(config_dir: &PathBuf) -> Self {
        let path = permissions_path(config_dir);
        if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            PermissionStore::default()
        }
    }

    /// Save to disk.
    pub fn save(&self, config_dir: &PathBuf) -> anyhow::Result<()> {
        let path = permissions_path(config_dir);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    /// Check if a tool has a persisted decision that allows it.
    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        self.tools
            .get(tool_name)
            .map(|d| d.is_allowed())
            .unwrap_or(false)
    }

    /// Check if a tool has a persisted decision that denies it.
    pub fn is_tool_denied(&self, tool_name: &str) -> bool {
        self.tools
            .get(tool_name)
            .map(|d| matches!(d, PersistedDecision::Deny))
            .unwrap_or(false)
    }

    /// Get the persisted decision for a tool, if any.
    pub fn get(&self, tool_name: &str) -> Option<&PersistedDecision> {
        self.tools.get(tool_name)
    }

    /// Set a decision for a tool and save to disk (only for durable decisions).
    pub fn set_and_save(
        &mut self,
        tool_name: &str,
        decision: &Decision,
        config_dir: &PathBuf,
    ) -> anyhow::Result<()> {
        if let Some(pd) = PersistedDecision::from_decision(decision) {
            if pd.is_durable() {
                self.tools.insert(tool_name.to_string(), pd);
                self.save(config_dir)?;
            }
        }
        Ok(())
    }

    /// Remove session-only decisions (AllowOnce, AllowSession) — called at startup.
    pub fn expire_session_scoped(&mut self) {
        self.tools.retain(|_, d| d.is_durable());
    }

    /// Convert to a JSON-serializable map for the WebView API.
    pub fn to_api_map(&self) -> HashMap<String, String> {
        self.tools
            .iter()
            .map(|(k, v)| {
                let s = match v {
                    PersistedDecision::AskUser => "ask_user",
                    PersistedDecision::AllowOnce => "allow_once",
                    PersistedDecision::AllowSession => "allow_session",
                    PersistedDecision::AllowAlways => "allow_always",
                    PersistedDecision::Deny => "deny",
                };
                (k.clone(), s.to_string())
            })
            .collect()
    }
}

impl Default for PermissionStore {
    fn default() -> Self {
        PermissionStore {
            tools: HashMap::new(),
        }
    }
}

fn permissions_path(config_dir: &PathBuf) -> PathBuf {
    config_dir.join("permissions.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_store_is_empty() {
        let store = PermissionStore::default();
        assert!(store.tools.is_empty());
    }

    #[test]
    fn test_is_tool_allowed() {
        let mut store = PermissionStore::default();
        store
            .tools
            .insert("web_search".into(), PersistedDecision::AllowAlways);
        assert!(store.is_tool_allowed("web_search"));
        assert!(!store.is_tool_allowed("code_exec"));
    }

    #[test]
    fn test_expire_session_scoped() {
        let mut store = PermissionStore::default();
        store
            .tools
            .insert("web_search".into(), PersistedDecision::AllowAlways);
        store
            .tools
            .insert("fs_read".into(), PersistedDecision::AllowSession);
        store.expire_session_scoped();
        assert!(store.tools.contains_key("web_search"));
        assert!(!store.tools.contains_key("fs_read"));
    }

    #[test]
    fn test_persist_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let mut store = PermissionStore::default();
        store
            .tools
            .insert("web_search".into(), PersistedDecision::AllowAlways);
        store.save(&dir).unwrap();
        let loaded = PermissionStore::load(&dir);
        assert!(loaded.is_tool_allowed("web_search"));
    }
}
