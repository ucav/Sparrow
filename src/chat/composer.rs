//! Input composer for the Sparrow chat.
//!
//! Manages multi-line input with history navigation and basic editing.

use std::io::{self, Write};

/// A simple input composer that supports multi-line input,
/// history navigation, and basic slash-command completion.
pub struct InputComposer {
    /// Command history (most recent first)
    history: Vec<String>,
    /// Current history position (None = not navigating)
    history_pos: Option<usize>,
    /// Known slash commands for auto-completion
    commands: Vec<String>,
    /// Current input buffer (lines) — reserved for future TUI-driven editing.
    #[allow(dead_code)]
    buffer: Vec<String>,
    /// Current cursor position (row, col) — reserved for future TUI-driven editing.
    #[allow(dead_code)]
    cursor_row: usize,
    #[allow(dead_code)]
    cursor_col: usize,
    /// Prompt string
    prompt: String,
    /// Continuation prompt for multi-line
    cont_prompt: String,
}

impl InputComposer {
    /// Create a new input composer.
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            history_pos: None,
            commands: vec![
                "/help".into(),
                "/plan".into(),
                "/run".into(),
                "/chat".into(),
                "/clear".into(),
                "/history".into(),
                "/save".into(),
                "/load".into(),
                "/exit".into(),
                "/quit".into(),
            ],
            buffer: vec![String::new()],
            cursor_row: 0,
            cursor_col: 0,
            prompt: "> ".into(),
            cont_prompt: ". ".into(),
        }
    }

    /// Add commands to the autocomplete list.
    pub fn add_commands(&mut self, cmds: &[&str]) {
        for cmd in cmds {
            self.commands.push(cmd.to_string());
        }
    }

    /// Add a line to history.
    pub fn add_history(&mut self, line: &str) {
        if !line.trim().is_empty() {
            self.history.insert(0, line.to_string());
            if self.history.len() > 1000 {
                self.history.pop();
            }
        }
        self.history_pos = None;
    }

    /// Read a complete input (terminated by empty line or Ctrl+D).
    pub fn read_input(&mut self) -> anyhow::Result<String> {
        let mut lines: Vec<String> = Vec::new();
        let mut first = true;

        loop {
            let prompt = if first {
                &self.prompt
            } else {
                &self.cont_prompt
            };
            print!("{prompt}");
            io::stdout().flush()?;

            let mut line = String::new();
            let bytes = io::stdin().read_line(&mut line)?;

            if bytes == 0 {
                // EOF (Ctrl+D)
                break;
            }

            let trimmed = line.trim_end_matches('\n').trim_end_matches('\r');

            if first && trimmed.is_empty() {
                // Empty first line = cancel
                return Ok(String::new());
            }

            if !first && trimmed.is_empty() {
                // Empty continuation line = submit
                break;
            }

            let text = trimmed.to_string();
            if first {
                self.add_history(&text);
            }
            lines.push(text);
            first = false;
        }

        Ok(lines.join("\n"))
    }

    /// Auto-complete a partial slash command.
    pub fn autocomplete(&self, partial: &str) -> Vec<String> {
        if !partial.starts_with('/') {
            return Vec::new();
        }

        let lower = partial.to_lowercase();
        self.commands
            .iter()
            .filter(|cmd| cmd.to_lowercase().starts_with(&lower))
            .cloned()
            .collect()
    }

    /// Navigate history up (older entry).
    pub fn history_up(&mut self) -> Option<&str> {
        if self.history.is_empty() {
            return None;
        }
        let pos = self
            .history_pos
            .map_or(0, |p| (p + 1).min(self.history.len() - 1));
        self.history_pos = Some(pos);
        Some(&self.history[pos])
    }

    /// Navigate history down (newer entry).
    pub fn history_down(&mut self) -> Option<&str> {
        match self.history_pos {
            Some(0) | None => {
                self.history_pos = None;
                None
            }
            Some(p) => {
                self.history_pos = Some(p - 1);
                Some(&self.history[p - 1])
            }
        }
    }

    /// Get the command history.
    pub fn get_history(&self) -> &[String] {
        &self.history
    }
}

impl Default for InputComposer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autocomplete() {
        let composer = InputComposer::new();
        let matches = composer.autocomplete("/he");
        assert!(matches.contains(&"/help".to_string()));
    }

    #[test]
    fn test_history() {
        let mut composer = InputComposer::new();
        composer.add_history("hello");
        composer.add_history("world");

        // Newest first (history is stored most-recent-first).
        let up = composer.history_up();
        assert_eq!(up, Some("world"));

        // Going further back returns the older entry.
        let up2 = composer.history_up();
        assert_eq!(up2, Some("hello"));

        // Going back down returns the newer entry again.
        let down = composer.history_down();
        assert_eq!(down, Some("world"));

        // Past the newest, navigation resets.
        let none = composer.history_down();
        assert_eq!(none, None);
    }
}
