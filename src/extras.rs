use crate::event::Event;
use crate::memory::{Fact, Memory};
use crate::engine::{Engine, Task};
use std::sync::Arc;
use tokio::sync::mpsc;

// ─── Auto-distillation ─────────────────────────────────────────────────────────

/// After a successful run, extract durable facts about the user
/// from the conversation trajectory.
/// §3.8: "after sessions, distill durable facts/preferences into identity/facts_about_user"
pub struct Distiller;

impl Distiller {
    /// Analyze run events and extract facts
    pub async fn distill(
        memory: &Arc<dyn Memory>,
        events: &[Event],
        task_description: &str,
    ) {
        let mut facts = Vec::new();

        // Extract user preferences from tools used
        let mut lang_hints = Vec::new();
        let mut framework_hints = Vec::new();
        let mut style_hints = Vec::new();

        for event in events {
            match event {
                Event::ToolUseProposed { args, .. } => {
                    if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
                        if path.ends_with(".rs") { lang_hints.push("Rust".to_string()); }
                        if path.ends_with(".ts") || path.ends_with(".tsx") { lang_hints.push("TypeScript".to_string()); }
                        if path.ends_with(".py") { lang_hints.push("Python".to_string()); }
                        if path.ends_with(".go") { lang_hints.push("Go".to_string()); }
                        if path.ends_with(".js") || path.ends_with(".jsx") { lang_hints.push("JavaScript".to_string()); }
                    }
                    if let Some(content) = args.get("content").and_then(|v| v.as_str()) {
                        if content.contains("Cargo.toml") { framework_hints.push("Rust/Cargo".to_string()); }
                        if content.contains("package.json") { framework_hints.push("Node.js".to_string()); }
                        if content.contains("go.mod") { framework_hints.push("Go modules".to_string()); }
                    }
                }
                Event::ThinkingDelta { text, .. } => {
                    if text.contains("refactor") { style_hints.push("prefers refactoring".to_string()); }
                    if text.contains("test") || text.contains("TDD") { style_hints.push("test-driven".to_string()); }
                }
                _ => {}
            }
        }

        // Deduplicate and save facts
        lang_hints.sort(); lang_hints.dedup();
        framework_hints.sort(); framework_hints.dedup();
        style_hints.sort(); style_hints.dedup();

        for lang in &lang_hints {
            facts.push(Fact {
                id: uuid::Uuid::new_v4().to_string(),
                key: "user:language".into(),
                value: lang.clone(),
                created_at: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                updated_at: chrono::Utc::now().format("%Y-%m-%d").to_string(),
            });
        }
        for fw in &framework_hints {
            facts.push(Fact {
                id: uuid::Uuid::new_v4().to_string(),
                key: "user:framework".into(),
                value: fw.clone(),
                created_at: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                updated_at: chrono::Utc::now().format("%Y-%m-%d").to_string(),
            });
        }
        for style in &style_hints {
            facts.push(Fact {
                id: uuid::Uuid::new_v4().to_string(),
                key: "user:style".into(),
                value: style.clone(),
                created_at: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                updated_at: chrono::Utc::now().format("%Y-%m-%d").to_string(),
            });
        }

        // Save deduplicated facts
        let existing = memory.all_facts();
        let existing_keys: Vec<&str> = existing.iter().map(|f| f.key.as_str()).collect();

        for fact in facts {
            if !existing_keys.contains(&fact.key.as_str()) {
                let _ = memory.remember(fact);
            }
        }

        if !lang_hints.is_empty() || !framework_hints.is_empty() {
            tracing::info!("Distiller: extracted {} facts from session", lang_hints.len() + framework_hints.len() + style_hints.len());
        }
    }
}

// ─── Embeddings stub ────────────────────────────────────────────────────────────

/// Lightweight semantic embeddings for repo memory.
/// §3.8: "optional embeddings (per project)"
pub struct Embeddings {
    /// Simple TF-IDF-like word frequency vectors
    pub vectors: Vec<(String, Vec<f64>)>,
}

impl Embeddings {
    pub fn new() -> Self {
        Self { vectors: Vec::new() }
    }

    /// Build a simple embedding from text (bag-of-words normalized)
    pub fn embed(&self, text: &str) -> Vec<f64> {
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut freq = std::collections::HashMap::new();
        for w in &words {
            *freq.entry(w.to_lowercase()).or_insert(0.0) += 1.0;
        }
        let norm = (freq.values().map(|v| v * v).sum::<f64>()).sqrt();
        if norm > 0.0 {
            freq.values_mut().for_each(|v| *v /= norm);
        }
        freq.into_values().collect()
    }

    /// Find the most similar stored text to the query
    pub fn search(&self, query: &str, k: usize) -> Vec<String> {
        let q_embed = self.embed(query);
        let mut scored: Vec<(f64, &str)> = self.vectors.iter()
            .map(|(text, emb)| (cosine_sim(&q_embed, emb), text.as_str()))
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().take(k).map(|(_, t)| t.to_string()).collect()
    }

    pub fn add(&mut self, text: &str) {
        self.vectors.push((text.to_string(), self.embed(text)));
    }
}

fn cosine_sim(a: &[f64], b: &[f64]) -> f64 {
    let len = a.len().min(b.len());
    if len == 0 { return 0.0; }
    let dot: f64 = a.iter().zip(b.iter()).take(len).map(|(x, y)| x * y).sum();
    let norm_a: f64 = a.iter().take(len).map(|x| x * x).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().take(len).map(|x| x * x).sum::<f64>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 { 0.0 } else { dot / (norm_a * norm_b) }
}

// ─── Replay re-execute ──────────────────────────────────────────────────────────

/// Re-execute a transcript against a chosen model.
/// §3.15: "can re-execute against a chosen model"
pub struct ReExecuter {
    engine: Arc<Engine>,
}

impl ReExecuter {
    pub fn new(engine: Arc<Engine>) -> Self {
        Self { engine }
    }

    /// Re-execute from a transcript: send the original task to the engine
    /// with the same parameters.
    pub async fn re_execute(
        &self,
        transcript: &crate::runtime::recorder::Transcript,
    ) -> anyhow::Result<crate::event::OutcomeSummary> {
        let (tx, _rx) = mpsc::unbounded_channel::<Event>();
        let task = Task {
            description: transcript.inputs.task.clone(),
            context: vec![],
        };
        self.engine.drive(task, tx).await
    }
}

// ─── OAuth flow ─────────────────────────────────────────────────────────────────

pub struct OAuthFlow;

impl OAuthFlow {
    /// Start a device code OAuth flow for a provider.
    /// Returns the URL the user should visit and the device code.
    pub async fn start_device_flow(
        provider: &str,
        client_id: &str,
    ) -> anyhow::Result<(String, String, String)> {
        let device_url = match provider {
            "github" => "https://github.com/login/device/code",
            "google" => "https://oauth2.googleapis.com/device/code",
            "microsoft" => "https://login.microsoftonline.com/common/oauth2/v2.0/devicecode",
            _ => anyhow::bail!("OAuth device flow not supported for {}", provider),
        };

        let client = reqwest::Client::new();
        let resp: serde_json::Value = client
            .post(device_url)
            .form(&[
                ("client_id", client_id),
                ("scope", if provider == "github" { "read:user" } else { "openid profile" }),
            ])
            .send().await?
            .json().await?;

        Ok((
            resp["verification_uri"].as_str().unwrap_or("").to_string(),
            resp["user_code"].as_str().unwrap_or("").to_string(),
            resp["device_code"].as_str().unwrap_or("").to_string(),
        ))
    }

    /// Poll for OAuth token completion
    pub async fn poll_token(
        provider: &str,
        client_id: &str,
        device_code: &str,
        timeout_secs: u64,
    ) -> anyhow::Result<String> {
        let token_url = match provider {
            "github" => "https://github.com/login/oauth/access_token",
            "google" => "https://oauth2.googleapis.com/token",
            _ => anyhow::bail!("OAuth not supported for {}", provider),
        };

        let client = reqwest::Client::new();
        let start = std::time::Instant::now();

        loop {
            if start.elapsed().as_secs() > timeout_secs {
                anyhow::bail!("OAuth timed out after {}s", timeout_secs);
            }

            let resp: serde_json::Value = client
                .post(token_url)
                .form(&[
                    ("client_id", client_id),
                    ("device_code", device_code),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ])
                .send().await?
                .json().await?;

            if let Some(token) = resp["access_token"].as_str() {
                return Ok(token.to_string());
            }

            if resp["error"].as_str() == Some("authorization_pending") {
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                continue;
            }

            anyhow::bail!("OAuth error: {:?}", resp["error"]);
        }
    }
}

// ─── IBM Plex Mono reference ────────────────────────────────────────────────────

/// §9.3: "IBM Plex Mono everywhere (TUI authenticity + web)"
/// The font is not embedded in the binary; users install it system-wide.
/// This constant provides the download URL and instructions.
pub const IBM_PLEX_MONO_URL: &str = "https://github.com/IBM/plex/releases/latest/download/IBM-Plex-Mono.zip";

pub fn ibm_plex_install_instructions() -> String {
    r#"IBM Plex Mono — recommended font for Sparrow TUI.

Install:
  Linux:   sudo apt install fonts-ibm-plex
  macOS:   brew install font-ibm-plex
  Windows: Download from https://github.com/IBM/plex/releases

Then update your terminal to use "IBM Plex Mono" as the font.
"#.to_string()
}

// ─── Chat mode ──────────────────────────────────────────────────────────────────

/// Interactive multi-turn chat loop.
/// §4: "sparrow chat — interactive multi-turn (TUI/inline)"
pub struct ChatSession {
    engine: Arc<Engine>,
    history: Vec<crate::provider::Msg>,
    running: bool,
}

impl ChatSession {
    pub fn new(engine: Arc<Engine>) -> Self {
        Self { engine, history: Vec::new(), running: true }
    }

    pub async fn run_interactive(&mut self) -> anyhow::Result<()> {
        use std::io::{self, Write};

        println!("═══ Sparrow Chat ═══");
        println!("Type your message and press Enter. Type /exit to quit.");
        println!();

        while self.running {
            print!("◆ you › ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim().to_string();

            if input.is_empty() { continue; }
            if input == "/exit" || input == "/quit" { break; }

            self.history.push(crate::provider::Msg {
                role: "user".into(),
                content: vec![crate::provider::ContentBlock::Text { text: input.clone() }],
            });

            let (tx, mut rx) = mpsc::unbounded_channel::<Event>();
            let task = Task {
                description: input.clone(),
                context: self.history.clone(),
            };

            let engine = self.engine.clone();
            let handle = tokio::spawn(async move {
                engine.drive(task, tx).await
            });

            while let Some(event) = rx.recv().await {
                match &event {
                    Event::ThinkingDelta { text, .. } => {
                        print!("{}", text);
                        io::stdout().flush()?;
                    }
                    Event::RunFinished { outcome, .. } => {
                        println!("\n── {} | ${:.4} ──", outcome.status, outcome.cost_usd);
                    }
                    Event::Error { message, .. } => {
                        eprintln!("\nError: {}", message);
                    }
                    _ => {}
                }
            }

            match handle.await? {
                Ok(outcome) => {
                    self.history.push(crate::provider::Msg {
                        role: "assistant".into(),
                        content: vec![crate::provider::ContentBlock::Text {
                            text: format!("[{}]", outcome.status),
                        }],
                    });
                }
                Err(e) => {
                    eprintln!("Chat error: {}", e);
                }
            }
            println!();
        }

        Ok(())
    }
}

// ─── Configurable pipeline ──────────────────────────────────────────────────────

use crate::orchestrator::SwarmPlan;

/// Allow users to define custom swarm pipeline graphs.
/// §3.11: "Configurable: number of agents, which model per role, pipeline graph."
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PipelineConfig {
    pub name: String,
    pub steps: Vec<PipelineStep>,
    pub max_reworks: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PipelineStep {
    pub role: String,
    pub model_preference: Option<String>,
    pub prompt_override: Option<String>,
    pub depends_on: Vec<String>,
}

impl PipelineConfig {
    pub fn default_pipeline() -> Self {
        Self {
            name: "planner-coder-verifier".into(),
            steps: vec![
                PipelineStep { role: "planner".into(), model_preference: None, prompt_override: None, depends_on: vec![] },
                PipelineStep { role: "coder".into(), model_preference: None, prompt_override: None, depends_on: vec!["planner".into()] },
                PipelineStep { role: "verifier".into(), model_preference: None, prompt_override: None, depends_on: vec!["coder".into()] },
            ],
            max_reworks: 3,
        }
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        if self.steps.is_empty() {
            anyhow::bail!("Pipeline must have at least one step");
        }
        for step in &self.steps {
            for dep in &step.depends_on {
                if !self.steps.iter().any(|s| s.role == *dep) {
                    anyhow::bail!("Step '{}' depends on unknown role '{}'", step.role, dep);
                }
            }
        }
        Ok(())
    }

    pub fn from_toml(content: &str) -> anyhow::Result<Self> {
        Ok(toml::from_str(content)?)
    }

    pub fn to_toml(&self) -> anyhow::Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }
}

// ─── Profile isolation ──────────────────────────────────────────────────────────

/// Full profile isolation: separate config, memory, agents per profile.
/// §4: "sparrow profile <create|list|use> — multi-instance profiles"
pub struct Profile {
    pub name: String,
    pub config_dir: std::path::PathBuf,
    pub state_dir: std::path::PathBuf,
    pub config: crate::config::Config,
    pub memory: Arc<dyn Memory>,
}

impl Profile {
    pub fn load(name: &str) -> anyhow::Result<Self> {
        let base_config = dirs::config_dir().unwrap_or_default().join("sparrow");
        let base_state = dirs::state_dir().unwrap_or_default().join("sparrow");

        let config_dir = base_config.join("profiles").join(name);
        let state_dir = base_state.join("profiles").join(name);

        std::fs::create_dir_all(&config_dir)?;
        std::fs::create_dir_all(&state_dir)?;

        let config = if config_dir.join("config.toml").exists() {
            let content = std::fs::read_to_string(config_dir.join("config.toml"))?;
            toml::from_str(&content)?
        } else {
            // Inherit from default config if available
            let default = base_config.join("config.toml");
            if default.exists() {
                let content = std::fs::read_to_string(&default)?;
                toml::from_str(&content)?
            } else {
                crate::config::Config {
                    defaults: Default::default(),
                    routing: Default::default(),
                    budget: Default::default(),
                    providers: Default::default(),
                    surfaces: Default::default(),
                    skills: Default::default(),
                    theme: "captain".into(),
                    config_dir: config_dir.clone(),
                    state_dir: state_dir.clone(),
                }
            }
        };

        let memory: Arc<dyn Memory> = Arc::new(
            crate::memory::SqliteMemory::open(&state_dir.join("profile.db"))?
        );

        Ok(Self { name: name.to_string(), config_dir, state_dir, config, memory })
    }

    pub fn create(name: &str) -> anyhow::Result<()> {
        let base_config = dirs::config_dir().unwrap_or_default().join("sparrow");
        let config_dir = base_config.join("profiles").join(name);
        std::fs::create_dir_all(&config_dir)?;

        // Copy default config
        let default = base_config.join("config.toml");
        if default.exists() {
            std::fs::copy(&default, config_dir.join("config.toml"))?;
        }

        let base_state = dirs::state_dir().unwrap_or_default().join("sparrow");
        std::fs::create_dir_all(base_state.join("profiles").join(name))?;

        Ok(())
    }

    pub fn list() -> Vec<String> {
        let base_config = dirs::config_dir().unwrap_or_default().join("sparrow");
        let profiles_dir = base_config.join("profiles");
        let mut names = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&profiles_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        names.push(name.to_string());
                    }
                }
            }
        }
        names.sort();
        names
    }
}
