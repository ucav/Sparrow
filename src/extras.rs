use crate::engine::{Engine, Task};
use crate::event::Event;
use crate::memory::{Fact, Memory};
use std::sync::Arc;
use tokio::sync::mpsc;

// ─── Auto-distillation ─────────────────────────────────────────────────────────

/// After a successful run, extract durable facts about the user
/// from the conversation trajectory.
/// §3.8: "after sessions, distill durable facts/preferences into identity/facts_about_user"
pub struct Distiller;

impl Distiller {
    /// Analyze run events and extract durable facts about the user and project.
    /// Called automatically after every successful run (§3.8).
    pub async fn distill(memory: &Arc<dyn Memory>, events: &[Event], task_description: &str) {
        let mut facts = Vec::new();

        // ── 1. Languages & frameworks (from file paths) ──────────────────────
        let mut lang_hints: Vec<String> = Vec::new();
        let mut framework_hints: Vec<String> = Vec::new();
        let mut tool_usage: std::collections::HashMap<String, u32> =
            std::collections::HashMap::new();
        let mut style_hints: Vec<String> = Vec::new();
        let mut pref_hints: Vec<String> = Vec::new();
        let mut convention_hints: Vec<String> = Vec::new();
        let mut directive_hints: Vec<String> = Vec::new();

        for event in events {
            match event {
                Event::ToolUseProposed { name, args, .. } => {
                    // Track tool usage frequency
                    *tool_usage.entry(name.clone()).or_insert(0) += 1;

                    // Languages from file extensions
                    if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
                        detect_languages(path, &mut lang_hints);
                        detect_conventions(path, &mut convention_hints);
                    }
                    // Frameworks from content
                    if let Some(content) = args.get("content").and_then(|v| v.as_str()) {
                        detect_frameworks(content, &mut framework_hints);
                    }
                }
                Event::ThinkingDelta { text, .. } => {
                    // Style preferences
                    if text.contains("refactor") {
                        style_hints.push("refactoring-oriented".to_string());
                    }
                    if text.contains("test") || text.contains("TDD") {
                        style_hints.push("test-driven".to_string());
                    }
                    if text.contains("async") || text.contains("await") {
                        style_hints.push("async-first".to_string());
                    }
                    // Explicit user preferences mentioned in thinking
                    detect_preferences(text, &mut pref_hints);
                }
                Event::Message { text, role, .. } if role == "user" => {
                    // User messages often contain preferences
                    detect_preferences(text, &mut pref_hints);
                    // Explicit durable directives ("remember that…", "I prefer…",
                    // "always…", "my name is…"). Captured verbatim so memory keeps
                    // the real instruction, not a generic hint.
                    detect_directives(text, &mut directive_hints);
                }
                _ => {}
            }
        }

        // ── 2. Deduplicate ──────────────────────────────────────────────────
        dedup(&mut lang_hints);
        dedup(&mut framework_hints);
        dedup(&mut style_hints);
        dedup(&mut pref_hints);
        dedup(&mut convention_hints);
        dedup(&mut directive_hints);

        // ── 3. Save facts ───────────────────────────────────────────────────
        for lang in &lang_hints {
            facts.push(fact("user:language", lang));
        }
        for fw in &framework_hints {
            facts.push(fact("user:framework", fw));
        }
        for style in &style_hints {
            facts.push(fact("user:style", style));
        }
        for pref in &pref_hints {
            facts.push(fact("user:preference", pref));
        }
        for conv in &convention_hints {
            facts.push(fact("project:convention", conv));
        }
        // Each captured directive becomes its own fact keyed by a stable hash of
        // its text, so distinct directives never collide on a shared key.
        for d in &directive_hints {
            facts.push(fact(&format!("user:directive:{}", short_hash(d)), d));
        }
        // Tools used 3+ times become learned preferences
        for (tool, count) in &tool_usage {
            if *count >= 3 {
                facts.push(fact(
                    "user:frequent_tool",
                    &format!("uses {} frequently ({}x this session)", tool, count),
                ));
            }
        }

        // ── 4. Persist (skip duplicates) ────────────────────────────────────
        // v0.9.1 fix: deduplicate on the (key, value) PAIR, not the key alone.
        // The keys here are generic buckets (`user:preference`, `user:language`,
        // `user:directive`…). The previous `existing_keys.contains(key)` check
        // meant that once ONE `user:preference` existed, no further preference
        // — with a different value — was ever stored. Memory saturated after the
        // first run and stopped learning. Comparing the full pair lets every new
        // distinct fact land while still skipping exact repeats.
        let existing = memory.all_facts();
        let existing_pairs: std::collections::HashSet<(String, String)> = existing
            .iter()
            .map(|f| (f.key.clone(), f.value.clone()))
            .collect();
        let mut saved = 0;

        for fact in &facts {
            if !existing_pairs.contains(&(fact.key.clone(), fact.value.clone())) {
                let _ = memory.remember(fact.clone());
                saved += 1;
            }
        }

        if saved > 0 {
            tracing::info!(
                "Distiller: extracted {} facts ({} new) from task: {}",
                facts.len(),
                saved,
                &task_description[..task_description.len().min(60)]
            );
        }
    }
}

// ─── Distiller helpers ─────────────────────────────────────────────────────────

fn detect_languages(path: &str, hints: &mut Vec<String>) {
    let ext_map: &[(&str, &str)] = &[
        (".rs", "Rust"),
        (".ts", "TypeScript"),
        (".tsx", "TypeScript/React"),
        (".py", "Python"),
        (".go", "Go"),
        (".js", "JavaScript"),
        (".jsx", "JavaScript/React"),
        (".java", "Java"),
        (".rb", "Ruby"),
        (".css", "CSS"),
        (".html", "HTML"),
        (".sql", "SQL"),
        (".tf", "Terraform"),
        (".yml", "YAML"),
        (".yaml", "YAML"),
        (".toml", "TOML"),
        (".json", "JSON"),
        (".md", "Markdown"),
        (".sh", "Shell"),
    ];
    let lower = path.to_lowercase();
    for (ext, lang) in ext_map {
        if lower.ends_with(ext) {
            hints.push(lang.to_string());
            return;
        }
    }
}

fn detect_frameworks(content: &str, hints: &mut Vec<String>) {
    let fw_map: &[(&str, &str)] = &[
        ("Cargo.toml", "Rust/Cargo"),
        ("package.json", "Node.js"),
        ("go.mod", "Go modules"),
        ("requirements.txt", "Python/pip"),
        ("pyproject.toml", "Python/poetry"),
        ("Dockerfile", "Docker"),
        ("docker-compose", "Docker Compose"),
        ("Makefile", "Make"),
        ("CMakeLists.txt", "CMake"),
        ("pom.xml", "Java/Maven"),
        ("build.gradle", "Java/Gradle"),
    ];
    for (pattern, fw) in fw_map {
        if content.contains(pattern) {
            hints.push(fw.to_string());
        }
    }
}

fn detect_preferences(text: &str, hints: &mut Vec<String>) {
    let pref_patterns: &[(&str, &str)] = &[
        ("prefer async", "prefers async/await"),
        ("prefer sync", "prefers synchronous code"),
        ("use tabs", "uses tabs for indentation"),
        ("use spaces", "uses spaces for indentation"),
        (
            "prefer unwrap",
            "prefers .unwrap() over proper error handling",
        ),
        ("prefer anyhow", "prefers anyhow for error handling"),
        ("instead of", "has strong opinions about alternatives"),
        ("don't use", "has explicit dislikes"),
        ("always use", "has explicit preferences"),
        ("I like", "expressed a personal preference"),
        ("I want", "expressed a desire"),
    ];
    let lower = text.to_lowercase();
    for (pattern, hint) in pref_patterns {
        if lower.contains(pattern) {
            hints.push(hint.to_string());
        }
    }
}

/// Capture explicit durable directives from a user message — the kind of thing
/// a user expects the agent to *remember* across sessions. We keep the user's
/// actual sentence (trimmed to one line, capped) rather than a generic hint, so
/// recall is faithful. Triggers on common FR/EN durable-intent markers.
fn detect_directives(text: &str, hints: &mut Vec<String>) {
    const MARKERS: &[&str] = &[
        // English
        "remember that",
        "remember to",
        "don't forget",
        "keep in mind",
        "note that",
        "from now on",
        "always ",
        "never ",
        "my name is",
        "i prefer",
        "i want you to",
        "make sure to",
        "going forward",
        // French
        "retiens",
        "souviens-toi",
        "souviens toi",
        "n'oublie pas",
        "note que",
        "désormais",
        "dorénavant",
        "toujours ",
        "jamais ",
        "je m'appelle",
        "je préfère",
        "je veux que",
        "à partir de maintenant",
        "rappelle-toi",
    ];
    let lower = text.to_lowercase();
    for line in text.lines() {
        let line_l = line.to_lowercase();
        if MARKERS.iter().any(|m| line_l.contains(m)) {
            let cleaned = line.trim();
            if cleaned.len() >= 4 {
                // Cap to keep a fact compact; preserve the real instruction.
                let capped: String = cleaned.chars().take(220).collect();
                hints.push(capped);
            }
        }
    }
    // Fallback: single-line message with a marker but no line break handled above.
    if hints.is_empty() && MARKERS.iter().any(|m| lower.contains(m)) {
        let capped: String = text.trim().chars().take(220).collect();
        if capped.len() >= 4 {
            hints.push(capped);
        }
    }
}

/// Short stable hex hash for deriving collision-free fact keys from text.
fn short_hash(s: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.trim().to_lowercase().hash(&mut h);
    format!("{:08x}", (h.finish() & 0xffff_ffff) as u32)
}

fn detect_conventions(path: &str, hints: &mut Vec<String>) {
    let conv_patterns: &[(&str, &str)] = &[
        ("src/main.rs", "Rust binary project structure"),
        ("src/lib.rs", "Rust library project structure"),
        ("src/index.ts", "TypeScript entry point convention"),
        ("src/app.py", "Python app entry point"),
        ("tests/", "has a test directory"),
        ("spec/", "has a spec directory"),
        ("docs/", "maintains documentation"),
        (".github/workflows/", "uses GitHub Actions CI"),
        (".gitignore", "has gitignore"),
    ];
    let lower = path.to_lowercase();
    for (pattern, hint) in conv_patterns {
        if lower.contains(&pattern.to_lowercase()) {
            hints.push(hint.to_string());
        }
    }
}

fn dedup(v: &mut Vec<String>) {
    v.sort();
    v.dedup();
}

fn fact(key: &str, value: &str) -> Fact {
    Fact {
        id: uuid::Uuid::new_v4().to_string(),
        key: key.to_string(),
        value: value.to_string(),
        created_at: chrono::Utc::now().format("%Y-%m-%d").to_string(),
        updated_at: chrono::Utc::now().format("%Y-%m-%d").to_string(),
    }
}

#[cfg(test)]
mod distiller_tests {
    use super::*;

    #[test]
    fn detect_directives_captures_english_durable_instructions() {
        let mut h = Vec::new();
        detect_directives("Remember that I want reports in ./artifacts", &mut h);
        assert!(h.iter().any(|s| s.contains("reports in ./artifacts")));

        let mut h2 = Vec::new();
        detect_directives("From now on, always run the tests first", &mut h2);
        assert_eq!(h2.len(), 1);
    }

    #[test]
    fn detect_directives_captures_french_durable_instructions() {
        let mut h = Vec::new();
        detect_directives("Retiens que je m'appelle Abdou", &mut h);
        assert!(h.iter().any(|s| s.contains("Abdou")));

        let mut h2 = Vec::new();
        detect_directives("Désormais, range les livrables dans ./out", &mut h2);
        assert_eq!(h2.len(), 1);
    }

    #[test]
    fn detect_directives_ignores_ordinary_text() {
        let mut h = Vec::new();
        detect_directives("Can you fix the bug in main.rs please?", &mut h);
        assert!(h.is_empty());
    }

    #[test]
    fn short_hash_is_stable_and_distinct() {
        // Stable across calls, case/whitespace-insensitive, distinct for
        // different content — so directive keys never collide.
        assert_eq!(
            short_hash("use ./artifacts"),
            short_hash("  USE ./artifacts  ")
        );
        assert_ne!(short_hash("prefer async"), short_hash("prefer sync"));
        assert_eq!(short_hash("x").len(), 8);
    }
}

// ─── Lightweight deterministic embeddings ──────────────────────────────────────

/// Lightweight semantic embeddings for repo memory.
/// §3.8: "optional embeddings (per project)"
#[derive(Debug, Clone)]
pub struct Embeddings {
    /// Stored text + normalized hashing-vector embedding.
    pub vectors: Vec<(String, Vec<f64>)>,
    dimensions: usize,
}

impl Embeddings {
    pub const DEFAULT_DIMENSIONS: usize = 512;

    pub fn new() -> Self {
        Self {
            vectors: Vec::new(),
            dimensions: Self::DEFAULT_DIMENSIONS,
        }
    }

    pub fn with_dimensions(dimensions: usize) -> Self {
        Self {
            vectors: Vec::new(),
            dimensions: dimensions.max(16),
        }
    }

    /// Build a deterministic hashing-vector embedding from text.
    ///
    /// This is intentionally local-first: no model/API key required, fixed
    /// dimensions across documents, stable across sessions, and good enough for
    /// lexical semantic recall in memory. It uses signed feature hashing with
    /// unigram + adjacent bigram features, sublinear term frequency, and L2
    /// normalization.
    pub fn embed(&self, text: &str) -> Vec<f64> {
        embed_with_dimensions(text, self.dimensions)
    }

    pub fn add(&mut self, text: &str) {
        let clean = text.trim();
        if clean.is_empty() {
            return;
        }
        self.vectors.push((clean.to_string(), self.embed(clean)));
    }

    pub fn add_many<I, S>(&mut self, texts: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for text in texts {
            self.add(text.as_ref());
        }
    }

    /// Find the most similar stored text to the query
    pub fn search(&self, query: &str, k: usize) -> Vec<String> {
        self.search_scored(query, k)
            .into_iter()
            .map(|(_, text)| text)
            .collect()
    }

    pub fn search_scored(&self, query: &str, k: usize) -> Vec<(f64, String)> {
        if k == 0 {
            return Vec::new();
        }
        let q_embed = self.embed(query);
        let mut scored: Vec<(f64, usize, &str)> = self
            .vectors
            .iter()
            .enumerate()
            .map(|(idx, (text, emb))| (cosine_sim(&q_embed, emb), idx, text.as_str()))
            .collect();
        scored.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.1.cmp(&b.1))
        });
        scored
            .into_iter()
            .take(k)
            .filter(|(score, _, _)| *score > 0.0)
            .map(|(score, _, text)| (score, text.to_string()))
            .collect()
    }

    pub fn save_to_path(&self, path: impl AsRef<std::path::Path>) -> anyhow::Result<()> {
        let snapshot = EmbeddingsSnapshot {
            dimensions: self.dimensions,
            texts: self.vectors.iter().map(|(text, _)| text.clone()).collect(),
        };
        let json = serde_json::to_string_pretty(&snapshot)?;
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load_from_path(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let snapshot: EmbeddingsSnapshot = serde_json::from_str(&json)?;
        let mut index = Self::with_dimensions(snapshot.dimensions);
        index.add_many(snapshot.texts);
        Ok(index)
    }
}

impl Default for Embeddings {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct EmbeddingsSnapshot {
    dimensions: usize,
    texts: Vec<String>,
}

fn embed_with_dimensions(text: &str, dimensions: usize) -> Vec<f64> {
    let mut vector = vec![0.0; dimensions.max(16)];
    let tokens = tokenize(text);
    for token in &tokens {
        add_feature(&mut vector, token, 1.0);
    }
    for pair in tokens.windows(2) {
        add_feature(&mut vector, &format!("{}__{}", pair[0], pair[1]), 1.35);
    }
    for value in &mut vector {
        if *value != 0.0 {
            *value = value.signum() * value.abs().ln_1p();
        }
    }
    normalize(&mut vector);
    vector
}

fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            current.extend(ch.to_lowercase());
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn add_feature(vector: &mut [f64], feature: &str, weight: f64) {
    let hash = fnv1a64(feature.as_bytes());
    let idx = (hash as usize) % vector.len();
    let sign = if hash & (1 << 63) == 0 { 1.0 } else { -1.0 };
    vector[idx] += sign * weight;
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn normalize(vector: &mut [f64]) {
    let norm = vector.iter().map(|v| v * v).sum::<f64>().sqrt();
    if norm > 0.0 {
        for value in vector {
            *value /= norm;
        }
    }
}

fn cosine_sim(a: &[f64], b: &[f64]) -> f64 {
    let len = a.len().min(b.len());
    if len == 0 {
        return 0.0;
    }
    let dot: f64 = a.iter().zip(b.iter()).take(len).map(|(x, y)| x * y).sum();
    let norm_a: f64 = a.iter().take(len).map(|x| x * x).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().take(len).map(|x| x * x).sum::<f64>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
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
    /// Start a device code OAuth flow.
    /// Accepts the endpoints and scope from the provider registry — no hardcoded list.
    pub async fn start_device_flow(
        device_endpoint: &str,
        token_endpoint_hint: &str, // unused here, kept for symmetry
        client_id: &str,
        scope: &str,
    ) -> anyhow::Result<(String, String, String)> {
        let _ = token_endpoint_hint;
        let client = reqwest::Client::new();
        let resp: serde_json::Value = client
            .post(device_endpoint)
            .form(&[("client_id", client_id), ("scope", scope)])
            .send()
            .await?
            .json()
            .await?;

        let verification_uri = resp["verification_uri"]
            .as_str()
            .or_else(|| resp["verification_url"].as_str())
            .unwrap_or("")
            .to_string();
        let user_code = resp["user_code"].as_str().unwrap_or("").to_string();
        let device_code = resp["device_code"].as_str().unwrap_or("").to_string();

        if device_code.is_empty() {
            anyhow::bail!("Device flow start failed — provider response: {}", resp);
        }

        Ok((verification_uri, user_code, device_code))
    }

    /// Poll for token completion using the provider token endpoint.
    pub async fn poll_token(
        token_endpoint: &str,
        client_id: &str,
        device_code: &str,
        timeout_secs: u64,
    ) -> anyhow::Result<String> {
        let client = reqwest::Client::new();
        let start = std::time::Instant::now();

        loop {
            if start.elapsed().as_secs() > timeout_secs {
                anyhow::bail!("OAuth timed out after {}s", timeout_secs);
            }

            let resp: serde_json::Value = client
                .post(token_endpoint)
                .form(&[
                    ("client_id", client_id),
                    ("device_code", device_code),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ])
                .send()
                .await?
                .json()
                .await?;

            if let Some(token) = resp["access_token"].as_str() {
                return Ok(token.to_string());
            }

            match resp["error"].as_str() {
                Some("authorization_pending") | Some("slow_down") => {
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    continue;
                }
                Some(e) => anyhow::bail!("OAuth error: {}", e),
                None => {
                    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                }
            }
        }
    }
}

// ─── IBM Plex Mono reference ────────────────────────────────────────────────────

/// §9.3: "IBM Plex Mono everywhere (TUI authenticity + web)"
/// The font is not embedded in the binary; users install it system-wide.
/// This constant provides the download URL and instructions.
pub const IBM_PLEX_MONO_URL: &str =
    "https://github.com/IBM/plex/releases/latest/download/IBM-Plex-Mono.zip";

pub fn ibm_plex_install_instructions() -> String {
    r#"IBM Plex Mono — recommended font for Sparrow TUI.

Install:
  Linux:   sudo apt install fonts-ibm-plex
  macOS:   brew install font-ibm-plex
  Windows: Download from https://github.com/IBM/plex/releases

Then update your terminal to use "IBM Plex Mono" as the font.
"#
    .to_string()
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
        Self {
            engine,
            history: Vec::new(),
            running: true,
        }
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

            if input.is_empty() {
                continue;
            }
            if input == "/exit" || input == "/quit" {
                break;
            }

            self.history.push(crate::provider::Msg {
                role: "user".into(),
                content: vec![crate::provider::ContentBlock::Text {
                    text: input.clone(),
                }],
            });

            let (tx, mut rx) = mpsc::unbounded_channel::<Event>();
            let task = Task {
                description: input.clone(),
                context: self.history.clone(),
            };

            let engine = self.engine.clone();
            let handle = tokio::spawn(async move { engine.drive(task, tx).await });

            while let Some(event) = rx.recv().await {
                match &event {
                    Event::ThinkingDelta { text, .. } => {
                        print!("{}", text);
                        io::stdout().flush()?;
                    }
                    Event::RunFinished { outcome, .. } => {
                        println!(
                            "\n── {} | ${:.4} {}──",
                            outcome.status,
                            outcome.cost_usd,
                            crate::cost::format_comparison_oneliner(
                                outcome.cost_usd,
                                &outcome.tokens
                            )
                        );
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
                PipelineStep {
                    role: "planner".into(),
                    model_preference: None,
                    prompt_override: None,
                    depends_on: vec![],
                },
                PipelineStep {
                    role: "coder".into(),
                    model_preference: None,
                    prompt_override: None,
                    depends_on: vec!["planner".into()],
                },
                PipelineStep {
                    role: "verifier".into(),
                    model_preference: None,
                    prompt_override: None,
                    depends_on: vec!["coder".into()],
                },
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
                    experience: Default::default(),
                    skills: Default::default(),
                    intel: Default::default(),
                    permissions: Default::default(),
                    hooks: Default::default(),
                    theme: "captain".into(),
                    config_dir: config_dir.clone(),
                    state_dir: state_dir.clone(),
                    forced_model: None,
                }
            }
        };

        let memory: Arc<dyn Memory> = Arc::new(crate::memory::SqliteMemory::open(
            &state_dir.join("profile.db"),
        )?);

        Ok(Self {
            name: name.to_string(),
            config_dir,
            state_dir,
            config,
            memory,
        })
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
