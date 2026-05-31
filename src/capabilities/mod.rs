use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::memory::Memory;

pub mod mcp;

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
        })
    }

    /// Convert a Skill back to SKILL.md markdown format
    pub fn to_markdown(&self) -> String {
        format!(
            "# Skill: {name}\n\n\
             **Trigger:** {trigger}\n\n\
             **Description:** {desc}\n\n\
             ## Body\n\
             {body}\n",
            name = self.name,
            trigger = self.trigger.join(", "),
            desc = self.description,
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

// ─── THE SKILL LIBRARY TRAIT ────────────────────────────────────────────────────

pub trait SkillLibrary: Send + Sync {
    fn relevant(&self, ctx: &str, limit: usize) -> Vec<Skill>;
    fn add(&self, skill: Skill) -> anyhow::Result<()>;
    fn all(&self) -> Vec<Skill>;
    fn curate(&self) -> anyhow::Result<()>;
    fn prune(&self, min_score: f64) -> anyhow::Result<usize>;
    fn get(&self, name: &str) -> Option<Skill>;
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
                            if let Some(skill) =
                                Skill::from_markdown(&content, &rel)
                            {
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

        scored
            .into_iter()
            .take(limit)
            .map(|(_, s)| s)
            .collect()
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
                let name_overlap = current.name[..current.name.len().min(3).min(skills[j].name.len())]
                    == skills[j].name[..skills[j].name.len().min(3).min(current.name.len())];

                let trigger_overlap = {
                    let a: std::collections::HashSet<_> =
                        current.trigger.iter().cloned().collect();
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
                    current.body =
                        format!("{}\n\n---\n\n{}", current.body, skills[j].body);
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
        merged.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
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
        // Simple heuristic: if the run was successful and the description
        // is specific enough, propose a skill.
        let words: Vec<&str> = run_description.split_whitespace().collect();
        if words.len() < 5 || outcome.contains("error") {
            return None;
        }

        // Extract key terms as triggers
        let triggers: Vec<String> = words
            .iter()
            .filter(|w| w.len() > 3)
            .take(5)
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())
            .collect();

        let name = words
            .iter()
            .take(4)
            .map(|w| {
                let mut c = w.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join("");

        Some(Skill {
            name,
            description: format!("Auto-generated from: {}", run_description),
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
        })
    }
}

impl Default for Curator {
    fn default() -> Self {
        Self::new()
    }
}
