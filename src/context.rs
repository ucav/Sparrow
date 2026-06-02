//! Context meter + handoff doc generator (Phase 12).
//!
//! Sparrow needs a reliable view of "what is in my context right now" so that
//! `/compact` and the `PreCompact` hook fire at the right moment, and so the
//! UI can render a meter. This module is intentionally pure: no I/O at the
//! type-level, no provider calls. Tests cover the math.

use serde::{Deserialize, Serialize};

/// Conservative chars-per-token ratio (matches `ContextManager`).
pub const TOKENS_PER_CHAR: f64 = 0.25;

/// Character counts per category of input that contributes to the model
/// context window. All five categories are tracked separately so a UI can show
/// where the budget is going.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextMeter {
    pub prompt_chars: usize,
    pub memory_chars: usize,
    pub tools_chars: usize,
    pub attachments_chars: usize,
    pub transcript_chars: usize,
    /// The model's hard context limit, in tokens. Used by `usage_ratio`.
    pub max_tokens: u64,
}

impl ContextMeter {
    pub fn new(max_tokens: u64) -> Self {
        Self {
            max_tokens,
            ..Default::default()
        }
    }

    pub fn total_chars(&self) -> usize {
        self.prompt_chars
            + self.memory_chars
            + self.tools_chars
            + self.attachments_chars
            + self.transcript_chars
    }

    pub fn estimated_tokens(&self) -> u64 {
        (self.total_chars() as f64 * TOKENS_PER_CHAR) as u64
    }

    /// Fraction of the budget consumed, in [0.0, +inf). `> 1.0` means the
    /// estimate already exceeds the limit.
    pub fn usage_ratio(&self) -> f64 {
        if self.max_tokens == 0 {
            return 0.0;
        }
        self.estimated_tokens() as f64 / self.max_tokens as f64
    }

    /// True if compaction should be triggered. `reserve_tokens` is how much
    /// headroom callers want to keep for the next model response.
    pub fn should_compact(&self, reserve_tokens: u64) -> bool {
        self.estimated_tokens() + reserve_tokens > self.max_tokens
    }

    /// A human-readable one-liner, e.g. `42% (prompt 1.2k · transcript 5.3k …)`.
    pub fn summary(&self) -> String {
        format!(
            "ctx {:.0}% · prompt {} · memory {} · tools {} · attach {} · transcript {} ({}t / {}t)",
            self.usage_ratio() * 100.0,
            self.prompt_chars,
            self.memory_chars,
            self.tools_chars,
            self.attachments_chars,
            self.transcript_chars,
            self.estimated_tokens(),
            self.max_tokens
        )
    }
}

/// A durable handoff document captured at compaction time. It is what makes
/// the next IA (or the same one on resume) productive without rereading the
/// whole transcript.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HandoffDoc {
    pub created_at: String,
    pub task: String,
    pub files_modified: Vec<String>,
    pub decisions: Vec<String>,
    pub tests_run: Vec<String>,
    pub blockers: Vec<String>,
    pub next_steps: Vec<String>,
    /// The compacted summary line from the context manager.
    pub context_summary: String,
}

impl HandoffDoc {
    pub fn new(task: impl Into<String>) -> Self {
        Self {
            created_at: chrono::Utc::now().to_rfc3339(),
            task: task.into(),
            ..Default::default()
        }
    }

    pub fn with_context(mut self, meter: &ContextMeter) -> Self {
        self.context_summary = meter.summary();
        self
    }

    /// Render as Markdown. Stable shape so workflows/tests can grep.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "# Sparrow Handoff\n\nCreated: {}\n\n",
            self.created_at
        ));
        out.push_str(&format!("## Task\n\n{}\n\n", self.task));
        section(&mut out, "Files modified", &self.files_modified);
        section(&mut out, "Decisions", &self.decisions);
        section(&mut out, "Tests run", &self.tests_run);
        section(&mut out, "Blockers", &self.blockers);
        section(&mut out, "Next steps", &self.next_steps);
        if !self.context_summary.is_empty() {
            out.push_str(&format!("## Context\n\n{}\n", self.context_summary));
        }
        out
    }
}

fn section(out: &mut String, title: &str, items: &[String]) {
    out.push_str(&format!("## {}\n\n", title));
    if items.is_empty() {
        out.push_str("_none_\n\n");
    } else {
        for item in items {
            out.push_str(&format!("- {}\n", item));
        }
        out.push('\n');
    }
}

/// Best-effort distillation of a transcript into decision/file/blocker lines.
/// Pure function — caller owns the messages slice. Used by `sparrow compact`.
pub fn distill_transcript(messages: &[String]) -> HandoffDoc {
    let mut files: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut decisions: Vec<String> = Vec::new();
    let mut tests: Vec<String> = Vec::new();
    let mut blockers: Vec<String> = Vec::new();

    for msg in messages {
        for word in msg.split_whitespace() {
            // Path-like tokens with known source extensions. Strip trailing
            // punctuation so "src/foo.rs." or "src/foo.rs," still matches.
            let cleaned = word.trim_end_matches(|c: char| matches!(c, ',' | '.' | ';' | ':' | ')' | ']'));
            if has_source_ext(cleaned) {
                files.insert(cleaned.to_string());
            }
        }
        for line in msg.lines() {
            let trimmed = line.trim();
            let lower = trimmed.to_lowercase();
            if lower.starts_with("decision:")
                || lower.starts_with("- decision:")
                || lower.starts_with("* decision:")
            {
                decisions.push(trimmed.to_string());
            } else if lower.contains("cargo test")
                || lower.contains("npm test")
                || lower.contains("pytest")
            {
                tests.push(trimmed.to_string());
            } else if lower.contains("blocker:") || lower.contains("blocked by") {
                blockers.push(trimmed.to_string());
            }
        }
    }

    HandoffDoc {
        created_at: chrono::Utc::now().to_rfc3339(),
        task: String::new(),
        files_modified: files.into_iter().collect(),
        decisions,
        tests_run: tests,
        blockers,
        next_steps: Vec::new(),
        context_summary: String::new(),
    }
}

fn has_source_ext(s: &str) -> bool {
    matches!(
        std::path::Path::new(s).extension().and_then(|e| e.to_str()),
        Some(
            "rs" | "toml"
                | "md"
                | "py"
                | "js"
                | "ts"
                | "tsx"
                | "jsx"
                | "go"
                | "java"
                | "c"
                | "cpp"
                | "h"
                | "hpp"
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meter_tracks_categories_separately() {
        let mut m = ContextMeter::new(4000);
        m.prompt_chars = 400;
        m.memory_chars = 200;
        m.tools_chars = 100;
        m.attachments_chars = 50;
        m.transcript_chars = 250;
        assert_eq!(m.total_chars(), 1000);
        // 1000 chars * 0.25 = 250 tokens; 250 / 4000 = 0.0625
        assert!((m.usage_ratio() - 0.0625).abs() < 1e-6);
        assert!(!m.should_compact(100));
    }

    #[test]
    fn should_compact_when_estimate_plus_reserve_exceeds_limit() {
        let mut m = ContextMeter::new(100);
        m.transcript_chars = 380; // 95 tokens
        assert!(!m.should_compact(0));
        assert!(m.should_compact(10));
    }

    #[test]
    fn handoff_markdown_has_stable_shape() {
        let mut doc = HandoffDoc::new("fix the auth bug");
        doc.files_modified = vec!["src/auth.rs".into()];
        doc.decisions = vec!["decision: roll back token rotation".into()];
        doc.tests_run = vec!["cargo test --test auth".into()];
        doc.next_steps = vec!["land the PR".into()];
        let md = doc.to_markdown();
        assert!(md.contains("# Sparrow Handoff"));
        assert!(md.contains("## Task"));
        assert!(md.contains("fix the auth bug"));
        assert!(md.contains("## Files modified"));
        assert!(md.contains("src/auth.rs"));
        assert!(md.contains("## Decisions"));
        assert!(md.contains("## Tests run"));
        assert!(md.contains("## Blockers"));
        assert!(md.contains("_none_")); // blockers empty
        assert!(md.contains("## Next steps"));
        assert!(md.contains("land the PR"));
    }

    #[test]
    fn distill_pulls_files_decisions_tests_blockers() {
        let msgs = vec![
            "Touched src/auth.rs and src/router/mod.rs. Updated docs/cli-reference.md.".into(),
            "Decision: rollback token rotation for now".into(),
            "Ran cargo test --test integration".into(),
            "Blocker: needs DB migration".into(),
        ];
        let doc = distill_transcript(&msgs);
        assert!(doc.files_modified.iter().any(|f| f == "src/auth.rs"));
        assert!(doc.files_modified.iter().any(|f| f == "src/router/mod.rs"));
        assert!(
            doc.decisions
                .iter()
                .any(|d| d.to_lowercase().contains("rollback"))
        );
        assert!(doc.tests_run.iter().any(|t| t.contains("cargo test")));
        assert!(
            doc.blockers
                .iter()
                .any(|b| b.to_lowercase().contains("blocker"))
        );
    }
}
