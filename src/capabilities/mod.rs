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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInvocation {
    pub skill: Skill,
    pub loaded_references: Vec<(String, String)>,
    pub loaded_templates: Vec<(String, String)>,
    pub loaded_scripts: Vec<(String, String)>,
    pub loaded_assets: Vec<(String, String)>,
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
             ## Body\n\
             {body}\n",
            name = self.name,
            trigger = self.trigger.join(", "),
            desc = self.description,
            references = self.references.join(", "),
            templates = self.templates.join(", "),
            scripts = self.scripts.join(", "),
            assets = self.assets.join(", "),
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

    /// Scan the skills directory and load all SKILL.md files
    fn scan(&self) -> Vec<Skill> {
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
                            if let Some(skill) = Skill::from_markdown(&content, &rel) {
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
                        if let Some(skill) = Skill::from_markdown(&content, &rel) {
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
        let skill_dir = self.skills_dir.join(&skill.name);
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
                let skill_dir = self.skills_dir.join(&skill.name);
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
                let candidate = base.join(f);
                if !candidate.starts_with(&base) || !candidate.exists() {
                    continue;
                }
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
        let skill_dir = self.skills_dir.join(name);
        let existed = skill_dir.exists();
        if existed {
            std::fs::remove_dir_all(&skill_dir)?;
        }
        Ok(existed)
    }
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

        // 4. Write back updated skills
        // Clear directory first
        if skills_dir.exists() {
            for entry in std::fs::read_dir(skills_dir)? {
                let entry = entry?;
                if entry.path().is_dir() {
                    std::fs::remove_dir_all(entry.path())?;
                }
            }
        }

        for skill in &merged {
            library.add(skill.clone())?;
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

        let specificity_markers = [
            "github.com",
            "repo",
            "http",
            "https",
            ".rs",
            ".py",
            ".js",
            ".ts",
            "this ",
            "that ",
            "the file",
            "my ",
            "your ",
            "2024",
            "2025",
            "2026",
            "v0.",
            "v1.",
            "v2.",
        ];
        if specificity_markers
            .iter()
            .any(|marker| lower.contains(marker))
        {
            return None;
        }
        if words.iter().any(|word| {
            let cleaned = word.trim_matches(|c: char| !c.is_alphanumeric());
            cleaned
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
                && cleaned.chars().count() > 8
        }) {
            return None;
        }

        let has_concrete_output = [
            "diff", "fn ", "struct ", "impl ", "test", "fixed", "refactor",
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
