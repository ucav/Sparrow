use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::memory::Memory;

pub mod mcp;
pub mod plugin;

// ─── Skill ──────────────────────────────────────────────────────────────────────

/// A reusable capability: a SKILL.md file loaded into agent context when relevant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    /// Keywords used to match this skill against a task context
    pub trigger: Vec<String>,
    /// The body/content loaded into the agent's system prompt or context
    pub body: String,
    /// Source file path (relative to skills dir)
    #[serde(default)]
    pub source_file: String,
    /// How many times this skill was used/loaded
    #[serde(default)]
    pub usage_count: u32,
    /// Creation timestamp
    #[serde(default)]
    pub created_at: String,
    /// Quality score (0.0–1.0), used by curator for pruning
    #[serde(default = "default_score")]
    pub score: f64,
    /// Whether this skill was auto-generated (by Curator) or user-created
    #[serde(default)]
    pub auto_generated: bool,
    /// Optional references loaded only when the skill is explicitly invoked.
    #[serde(default)]
    pub references: Vec<String>,
    /// Optional templates loaded only when explicitly requested.
    #[serde(default)]
    pub templates: Vec<String>,
    /// Optional scripts advertised by the skill.
    #[serde(default)]
    pub scripts: Vec<String>,
    /// Optional assets advertised by the skill.
    #[serde(default)]
    pub assets: Vec<String>,
    /// Optional manifest version for permission-aware skills.
    #[serde(default)]
    pub manifest_version: Option<String>,
    /// Optional allow-list of tools while this skill is active.
    #[serde(default)]
    pub allowed_tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInvocation {
    pub skill: Skill,
    pub loaded_references: Vec<(String, String)>,
    pub loaded_templates: Vec<(String, String)>,
    pub loaded_scripts: Vec<(String, String)>,
    pub loaded_assets: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillManifest {
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub allowed_tools: Vec<String>,
}

fn default_score() -> f64 {
    0.5
}

impl Skill {
    /// Parse a SKILL.md file into a Skill struct.
    /// Format:
    /// ```markdown
    /// # Skill: <name>
    ///
    /// **Trigger:** comma, separated, keywords
    ///
    /// ## Body
    /// <content>
    /// ```
    pub fn from_markdown(content: &str, source_file: &str) -> Option<Self> {
        let mut name = String::new();
        let mut description = String::new();
        let mut trigger = Vec::new();
        let mut body = String::new();
        let mut references = Vec::new();
        let mut templates = Vec::new();
        let mut scripts = Vec::new();
        let mut assets = Vec::new();
        let mut allowed_tools = Vec::new();
        let mut in_body = false;

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("# Skill:") || trimmed.starts_with("# ") {
                name = trimmed
                    .trim_start_matches("# Skill:")
                    .trim_start_matches("# ")
                    .trim()
                    .to_string();
                continue;
            }

            if trimmed.starts_with("**Trigger:**") || trimmed.starts_with("**Triggers:**") {
                let trig_str = trimmed
                    .trim_start_matches("**Trigger:**")
                    .trim_start_matches("**Triggers:**")
                    .trim();
                trigger = trig_str
                    .split(',')
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect();
                continue;
            }

            if trimmed.starts_with("**Description:**") {
                description = trimmed
                    .trim_start_matches("**Description:**")
                    .trim()
                    .to_string();
                continue;
            }
            if trimmed.starts_with("**References:**") {
                references = parse_csv_field(trimmed.trim_start_matches("**References:**"));
                continue;
            }
            if trimmed.starts_with("**Templates:**") {
                templates = parse_csv_field(trimmed.trim_start_matches("**Templates:**"));
                continue;
            }
            if trimmed.starts_with("**Scripts:**") {
                scripts = parse_csv_field(trimmed.trim_start_matches("**Scripts:**"));
                continue;
            }
            if trimmed.starts_with("**Assets:**") {
                assets = parse_csv_field(trimmed.trim_start_matches("**Assets:**"));
                continue;
            }
            if trimmed.starts_with("**Allowed Tools:**") {
                allowed_tools = parse_csv_field(trimmed.trim_start_matches("**Allowed Tools:**"));
                continue;
            }

            if trimmed == "## Body" || trimmed == "### Body" {
                in_body = true;
                continue;
            }

            if in_body {
                body.push_str(line);
                body.push('\n');
            }
        }

        if name.is_empty() {
            return None;
        }

        if body.is_empty() && !in_body {
            // If no ## Body header, take everything after the header as body
            body = content
                .lines()
                .skip_while(|l| !l.starts_with("**Trigger"))
                .skip(1)
                .collect::<Vec<_>>()
                .join("\n");
        }

        Some(Skill {
            name,
            description: if description.is_empty() {
                trigger.join(", ")
            } else {
                description
            },
            trigger,
            body: body.trim().to_string(),
            source_file: source_file.to_string(),
            usage_count: 0,
            created_at: chrono::Utc::now().format("%Y-%m-%d").to_string(),
            score: 0.5,
            auto_generated: false,
            references,
            templates,
            scripts,
            assets,
            manifest_version: None,
            allowed_tools,
        })
    }

    /// Convert a Skill back to SKILL.md markdown format
    pub fn to_markdown(&self) -> String {
        format!(
            "# Skill: {name}\n\n\
             **Trigger:** {trigger}\n\n\
             **Description:** {desc}\n\n\
             **References:** {references}\n\n\
             **Templates:** {templates}\n\n\
             **Scripts:** {scripts}\n\n\
             **Assets:** {assets}\n\n\
             **Allowed Tools:** {allowed_tools}\n\n\
             ## Body\n\
             {body}\n",
            name = self.name,
            trigger = self.trigger.join(", "),
            desc = self.description,
            references = self.references.join(", "),
            templates = self.templates.join(", "),
            scripts = self.scripts.join(", "),
            assets = self.assets.join(", "),
            allowed_tools = self.allowed_tools.join(", "),
            body = self.body,
        )
    }

    /// Check if this skill is relevant to a given context string.
    /// Returns a relevance score 0.0–1.0.
    pub fn relevance(&self, ctx: &str) -> f64 {
        let lower = ctx.to_lowercase();
        if self.trigger.is_empty() {
            return 0.0;
        }
        let matches: usize = self
            .trigger
            .iter()
            .filter(|kw| lower.contains(kw.as_str()))
            .count();
        if matches == 0 {
            return 0.0;
        }
        matches as f64 / self.trigger.len() as f64
    }
}

fn parse_csv_field(value: &str) -> Vec<String> {
    value
        .trim()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn apply_skill_manifest(skill: &mut Skill, dir: &Path) {
    if let Some(manifest) = load_skill_manifest(dir) {
        if manifest.version.is_some() {
            skill.manifest_version = manifest.version;
        }
        if !manifest.allowed_tools.is_empty() {
            skill.allowed_tools = manifest.allowed_tools;
        }
    }
}

fn load_skill_manifest(dir: &Path) -> Option<SkillManifest> {
    let toml_path = dir.join("manifest.toml");
    if toml_path.exists() {
        return std::fs::read_to_string(toml_path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok());
    }
    let json_path = dir.join("manifest.json");
    if json_path.exists() {
        return std::fs::read_to_string(json_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok());
    }
    None
}

// ─── THE SKILL LIBRARY TRAIT ────────────────────────────────────────────────────

pub trait SkillLibrary: Send + Sync {
    fn relevant(&self, ctx: &str, limit: usize) -> Vec<Skill>;
    fn add(&self, skill: Skill) -> anyhow::Result<()>;
    fn all(&self) -> Vec<Skill>;
    fn curate(&self) -> anyhow::Result<()>;
    fn prune(&self, min_score: f64) -> anyhow::Result<usize>;
    fn get(&self, name: &str) -> Option<Skill>;
    fn invoke(&self, name: &str) -> anyhow::Result<Option<SkillInvocation>>;
    /// Remove a skill by name (any kind). Returns true if it existed.
    fn remove(&self, name: &str) -> anyhow::Result<bool>;
    /// On-disk root for the library, if any. In-memory implementations
    /// return None; `sparrow skills update` reads SKILL.md from here.
    fn skills_root(&self) -> Option<std::path::PathBuf> {
        None
    }
}

// ─── Filesystem-backed skill library ────────────────────────────────────────────

pub struct FsSkillLibrary {
    skills_dir: PathBuf,
    memory: Option<Arc<dyn Memory>>,
}

impl FsSkillLibrary {
    pub fn new(skills_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&skills_dir).ok();
        Self {
            skills_dir,
            memory: None,
        }
    }

    pub fn with_memory(mut self, memory: Arc<dyn Memory>) -> Self {
        self.memory = Some(memory);
        self
    }

    /// Path to the skills root. Exposed so commands like
    /// `sparrow skills update` can re-read a skill from disk without going
    /// through the cached library view.
    pub fn skills_dir(&self) -> &std::path::Path {
        &self.skills_dir
    }

    /// Scan the skills directory and load all SKILL.md files
    pub fn scan(&self) -> Vec<Skill> {
        let mut skills = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.skills_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    // Look for SKILL.md inside subdirectories
                    let skill_file = path.join("SKILL.md");
                    if skill_file.exists() {
                        if let Ok(content) = std::fs::read_to_string(&skill_file) {
                            let rel = path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            if let Some(mut skill) = Skill::from_markdown(&content, &rel) {
                                apply_skill_manifest(&mut skill, &path);
                                skills.push(skill);
                            }
                        }
                    }
                } else if path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_lowercase().ends_with(".skill.md"))
                    .unwrap_or(false)
                {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let rel = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        if let Some(mut skill) = Skill::from_markdown(&content, &rel) {
                            apply_skill_manifest(
                                &mut skill,
                                path.parent().unwrap_or(&self.skills_dir),
                            );
                            skills.push(skill);
                        }
                    }
                }
            }
        }
        skills
    }
}

impl SkillLibrary for FsSkillLibrary {
    fn skills_root(&self) -> Option<std::path::PathBuf> {
        Some(self.skills_dir.clone())
    }

    fn relevant(&self, ctx: &str, limit: usize) -> Vec<Skill> {
        let mut scored: Vec<(f64, Skill)> = self
            .scan()
            .into_iter()
            .map(|s| {
                let r = s.relevance(ctx);
                (r, s)
            })
            .filter(|(r, _)| *r > 0.0)
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        scored.into_iter().take(limit).map(|(_, s)| s).collect()
    }

    fn add(&self, skill: Skill) -> anyhow::Result<()> {
        // Create a directory for the skill
        let skill_dir = self.skills_dir.join(safe_skill_dir_name(&skill.name)?);
        std::fs::create_dir_all(&skill_dir)?;

        let skill_file = skill_dir.join("SKILL.md");
        let content = skill.to_markdown();
        std::fs::write(&skill_file, content)?;

        // Also store in memory if available
        if let Some(mem) = &self.memory {
            let _ = mem.upsert_doc(crate::memory::WorkingDoc {
                id: format!("skill-{}", skill.name),
                title: format!("Skill: {}", skill.name),
                content: skill.body.clone(),
                updated_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            });
        }

        Ok(())
    }

    fn all(&self) -> Vec<Skill> {
        self.scan()
    }

    fn curate(&self) -> anyhow::Result<()> {
        let curator = Curator::new();
        curator.curate(&self.skills_dir)
    }

    fn prune(&self, min_score: f64) -> anyhow::Result<usize> {
        let skills = self.scan();
        let mut removed = 0;

        for skill in &skills {
            if skill.score < min_score && skill.auto_generated {
                let skill_dir = self.skills_dir.join(safe_skill_dir_name(&skill.name)?);
                if skill_dir.exists() {
                    std::fs::remove_dir_all(&skill_dir)?;
                    removed += 1;
                }
            }
        }

        Ok(removed)
    }

    fn get(&self, name: &str) -> Option<Skill> {
        self.scan().into_iter().find(|s| s.name == name)
    }

    fn invoke(&self, name: &str) -> anyhow::Result<Option<SkillInvocation>> {
        let Some(skill) = self.get(name) else {
            return Ok(None);
        };
        let base = if skill.source_file.ends_with(".skill.md") {
            self.skills_dir.clone()
        } else {
            self.skills_dir.join(&skill.source_file)
        };
        let load_files = |files: &[String]| -> Vec<(String, String)> {
            let mut loaded = Vec::new();
            for f in files {
                let Ok(candidate) = safe_relative_path(&base, f) else {
                    continue;
                };
                if let Ok(content) = std::fs::read_to_string(&candidate) {
                    loaded.push((f.clone(), content));
                }
            }
            loaded
        };
        Ok(Some(SkillInvocation {
            loaded_references: load_files(&skill.references),
            loaded_templates: load_files(&skill.templates),
            loaded_scripts: load_files(&skill.scripts),
            loaded_assets: load_files(&skill.assets),
            skill,
        }))
    }

    fn remove(&self, name: &str) -> anyhow::Result<bool> {
        // Skills live under a directory equal to their name. Remove it.
        let skill_dir = self.skills_dir.join(safe_skill_dir_name(name)?);
        let existed = skill_dir.exists();
        if existed {
            std::fs::remove_dir_all(&skill_dir)?;
        }
        Ok(existed)
    }
}

fn safe_skill_dir_name(name: &str) -> anyhow::Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty()
        || trimmed.contains("..")
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains(':')
    {
        anyhow::bail!("invalid skill name '{}'", name);
    }
    Ok(trimmed.to_string())
}

fn safe_relative_path(base: &Path, relative: &str) -> anyhow::Result<PathBuf> {
    let rel = Path::new(relative);
    if rel.is_absolute()
        || rel
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        anyhow::bail!("skill reference escapes skill directory: {}", relative);
    }
    let candidate = base.join(rel);
    let canonical_base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    let canonical_candidate = candidate
        .canonicalize()
        .unwrap_or_else(|_| candidate.to_path_buf());
    if !canonical_candidate.starts_with(&canonical_base) || !canonical_candidate.exists() {
        anyhow::bail!("skill reference outside base or missing: {}", relative);
    }
    Ok(canonical_candidate)
}

// ─── Curator: self-improvement loop ─────────────────────────────────────────────

pub struct Curator {
    min_score: f64,
    max_skills: usize,
}

impl Curator {
    pub fn new() -> Self {
        Self {
            min_score: 0.2,
            max_skills: 100,
        }
    }

    /// Grade, consolidate, dedupe, and prune the skill library.
    /// This is the closed learning loop — skills are created from experience
    /// and improved during use.
    pub fn curate(&self, skills_dir: &Path) -> anyhow::Result<()> {
        let library = FsSkillLibrary::new(skills_dir.to_path_buf());
        let mut skills = library.all();

        if skills.is_empty() {
            return Ok(());
        }

        // 0. Purge poisoned skills (G1): any skill whose description or body
        //    carries UI-status leakage or a user complaint is deleted from
        //    disk — it must never be re-injected into a system prompt.
        skills.retain(|s| {
            let poisoned = is_unfit_for_skill(&s.description) || is_unfit_for_skill(&s.body);
            if poisoned {
                if let Ok(dir_name) = safe_skill_dir_name(&s.name) {
                    let _ = std::fs::remove_dir_all(skills_dir.join(dir_name));
                }
                tracing::warn!("Curator: purged poisoned skill `{}`", s.name);
            }
            !poisoned
        });

        if skills.is_empty() {
            return Ok(());
        }

        // 1. Grade: score each skill based on usage and content quality
        for skill in &mut skills {
            // More usage = higher score
            skill.score += skill.usage_count as f64 * 0.05;
            // Longer body = more detailed = slightly higher score
            skill.score += (skill.body.len() as f64 / 5000.0).min(0.1);
            // Cap at 1.0
            skill.score = skill.score.min(1.0);
        }

        // 2. Dedupe: merge skills with similar names or overlapping triggers
        let mut merged = Vec::new();
        let mut merged_indices = std::collections::HashSet::new();

        for i in 0..skills.len() {
            if merged_indices.contains(&i) {
                continue;
            }
            let mut current = skills[i].clone();

            for j in (i + 1)..skills.len() {
                if merged_indices.contains(&j) {
                    continue;
                }
                // Check similarity: same first 3 chars of name, or >50% trigger overlap
                let name_overlap = current.name
                    [..current.name.len().min(3).min(skills[j].name.len())]
                    == skills[j].name[..skills[j].name.len().min(3).min(current.name.len())];

                let trigger_overlap = {
                    let a: std::collections::HashSet<_> = current.trigger.iter().cloned().collect();
                    let b: std::collections::HashSet<_> =
                        skills[j].trigger.iter().cloned().collect();
                    let intersection = a.intersection(&b).count();
                    let union = a.union(&b).count();
                    if union == 0 {
                        false
                    } else {
                        intersection as f64 / union as f64 > 0.5
                    }
                };

                if name_overlap || trigger_overlap {
                    // Merge: combine bodies, take higher score
                    current.body = format!("{}\n\n---\n\n{}", current.body, skills[j].body);
                    current.score = current.score.max(skills[j].score);
                    current.trigger.extend(skills[j].trigger.clone());
                    current.trigger.sort();
                    current.trigger.dedup();
                    merged_indices.insert(j);
                }
            }
            merged.push(current);
        }

        // 3. Prune: remove low-score auto-generated skills, keep total under max
        merged.retain(|s| !s.auto_generated || s.score >= self.min_score);
        merged.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        if merged.len() > self.max_skills {
            merged.truncate(self.max_skills);
        }

        // 4. Write back updated SKILL.md files without deleting references,
        // templates, scripts or assets stored beside them.
        for skill in &merged {
            let skill_dir = skills_dir.join(safe_skill_dir_name(&skill.name)?);
            std::fs::create_dir_all(&skill_dir)?;
            std::fs::write(skill_dir.join("SKILL.md"), skill.to_markdown())?;
        }

        tracing::info!(
            "Curator: {} skills before → {} after (deduped {}, pruned {})",
            skills.len(),
            merged.len(),
            skills.len() - merged_indices.len(),
            skills.len() + merged_indices.len() - merged.len() - skills.len().min(merged.len()),
        );

        Ok(())
    }

    /// Generate a skill candidate from a successful run trajectory.
    /// Called by the engine after a `RunFinished` event.
    pub fn propose_skill(run_description: &str, outcome: &str) -> Option<Skill> {
        let words: Vec<&str> = run_description.split_whitespace().collect();
        let lower = run_description.to_lowercase();
        let outcome_lower = outcome.to_lowercase();

        if words.len() < 5 || outcome_lower.contains("error") {
            return None;
        }

        // G1: never learn from UI-status leakage or a user complaint. The
        // poisoned `code-review` skill on disk was born from a task description
        // polluted with cockpit status text ("coder ◌ consulting … parsing
        // request…") and an outcome carrying "✓ coder completed · 4487↑ 150↓
        // tok" plus a frustrated correction. None of that is a reusable skill.
        if is_unfit_for_skill(run_description) || is_unfit_for_skill(outcome) {
            return None;
        }

        let specificity_markers = [
            "github.com",
            "http",
            "https",
            "this ",
            "that ",
            "the file",
            "my ",
            "your ",
            "2024",
            "2025",
            "2026",
        ];
        if specificity_markers
            .iter()
            .any(|marker| lower.contains(marker))
        {
            return None;
        }
        if words.iter().any(|word| {
            let cleaned = word.trim_matches(|c: char| !c.is_alphanumeric());
            // Only bail on long proper-noun-looking tokens (> 12 chars, starts uppercase).
            // Shorter capitalized words (structs, types) are normal in coding tasks.
            cleaned
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
                && cleaned.chars().count() > 12
        }) {
            return None;
        }

        let has_concrete_output = [
            "diff", "fn ", "struct ", "impl ", "test", "fixed", "refactor", "added", "updated",
            "created", "modified", "patch", "write", "edit", "return", "async", "pub ", "let ",
            "const ", "mod ",
        ]
        .iter()
        .any(|needle| outcome_lower.contains(needle));
        if !has_concrete_output {
            return None;
        }

        let name = skill_name_from_pattern(run_description)?.to_string();
        let triggers = skill_triggers_for_pattern(&name);

        Some(Skill {
            name,
            description: format!("Reusable pattern learned from: {}", run_description),
            trigger: triggers,
            body: format!(
                "## Context\nTask: {}\n\n## Approach\n{}",
                run_description, outcome
            ),
            source_file: String::new(),
            usage_count: 0,
            created_at: chrono::Utc::now().format("%Y-%m-%d").to_string(),
            score: 0.3,
            auto_generated: true,
            references: Vec::new(),
            templates: Vec::new(),
            scripts: Vec::new(),
            assets: Vec::new(),
            manifest_version: None,
            allowed_tools: Vec::new(),
        })
    }

    pub fn propose_skill_if_missing(
        run_description: &str,
        outcome: &str,
        library: &dyn SkillLibrary,
    ) -> Option<Skill> {
        let candidate = Self::propose_skill(run_description, outcome)?;
        if library.get(&candidate.name).is_some() {
            None
        } else {
            Some(candidate)
        }
    }
}

/// G1: signatures of text that must NEVER become a skill — cockpit/UI status
/// leakage and user complaints/corrections. Used to reject both the task
/// description and the outcome before a skill is proposed, and to recognise
/// already-poisoned skills on disk for purging.
pub fn is_unfit_for_skill(text: &str) -> bool {
    let lower = text.to_lowercase();
    // UI / cockpit status artifacts that should never reach the curator.
    const UI_ARTIFACTS: &[&str] = &[
        "◌",
        "consulting ",
        "parsing request",
        "↑",
        "↓",
        "completed ·",
        "route set",
        "reusable pattern learned from",
        "metrics captured",
        ".claude/worktrees",
    ];
    if UI_ARTIFACTS.iter().any(|m| lower.contains(m)) {
        return true;
    }
    // Complaint / correction markers — the user telling Sparrow it failed.
    const COMPLAINT_MARKERS: &[&str] = &[
        "tu as vraiment un problème",
        "regarde ce que tu m'as",
        "n'importe quoi",
        "ça marche pas",
        "ne marche pas",
        "you have a problem",
        "this is broken",
        "that's wrong",
    ];
    if COMPLAINT_MARKERS.iter().any(|m| lower.contains(m)) {
        return true;
    }
    false
}

pub fn skill_name_from_pattern(description: &str) -> Option<&'static str> {
    let d = description.to_lowercase();
    if d.contains("test") && (d.contains("add") || d.contains("write") || d.contains("fix")) {
        return Some("write-and-fix-tests");
    }
    if d.contains("refactor") || d.contains("rename") || d.contains("extract") {
        return Some("refactor-safely");
    }
    if d.contains("debug") || d.contains("error") || d.contains("panic") || d.contains("crash") {
        return Some("debug-systematically");
    }
    if d.contains("document")
        || d.contains("comment")
        || d.contains("readme")
        || d.contains("docstring")
    {
        return Some("write-docs");
    }
    if d.contains("secur") || d.contains("vulnerab") || d.contains("audit") {
        return Some("security-audit");
    }
    if d.contains("performance") || d.contains("slow") || d.contains("optim") || d.contains("bench")
    {
        return Some("performance-profile");
    }
    if d.contains("upgrade") || d.contains("bump") || d.contains("depend") || d.contains("package")
    {
        return Some("upgrade-dependencies");
    }
    if d.contains("review") || d.contains("pr") || d.contains("pull request") || d.contains("diff")
    {
        return Some("code-review");
    }
    if d.contains("git") || d.contains("commit") || d.contains("branch") || d.contains("merge") {
        return Some("git-workflow");
    }
    None
}

fn skill_triggers_for_pattern(name: &str) -> Vec<String> {
    match name {
        "write-and-fix-tests" => vec!["test", "unit", "fix", "assert"],
        "refactor-safely" => vec!["refactor", "rename", "extract", "safe"],
        "debug-systematically" => vec!["debug", "error", "panic", "crash"],
        "write-docs" => vec!["document", "readme", "comment", "docstring"],
        "security-audit" => vec!["security", "audit", "vulnerability", "safe"],
        "performance-profile" => vec!["performance", "slow", "optimize", "bench"],
        "upgrade-dependencies" => vec!["upgrade", "bump", "dependency", "package"],
        "code-review" => vec!["review", "pr", "diff", "pull-request"],
        "git-workflow" => vec!["git", "commit", "branch", "merge"],
        _ => vec!["skill"],
    }
    .into_iter()
    .map(String::from)
    .collect()
}

impl Default for Curator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "sparrow-tier2-{name}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn skill_invocation_rejects_parent_dir_references() {
        let root = temp_dir("skill-ref-escape");
        std::fs::create_dir_all(root.join("review").join("references")).unwrap();
        std::fs::write(
            root.join("review").join("SKILL.md"),
            "# Skill: review\n\n**Trigger:** review\n\n**References:** ../secret.txt, references/checklist.md\n\n## Body\nReview carefully.",
        )
        .unwrap();
        std::fs::write(
            root.join("review").join("references").join("checklist.md"),
            "ok",
        )
        .unwrap();
        std::fs::write(root.join("secret.txt"), "nope").unwrap();

        let lib = FsSkillLibrary::new(root.clone());
        let invocation = lib.invoke("review").unwrap().expect("skill should exist");

        assert_eq!(invocation.loaded_references.len(), 1);
        assert_eq!(invocation.loaded_references[0].0, "references/checklist.md");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn curator_preserves_skill_assets_and_updates_skill_md_only() {
        let root = temp_dir("curator-assets");
        let skill_dir = root.join("refactor-safely");
        std::fs::create_dir_all(skill_dir.join("references")).unwrap();
        std::fs::write(skill_dir.join("references").join("checklist.md"), "keep me").unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "# Skill: refactor-safely\n\n**Trigger:** refactor, rename\n\n**References:** references/checklist.md\n\n## Body\nMove in small steps.",
        )
        .unwrap();

        Curator::new().curate(&root).unwrap();

        assert!(
            skill_dir.join("references").join("checklist.md").exists(),
            "curator must not delete progressive-disclosure assets"
        );
        let lib = FsSkillLibrary::new(root.clone());
        let invocation = lib
            .invoke("refactor-safely")
            .unwrap()
            .expect("skill should remain");
        assert_eq!(invocation.loaded_references[0].1, "keep me");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn curator_purges_poisoned_skill_from_disk() {
        // G1: a skill whose description is a complaint laced with UI status
        // text must be deleted on the next curate pass.
        let root = temp_dir("curator-purge");
        let toxic = root.join("code-review");
        std::fs::create_dir_all(&toxic).unwrap();
        std::fs::write(
            toxic.join("SKILL.md"),
            "# Skill: code-review\n\n**Trigger:** review, pr, diff\n\n**Description:** Reusable pattern learned from: non tu as vraiment un problème regarde ce que tu m'as écris : coder ◌ consulting deepseek-v4-pro\n\n## Body\n✓ coder completed · 4487↑ 150↓ tok",
        )
        .unwrap();
        // A legitimate skill must survive.
        let good = root.join("refactor-safely");
        std::fs::create_dir_all(&good).unwrap();
        std::fs::write(
            good.join("SKILL.md"),
            "# Skill: refactor-safely\n\n**Trigger:** refactor\n\n**Description:** Move code in small verified steps.\n\n## Body\nExtract, compile, test, repeat.",
        )
        .unwrap();

        Curator::new().curate(&root).unwrap();

        assert!(!toxic.exists(), "poisoned skill dir must be removed");
        assert!(good.exists(), "legitimate skill must survive");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn propose_skill_rejects_ui_status_and_complaints() {
        // The exact polluted inputs that produced the poisoned skill.
        assert!(
            Curator::propose_skill(
                "non tu as vraiment un problème regarde ce que tu m'as écris coder",
                "✓ coder completed · 4487↑ 150↓ tok",
            )
            .is_none()
        );
        assert!(is_unfit_for_skill(
            "coder ◌ consulting deepseek-v4-pro · parsing request…"
        ));
        assert!(is_unfit_for_skill(
            "## Approach\n.claude/worktrees/tmp/src/main.rs"
        ));
        assert!(!is_unfit_for_skill(
            "Refactor the auth module by extracting the token parser into its own function."
        ));
    }

    #[test]
    fn skill_names_cannot_escape_skill_root() {
        let root = temp_dir("skill-name-escape");
        let lib = FsSkillLibrary::new(root.clone());
        let skill = Skill {
            name: "../outside".into(),
            description: "bad".into(),
            trigger: vec!["bad".into()],
            body: "bad".into(),
            source_file: String::new(),
            usage_count: 0,
            created_at: String::new(),
            score: 0.5,
            auto_generated: false,
            references: Vec::new(),
            templates: Vec::new(),
            scripts: Vec::new(),
            assets: Vec::new(),
            manifest_version: None,
            allowed_tools: Vec::new(),
        };

        assert!(lib.add(skill).is_err());
        assert!(!root.join("..").join("outside").exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn skill_manifest_restricts_tool_specs() {
        let root = temp_dir("skill-manifest-tools");
        let skill_dir = root.join("read-only-review");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "# Skill: read-only-review\n\n**Trigger:** review\n\n## Body\nInspect only.",
        )
        .unwrap();
        std::fs::write(
            skill_dir.join("manifest.toml"),
            "version = \"2\"\nallowed_tools = [\"fs_read\"]\n",
        )
        .unwrap();

        let lib = FsSkillLibrary::new(root.clone());
        let invocation = lib
            .invoke("read-only-review")
            .unwrap()
            .expect("skill should load");
        assert_eq!(invocation.skill.manifest_version.as_deref(), Some("2"));
        assert_eq!(invocation.skill.allowed_tools, vec!["fs_read"]);

        let mut registry = crate::tools::ToolRegistry::new();
        registry.register(std::sync::Arc::new(crate::tools::fs::FsRead));
        registry.register(std::sync::Arc::new(crate::tools::fs::FsWrite));
        let specs = registry.to_specs_for_skill(&invocation.skill.allowed_tools);
        let names: Vec<_> = specs.into_iter().map(|s| s.name).collect();
        assert_eq!(names, vec!["fs_read"]);

        let _ = std::fs::remove_dir_all(root);
    }
}
