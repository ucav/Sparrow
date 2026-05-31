use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::event::AutonomyLevel;

pub mod providers;
pub mod validate;

/// The full configuration tree (§11).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub defaults: Defaults,
    #[serde(default)]
    pub routing: Routing,
    #[serde(default)]
    pub budget: Budget,
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
    #[serde(default)]
    pub surfaces: SurfaceConfig,
    #[serde(default)]
    pub skills: SkillsConfig,
    #[serde(default)]
    pub theme: String,
    #[serde(default = "default_config_dir")]
    pub config_dir: PathBuf,
    #[serde(default = "default_state_dir")]
    pub state_dir: PathBuf,
}

fn default_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("sparrow")
}

fn default_state_dir() -> PathBuf {
    dirs::state_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("sparrow")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Defaults {
    #[serde(default = "default_autonomy")]
    pub autonomy: AutonomyLevel,
    #[serde(default = "default_sandbox")]
    pub sandbox: String,
    #[serde(default = "default_theme")]
    pub theme: String,
}

impl Default for Defaults {
    fn default() -> Self {
        Self {
            autonomy: default_autonomy(),
            sandbox: default_sandbox(),
            theme: default_theme(),
        }
    }
}

fn default_autonomy() -> AutonomyLevel {
    AutonomyLevel::Trusted
}
fn default_sandbox() -> String {
    "local-hardened".into()
}
fn default_theme() -> String {
    "captain".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Routing {
    #[serde(default = "default_true")]
    pub free_first: bool,
    #[serde(default = "default_policy")]
    pub policy: HashMap<String, String>,
    #[serde(default = "default_on_budget")]
    pub on_budget: String,
}

impl Default for Routing {
    fn default() -> Self {
        Self {
            free_first: default_true(),
            policy: default_policy(),
            on_budget: default_on_budget(),
        }
    }
}

fn default_true() -> bool {
    true
}
fn default_policy() -> HashMap<String, String> {
    HashMap::from([
        ("trivial".into(), "local".into()),
        ("small".into(), "groq".into()),
        ("medium".into(), "nvidia".into()),
        ("hard".into(), "anthropic".into()),
        ("vision".into(), "anthropic".into()),
    ])
}
fn default_on_budget() -> String {
    "downgrade".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Budget {
    #[serde(default = "default_five")]
    pub daily_usd: f64,
    #[serde(default = "default_one")]
    pub session_usd: f64,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            daily_usd: default_five(),
            session_usd: default_one(),
        }
    }
}

fn default_five() -> f64 {
    5.0
}
fn default_one() -> f64 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub adapter: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SurfaceConfig {
    #[serde(default)]
    pub telegram: Option<MessagingSurface>,
    #[serde(default)]
    pub discord: Option<MessagingSurface>,
    #[serde(default)]
    pub slack: Option<MessagingSurface>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagingSurface {
    pub enabled: bool,
    #[serde(default)]
    pub allow_users: Vec<String>,
    #[serde(default)]
    pub token_env: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsConfig {
    #[serde(default = "default_skills_dir")]
    pub dir: PathBuf,
    #[serde(default = "default_curator_cron")]
    pub curator_cron: String,
}

impl Default for SkillsConfig {
    fn default() -> Self {
        Self {
            dir: default_skills_dir(),
            curator_cron: default_curator_cron(),
        }
    }
}

fn default_skills_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("sparrow")
        .join("skills")
}

fn default_curator_cron() -> String {
    "0 */6 * * *".into()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            defaults: Defaults::default(),
            routing: Routing::default(),
            budget: Budget::default(),
            providers: std::collections::HashMap::new(),
            surfaces: SurfaceConfig::default(),
            skills: SkillsConfig::default(),
            theme: "captain".into(),
            config_dir: default_config_dir(),
            state_dir: default_state_dir(),
        }
    }
}

// ─── ConfigStore trait ──────────────────────────────────────────────────────────

/// Loads/merges config from defaults → config.toml → env (SPARROW_*) → CLI flags.
pub trait ConfigStore: Send + Sync {
    fn load(&self) -> anyhow::Result<Config>;
    fn save(&self, c: &Config) -> anyhow::Result<()>;
}

/// Filesystem-backed config store.
pub struct FsConfigStore {
    config_dir: PathBuf,
}

impl FsConfigStore {
    pub fn new(config_dir: PathBuf) -> Self {
        Self { config_dir }
    }

    fn config_path(&self) -> PathBuf {
        self.config_dir.join("config.toml")
    }

    /// Merge environment variables (SPARROW_*) into config.
    fn apply_env_overrides(cfg: &mut Config) {
        // SPARROW_DEFAULTS_AUTONOMY
        if let Ok(v) = std::env::var("SPARROW_DEFAULTS_AUTONOMY") {
            if let Ok(level) = serde_json::from_str::<AutonomyLevel>(&format!("\"{}\"", v)) {
                cfg.defaults.autonomy = level;
            }
        }
        // SPARROW_DEFAULTS_SANDBOX
        if let Ok(v) = std::env::var("SPARROW_DEFAULTS_SANDBOX") {
            cfg.defaults.sandbox = v;
        }
        // SPARROW_BUDGET_DAILY
        if let Ok(v) = std::env::var("SPARROW_BUDGET_DAILY") {
            if let Ok(amt) = v.parse::<f64>() {
                cfg.budget.daily_usd = amt;
            }
        }
        // SPARROW_BUDGET_SESSION
        if let Ok(v) = std::env::var("SPARROW_BUDGET_SESSION") {
            if let Ok(amt) = v.parse::<f64>() {
                cfg.budget.session_usd = amt;
            }
        }
        // SPARROW_THEME
        if let Ok(v) = std::env::var("SPARROW_THEME") {
            cfg.theme = v;
        }
    }
}

impl ConfigStore for FsConfigStore {
    fn load(&self) -> anyhow::Result<Config> {
        let path = self.config_path();
        let mut cfg = if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            toml::from_str::<Config>(&content)?
        } else {
            // Default config when no file exists
            let mut c = Config {
                defaults: Defaults::default(),
                routing: Routing::default(),
                budget: Budget::default(),
                providers: HashMap::new(),
                surfaces: SurfaceConfig::default(),
                skills: SkillsConfig::default(),
                theme: "captain".into(),
                config_dir: self.config_dir.clone(),
                state_dir: default_state_dir(),
            };
            // Auto-detect local ollama if available
            if let Ok(v) = std::env::var("OLLAMA_HOST") {
                c.providers.insert(
                    "ollama".into(),
                    ProviderConfig {
                        adapter: "ollama".into(),
                        base_url: Some(v),
                        models: vec![],
                        api_key_env: None,
                    },
                );
            }
            c
        };
        Self::apply_env_overrides(&mut cfg);
        Ok(cfg)
    }

    fn save(&self, c: &Config) -> anyhow::Result<()> {
        let path = self.config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(c)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}
