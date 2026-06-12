use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use sparrow_core::event::AutonomyLevel;
use crate::permissions::PermissionConfig;

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
    pub experience: ExperienceConfig,
    #[serde(default)]
    pub skills: SkillsConfig,
    #[serde(default)]
    pub intel: IntelConfig,
    #[serde(default)]
    pub permissions: PermissionConfig,
    #[serde(default)]
    pub hooks: Vec<crate::hooks::Hook>,
    #[serde(default)]
    pub theme: String,
    #[serde(default = "default_config_dir")]
    pub config_dir: PathBuf,
    #[serde(default = "default_state_dir")]
    pub state_dir: PathBuf,
    #[serde(skip)]
    pub forced_model: Option<(String, String)>,
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
    /// Optional verification command run after mutating batches (e.g. "cargo build").
    /// On non-zero exit, the failure is re-injected so the agent fixes it.
    #[serde(default)]
    pub verify_command: Option<String>,
    /// Whether to create Git-backed checkpoints before mutating tools. Default
    /// true. Disabled by the `--no-checkpoint` CLI flag (which was parsed but
    /// never enforced).
    #[serde(default = "default_true")]
    pub checkpointing: bool,
}

impl Default for Defaults {
    fn default() -> Self {
        Self {
            autonomy: default_autonomy(),
            sandbox: default_sandbox(),
            theme: default_theme(),
            verify_command: None,
            checkpointing: true,
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
    /// When true, automatically scan /v1/models on every provider as soon as an
    /// API key is stored, and cache the results for 24h. Defaults to true.
    #[serde(default = "default_true")]
    pub auto_discover: bool,
    /// Pin ALL routing tiers to a single provider. When set, this overrides
    /// every entry in `policy` (but still respects capability hard constraints
    /// like vision/tools). Set via `sparrow route set <provider>` or directly
    /// in config.yaml under `routing.preferred_provider`.
    #[serde(default)]
    pub preferred_provider: Option<String>,
    /// Pin ALL routing tiers to a single MODEL. When set with routing_mode=manual,
    /// Sparrow uses exactly this model (e.g. \"deepseek-v4-pro\") and never falls
    /// back. Set via `sparrow route model <model>`.
    #[serde(default)]
    pub preferred_model: Option<String>,
    /// Routing mode: \"auto\" (tier-based policy + free_first) or \"manual\"
    /// (always use preferred_provider or the model the user picked, never
    /// auto-fallback). Set via `sparrow route manual`.
    #[serde(default = "default_routing_mode")]
    pub routing_mode: String,
}

impl Default for Routing {
    fn default() -> Self {
        Self {
            free_first: default_true(),
            policy: default_policy(),
            on_budget: default_on_budget(),
            auto_discover: true,
            preferred_provider: None,
            preferred_model: None,
            routing_mode: default_routing_mode(),
        }
    }
}

fn default_routing_mode() -> String {
    "auto".into()
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
    /// Hard wall-clock cap for a single run, in seconds. `None` = no time cap.
    /// Set by the `--max-wall-secs` CLI flag (was parsed but never enforced).
    #[serde(default)]
    pub max_wall_secs: Option<u64>,
    /// Hard cap on total tokens (input + output) for a single run. `None` =
    /// no token cap. Set by `--max-tokens`.
    #[serde(default)]
    pub max_tokens: Option<u64>,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            daily_usd: default_five(),
            session_usd: default_one(),
            max_wall_secs: None,
            max_tokens: None,
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

/// v0.9 Pilier 2 — how Sparrow talks to the user. Stored separately from the
/// messaging `surfaces` (telegram/discord/…) which are a different concept.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperienceConfig {
    /// "simple" (plain language, no jargon) · "builder" (common build
    /// workflows) · "pro" (full technical output) · "auto" (start simple, the
    /// user can switch). Default: auto.
    #[serde(default = "default_experience_mode")]
    pub mode: String,
    /// "fr" · "en" · "auto" (follow the system locale, fall back to French).
    #[serde(default = "default_experience_language")]
    pub language: String,
}

fn default_experience_mode() -> String {
    "auto".into()
}

fn default_experience_language() -> String {
    "auto".into()
}

impl Default for ExperienceConfig {
    fn default() -> Self {
        Self {
            mode: default_experience_mode(),
            language: default_experience_language(),
        }
    }
}

impl ExperienceConfig {
    /// True when output should use the plain-language (simple) layer. `auto`
    /// resolves to simple — v0.9's default is human-first; pro users opt in
    /// with `mode = "pro"` (or `sparrow mode pro`).
    pub fn is_simple(&self) -> bool {
        matches!(self.mode_name(), "simple" | "auto")
    }

    pub fn is_builder(&self) -> bool {
        self.mode_name() == "builder"
    }

    pub fn mode_name(&self) -> &str {
        let mode = self.mode.trim();
        if mode.eq_ignore_ascii_case("simple") {
            "simple"
        } else if mode.eq_ignore_ascii_case("builder") {
            "builder"
        } else if mode.eq_ignore_ascii_case("pro") {
            "pro"
        } else {
            "auto"
        }
    }

    /// Resolved display language for the human layer.
    pub fn lang(&self) -> crate::humanize::Lang {
        let code = self.language.trim().to_lowercase();
        if code == "auto" || code.is_empty() {
            crate::humanize::Lang::from_code(&detect_locale())
        } else {
            crate::humanize::Lang::from_code(&code)
        }
    }
}

/// Best-effort system locale, used only when language = "auto".
fn detect_locale() -> String {
    for key in ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
        if let Ok(val) = std::env::var(key) {
            if !val.trim().is_empty() {
                return val;
            }
        }
    }
    // Windows: no standard env locale; default to French (Sparrow's primary).
    String::new()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SurfaceConfig {
    #[serde(default)]
    pub telegram: Option<MessagingSurface>,
    #[serde(default)]
    pub discord: Option<MessagingSurface>,
    #[serde(default)]
    pub slack: Option<MessagingSurface>,
    #[serde(default)]
    pub email: Option<EmailSurface>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailSurface {
    pub enabled: bool,
    pub from: String,
    pub smtp_host: String,
    #[serde(default = "default_smtp_port")]
    pub smtp_port: u16,
    pub username_env: String,
    pub password_env: String,
    #[serde(default)]
    pub allowed_to: Vec<String>,
    /// Optional IMAP server for inbound polling.
    #[serde(default)]
    pub imap_host: Option<String>,
    #[serde(default = "default_imap_port")]
    pub imap_port: u16,
}

fn default_smtp_port() -> u16 {
    587
}

fn default_imap_port() -> u16 {
    993
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

/// Public competitive intelligence is opt-in. When disabled, Sparrow may read
/// cached digests/backlog locally, but it must not fetch the network.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IntelConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub sources: Vec<IntelSourceConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelSourceConfig {
    pub name: String,
    pub kind: String,
    pub url: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl IntelSourceConfig {
    pub fn to_sparrow_intel(&self) -> Option<sparrow_intel::SourceConfig> {
        let kind = match self.kind.trim().to_lowercase().as_str() {
            "github_releases" => sparrow_intel::SourceKind::GithubReleases,
            "changelog_url" => sparrow_intel::SourceKind::ChangelogUrl,
            "docs_url" => sparrow_intel::SourceKind::DocsUrl,
            _ => return None,
        };
        Some(sparrow_intel::SourceConfig {
            name: self.name.clone(),
            kind,
            url: self.url.clone(),
            tags: self.tags.clone(),
        })
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            defaults: Defaults::default(),
            routing: Routing::default(),
            budget: Budget::default(),
            providers: std::collections::HashMap::new(),
            surfaces: SurfaceConfig::default(),
            experience: ExperienceConfig::default(),
            skills: SkillsConfig::default(),
            intel: IntelConfig::default(),
            permissions: PermissionConfig::default(),
            hooks: Vec::new(),
            theme: "captain".into(),
            config_dir: default_config_dir(),
            state_dir: default_state_dir(),
            forced_model: None,
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
            if !v.trim().is_empty() {
                cfg.theme = v;
            }
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
                experience: ExperienceConfig::default(),
                skills: SkillsConfig::default(),
                intel: IntelConfig::default(),
                permissions: PermissionConfig::default(),
                hooks: Vec::new(),
                theme: "captain".into(),
                config_dir: self.config_dir.clone(),
                state_dir: default_state_dir(),
                forced_model: None,
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
        if cfg.theme.trim().is_empty() {
            cfg.theme = default_theme();
        }
        Ok(cfg)
    }

    fn save(&self, c: &Config) -> anyhow::Result<()> {
        let path = self.config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(c)?;
        std::fs::write(&path, human_config_header().to_string() + &content)?;
        Ok(())
    }
}

pub fn human_config_header() -> &'static str {
    "# Configuration Sparrow\n\
     # Mode simple par défaut : tu peux changer ce fichier à la main, puis relancer Sparrow.\n\
     # Les clés API restent dans tes variables d'environnement ou le coffre Sparrow, pas ici.\n\
     # Pour revenir au mode expert : `sparrow mode pro` ou `sparrow launch --pro`.\n\n"
}

/// Merge configured providers with auto-detected ones (env vars, stored credentials).
/// Used by setup and routing to show what's actually available.
pub fn effective_provider_configs(config: &Config) -> HashMap<String, ProviderConfig> {
    use crate::auth::AuthStore; // bring the .get(...) trait method into scope
    let mut effective = config.providers.clone();
    let auth = crate::auth::store::ChainedAuthStore::new(config.config_dir.clone());

    for (name, pconfig) in effective.iter_mut() {
        if pconfig.models.is_empty() {
            pconfig.models = providers::default_models(name);
        }
    }

    for def in providers::provider_registry() {
        if effective.contains_key(&def.id) {
            continue;
        }

        let has_env_credential = def
            .api_key_env
            .as_ref()
            .map(|env| {
                if def.adapter == "ollama" {
                    true
                } else {
                    std::env::var(env)
                        .map(|value| !value.trim().is_empty())
                        .unwrap_or(false)
                }
            })
            .unwrap_or(def.adapter == "ollama");
        let has_stored_credential = auth.get(&def.id).is_some();

        if !has_env_credential && !has_stored_credential {
            continue;
        }

        let base_url = if def.adapter == "ollama" {
            std::env::var("OLLAMA_HOST")
                .ok()
                .or(Some(def.base_url.clone()))
        } else {
            Some(def.base_url.clone())
        };

        effective.insert(
            def.id.clone(),
            ProviderConfig {
                adapter: def.adapter,
                base_url,
                models: providers::default_models(&def.id),
                api_key_env: def.api_key_env,
            },
        );
    }

    effective
}
