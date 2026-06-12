use chrono::{DateTime, Utc};
use secrecy::{ExposeSecret, SecretString};

pub mod store;
// ─── Credential types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Credential {
    ApiKey(SecretString),
    OAuth {
        access: SecretString,
        refresh: Option<SecretString>,
        expires: Option<DateTime<Utc>>,
    },
}

impl Credential {
    pub fn api_key(key: impl Into<String>) -> Self {
        Credential::ApiKey(SecretString::new(key.into().into_boxed_str()))
    }

    pub fn expose_key(&self) -> Option<&str> {
        match self {
            Credential::ApiKey(k) => Some(k.expose_secret()),
            Credential::OAuth { access, .. } => Some(access.expose_secret()),
        }
    }

    pub fn is_expired(&self) -> bool {
        match self {
            Credential::ApiKey(_) => false,
            Credential::OAuth {
                expires: Some(exp), ..
            } => *exp <= Utc::now(),
            _ => false,
        }
    }
}

// ─── AuthStore trait ────────────────────────────────────────────────────────────

/// Stores provider credentials.
/// Backend priority: OS keychain → encrypted file → env.
pub trait AuthStore: Send + Sync {
    fn get(&self, provider: &str) -> Option<Credential>;
    fn set(&self, provider: &str, c: Credential) -> anyhow::Result<()>;
    fn list(&self) -> Vec<String>;
    fn remove(&self, provider: &str) -> anyhow::Result<()>;
}

// ─── In-memory + env-based implementation (no keychain for initial build) ────────

use std::collections::HashMap;
use std::sync::RwLock;

pub struct MemoryAuthStore {
    credentials: RwLock<HashMap<String, Credential>>,
}

impl MemoryAuthStore {
    pub fn new() -> Self {
        let mut store = Self {
            credentials: RwLock::new(HashMap::new()),
        };
        store.load_from_env();
        store
    }

    /// Scan environment for provider keys (ANTHROPIC_API_KEY, OPENAI_API_KEY, etc.)
    fn load_from_env(&mut self) {
        let env_map: Vec<(&str, &str)> = vec![
            ("ANTHROPIC_API_KEY", "anthropic"),
            ("OPENAI_API_KEY", "openai"),
            ("GROQ_API_KEY", "groq"),
            ("TOGETHER_API_KEY", "together"),
            ("NVIDIA_API_KEY", "nvidia"),
            ("CEREBRAS_API_KEY", "cerebras"),
            ("OPENROUTER_API_KEY", "openrouter"),
        ];

        let mut creds = self.credentials.write().unwrap();
        for (env_var, provider) in env_map {
            if let Ok(key) = std::env::var(env_var) {
                if !key.is_empty() {
                    creds.insert(provider.to_string(), Credential::api_key(key));
                }
            }
        }
    }
}

impl AuthStore for MemoryAuthStore {
    fn get(&self, provider: &str) -> Option<Credential> {
        let creds = self.credentials.read().unwrap();
        creds.get(provider).cloned()
    }

    fn set(&self, provider: &str, c: Credential) -> anyhow::Result<()> {
        let mut creds = self.credentials.write().unwrap();
        creds.insert(provider.to_string(), c);
        Ok(())
    }

    fn list(&self) -> Vec<String> {
        let creds = self.credentials.read().unwrap();
        creds.keys().cloned().collect()
    }

    fn remove(&self, provider: &str) -> anyhow::Result<()> {
        let mut creds = self.credentials.write().unwrap();
        creds.remove(provider);
        Ok(())
    }
}

impl Default for MemoryAuthStore {
    fn default() -> Self {
        Self::new()
    }
}
