use crate::event::Event;
use std::collections::HashSet;

// ─── Redaction filter ───────────────────────────────────────────────────────────

/// Filters secrets from events, transcripts, logs, and model context.
/// §5: "Secrets never enter transcripts, logs, or model context unless explicitly a tool argument."
pub struct RedactionFilter {
    /// Known secrets to redact (loaded from auth store)
    secrets: HashSet<String>,
    /// Patterns to redact (regex or prefix patterns)
    patterns: Vec<String>,
    /// Replacement string
    replacement: String,
}

impl RedactionFilter {
    pub fn new() -> Self {
        Self {
            secrets: HashSet::new(),
            patterns: vec![
                // Common secret patterns
                "sk-".into(),
                "sk-ant-".into(),
                "xai-".into(),
                "ghp_".into(),
                "gho_".into(),
                "ghu_".into(),
                "ghs_".into(),
                "ghr_".into(),
                "hf_".into(),
                "nvapi-".into(),
                "gsk_".into(),
                "org-".into(),
                "proj-".into(),
            ],
            replacement: "[REDACTED]".into(),
        }
    }

    /// Load secrets from the auth store
    pub fn load_secrets(&mut self, secrets: Vec<String>) {
        for s in secrets {
            if !s.is_empty() {
                self.secrets.insert(s);
            }
        }
    }

    /// Redact a string, replacing all known secrets with the replacement
    pub fn redact_str(&self, text: &str) -> String {
        let mut result = text.to_string();

        // Exact match redaction
        for secret in &self.secrets {
            if !secret.is_empty() {
                result = result.replace(secret.as_str(), &self.replacement);
            }
        }

        // Pattern-based redaction (secrets starting with known prefixes)
        for pattern in &self.patterns {
            let lower = result.to_lowercase();
            if let Some(pos) = lower.find(pattern) {
                // Find the end of the secret (until whitespace or end)
                let end = result[pos..]
                    .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
                    .map(|e| pos + e)
                    .unwrap_or(result.len());
                result.replace_range(pos..end, &self.replacement);
            }
        }

        result
    }

    /// Redact an Event, returning a new Event with secrets removed
    pub fn redact_event(&self, event: &Event) -> Event {
        let mut e = event.clone();
        match &mut e {
            Event::ThinkingDelta { text, .. } => {
                *text = self.redact_str(text);
            }
            Event::Message { text, .. } => {
                *text = self.redact_str(text);
            }
            Event::ApprovalRequested { summary, .. } => {
                *summary = self.redact_str(summary);
            }
            Event::ToolOutput { blocks, .. } => {
                for block in blocks {
                    match block {
                        crate::event::Block::Text(t) => {
                            *t = self.redact_str(t);
                        }
                        _ => {}
                    }
                }
            }
            Event::Error { message, .. } => {
                *message = self.redact_str(message);
            }
            _ => {}
        }
        e
    }

    /// Check if a string contains any secrets
    pub fn contains_secret(&self, text: &str) -> bool {
        for secret in &self.secrets {
            if !secret.is_empty() && text.contains(secret.as_str()) {
                return true;
            }
        }
        for pattern in &self.patterns {
            if text.to_lowercase().contains(pattern) {
                return true;
            }
        }
        false
    }
}

impl Default for RedactionFilter {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Context Manager ────────────────────────────────────────────────────────────

use crate::memory::RepoMap;
use crate::provider::Msg;

/// Manages context window by summarizing/compacting when approaching limits.
/// §3.7: "The Context Manager enforces the model's window via summarization/compaction
/// and a repo-map instead of dumping files."
pub struct ContextManager {
    /// Maximum context tokens before compaction
    max_tokens: u64,
    /// Approximate tokens per character (conservative)
    tokens_per_char: f64,
}

impl ContextManager {
    pub fn new(max_tokens: u64) -> Self {
        Self {
            max_tokens,
            tokens_per_char: 0.25, // ~4 chars per token
        }
    }

    /// Estimate token count for a string
    pub fn estimate_tokens(&self, text: &str) -> u64 {
        (text.len() as f64 * self.tokens_per_char) as u64
    }

    /// Check if we're approaching the context limit
    pub fn needs_compaction(&self, total_chars: usize, reserve_tokens: u64) -> bool {
        let used = self.estimate_tokens(&"x".repeat(total_chars));
        used + reserve_tokens > self.max_tokens
    }

    /// Compact messages: keep system + last N messages, summarize earlier ones
    pub fn compact_messages(
        &self,
        messages: &[Msg],
        system_prompt_len: usize,
        keep_last: usize,
    ) -> Vec<Msg> {
        let system_tokens = self.estimate_tokens(&"x".repeat(system_prompt_len));
        let available = self.max_tokens.saturating_sub(system_tokens);

        if messages.len() <= keep_last {
            return messages.to_vec();
        }

        let mut compacted = Vec::new();
        let mut used = 0u64;

        // Always keep the first user message
        if let Some(first) = messages.first() {
            compacted.push(first.clone());
            used += self.estimate_tokens(&serde_json::to_string(first).unwrap_or_default());
        }

        // Summarize middle section: extract real key topics
        let middle: Vec<&Msg> = messages[1..messages.len() - keep_last].iter().collect();
        if !middle.is_empty() {
            // Extract key topics from actual message content
            let mut tools_used = std::collections::HashSet::new();
            let mut files_mentioned = std::collections::HashSet::new();
            let mut error_count = 0u32;

            for msg in &middle {
                for block in &msg.content {
                    if let crate::provider::ContentBlock::Text { text } = block {
                        // Extract tool names
                        for tool in &[
                            "fs_read", "fs_write", "edit", "exec", "git", "search", "test",
                        ] {
                            if text.contains(tool) {
                                tools_used.insert(*tool);
                            }
                        }
                        // Extract file mentions
                        for word in text.split_whitespace() {
                            if word.ends_with(".rs")
                                || word.ends_with(".toml")
                                || word.ends_with(".md")
                                || word.ends_with(".py")
                                || word.ends_with(".js")
                                || word.ends_with(".ts")
                            {
                                files_mentioned.insert(word.to_string());
                            }
                        }
                        if text.contains("error")
                            || text.contains("Error")
                            || text.contains("FAILED")
                        {
                            error_count += 1;
                        }
                    }
                }
            }

            let mut topics = Vec::new();
            if !tools_used.is_empty() {
                let mut tools: Vec<_> = tools_used.into_iter().collect();
                tools.sort();
                topics.push(format!("tools: {}", tools.join(", ")));
            }
            if !files_mentioned.is_empty() {
                let mut files: Vec<_> = files_mentioned.into_iter().collect();
                files.sort();
                topics.push(format!("files: {}", files.join(", ")));
            }
            if error_count > 0 {
                topics.push(format!("errors encountered: {}", error_count));
            }

            let summary_str = if topics.is_empty() {
                format!("[{} messages summarized]", middle.len())
            } else {
                format!(
                    "[{} messages summarized. {}]",
                    middle.len(),
                    topics.join("; ")
                )
            };

            compacted.push(Msg {
                role: "user".into(),
                content: vec![crate::provider::ContentBlock::Text {
                    text: summary_str.clone(),
                }],
            });
            used += self.estimate_tokens(&summary_str);
        }

        // Keep last N messages
        for msg in messages.iter().rev().take(keep_last).rev() {
            let tokens = self.estimate_tokens(&serde_json::to_string(msg).unwrap_or_default());
            if used + tokens > available {
                break;
            }
            compacted.push(msg.clone());
            used += tokens;
        }

        compacted
    }

    /// Build a compact repo map representation for context
    pub fn repo_map_summary(&self, map: &RepoMap, max_files: usize) -> String {
        let mut lines = vec![format!(
            "Workspace: {} files, {} symbols",
            map.files.len(),
            map.symbols.len()
        )];

        // Show directory tree (top-level files/dirs)
        let mut dirs: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for f in &map.files {
            if let Some(first) = f.path.split('/').next() {
                dirs.insert(first.to_string());
            }
        }

        lines.push("Top-level:".into());
        for d in dirs.iter().take(max_files) {
            lines.push(format!("  {}", d));
        }

        // Show key symbols
        if !map.symbols.is_empty() {
            lines.push("Key symbols:".into());
            for s in map.symbols.iter().take(20) {
                lines.push(format!("  {} ({}) in {}", s.name, s.kind, s.file));
            }
        }

        lines.join("\n")
    }
}

impl Default for ContextManager {
    fn default() -> Self {
        Self::new(128_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_api_key() {
        let mut filter = RedactionFilter::new();
        filter.load_secrets(vec!["sk-ant-api03-abcdef123456".into()]);

        let input = "Using key sk-ant-api03-abcdef123456 for auth";
        let redacted = filter.redact_str(input);
        assert!(!redacted.contains("sk-ant-api03-abcdef123456"));
        assert!(redacted.contains("[REDACTED]"));
    }

    #[test]
    fn test_redact_event() {
        let mut filter = RedactionFilter::new();
        filter.load_secrets(vec!["mysecret123".into()]);

        let event = Event::ThinkingDelta {
            run: crate::event::RunId("test".into()),
            text: "The secret is mysecret123".into(),
        };
        let redacted = filter.redact_event(&event);
        match redacted {
            Event::ThinkingDelta { text, .. } => {
                assert!(!text.contains("mysecret123"));
                assert!(text.contains("[REDACTED]"));
            }
            _ => panic!("wrong event type"),
        }
    }

    #[test]
    fn test_context_compaction() {
        let cm = ContextManager::new(1000);
        let messages = vec![
            Msg {
                role: "user".into(),
                content: vec![],
            },
            Msg {
                role: "assistant".into(),
                content: vec![],
            },
            Msg {
                role: "user".into(),
                content: vec![],
            },
            Msg {
                role: "assistant".into(),
                content: vec![],
            },
            Msg {
                role: "user".into(),
                content: vec![],
            },
        ];
        let compacted = cm.compact_messages(&messages, 100, 2);
        // Should have: first msg + summary + last 2
        assert!(compacted.len() <= 4);
    }
}
