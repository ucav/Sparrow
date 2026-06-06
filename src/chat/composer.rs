// ─── Input composer ──────────────────────────────────────────────────────────
//
// Rich input compositing with multi-line support, history navigation,
// slash-command autocomplete, and basic syntax highlighting.

use std::io::{self, Write};

use anyhow::Result;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    style::{Color, Print, SetForegroundColor, ResetColor},
    terminal::{self, Clear, ClearType},
};

// ─── Slash commands registry ─────────────────────────────────────────────────

const SLASH_COMMANDS: &[&str] = &[
    "/help",
    "/exit",
    "/quit",
    "/clear",
    "/history",
    "/save",
    "/load",
    "/model",
    "/agent",
    "/plan",
    "/run",
    "/chat",
    "/swarm",
    "/compact",
    "/checkpoint",
    "/replay",
    "/auth",
    "/permissions",
    "/skills",
    "/tools",
    "/submit",
];

// ─── Input composer ──────────────────────────────────────────────────────────

/// Manages multi-line user input with history, autocomplete, and syntax highlighting.
pub struct InputComposer {
    /// Current input lines (multi-line support).
    lines: Vec<String>,
    /// Cursor row within lines.
    cursor_row: usize,
    /// Cursor column within lines[row].
    cursor_col: usize,
    /// Command history (oldest first).
    history: Vec<String>,
    /// Index into history when navigating (None = fresh input).
    history_idx: Option<usize>,
    /// Saved partial input when navigating history.
    saved_input: String,
    /// Slash-command autocomplete matches.
    autocomplete_matches: Vec<String>,
    /// Autocomplete match index.
    autocomplete_idx: usize,
    /// Whether autocomplete is active.
    autocomplete_active: bool,
    /// Maximum history size.
    max_history: usize,
    /// Prompt string shown before input.
    prompt: String,
    /// Whether to print the prompt on read.
    show_prompt: bool,
    /// Continuation prompt for multi-line input.
    continuation_prompt: String,
}

impl InputComposer {
    /// Create a new input composer.
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor_row: 0,
            cursor_col: 0,
            history: Vec::new(),
            history_idx: None,
            saved_input: String::new(),
            autocomplete_matches: Vec::new(),
            autocomplete_idx: 0,
            autocomplete_active: false,
            max_history: 500,
            prompt: "◆ you › ".to_string(),
            show_prompt: true,
            continuation_prompt: "   … ".to_string(),
        }
    }

    /// Set the prompt string.
    pub fn with_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = prompt.into();
        self
    }

    /// Set the continuation prompt for multi-line input.
    pub fn with_continuation(mut self, prompt: impl Into<String>) -> Self {
        self.continuation_prompt = prompt.into();
        self
    }

    /// Load history from a vector of strings.
    pub fn with_history(mut self, history: Vec<String>) -> Self {
        self.history = history;
        self
    }

    /// Set whether to print the prompt automatically.
    pub fn with_show_prompt(mut self, show: bool) -> Self {
        self.show_prompt = show;
        self
    }

    /// Read a complete input from the user. Returns the submitted text.
    /// Ctrl+D on an empty line returns empty string (signals EOF).
    /// Sending `/submit` (or pressing Ctrl+Enter) submits multi-line input.
    pub fn read_input(&mut self) -> Result<String> {
        // Initialize fresh input state
        self.lines = vec![String::new()];
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.history_idx = None;
        self.saved_input.clear();
        self.autocomplete_active = false;

        loop {
            // Render current state
            self.render()?;

            // Read key event
            let event = event::read()?;

            match event {
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    match (key.code, key.modifiers) {
                        // ── Submit ──────────────────────────────────────
                        (KeyCode::Enter, KeyModifiers::NONE) => {
                            let input = self.current_input();
                            // Check for /submit command
                            if input.trim() == "/submit" {
                                // Submit the accumulated multi-line content before /submit
                                // In this case, we just submit whatever is there
                                self.commit_to_history(&input);
                                self.clear_autocomplete();
                                self.render()?;
                                return Ok(input);
                            }
                            // Treat multi-line: Enter adds a new line
                            // Single-line submit: when cursor is on last line
                            self.commit_to_history(&input);
                            self.clear_autocomplete();
                            self.render_newline()?;
                            return Ok(input);
                        }
                        (KeyCode::Enter, KeyModifiers::CONTROL) => {
                            // Ctrl+Enter always submits multi-line content
                            let input = self.current_input();
                            self.commit_to_history(&input);
                            self.clear_autocomplete();
                            self.render_newline()?;
                            return Ok(input);
                        }

                        // ── Navigation ──────────────────────────────────
                        (KeyCode::Up, KeyModifiers::NONE) => {
                            if self.autocomplete_active {
                                self.autocomplete_prev();
                            } else {
                                self.history_prev();
                            }
                        }
                        (KeyCode::Down, KeyModifiers::NONE) => {
                            if self.autocomplete_active {
                                self.autocomplete_next();
                            } else {
                                self.history_next();
                            }
                        }
                        (KeyCode::Left, KeyModifiers::NONE) => {
                            self.cursor_left();
                            self.clear_autocomplete();
                        }
                        (KeyCode::Right, KeyModifiers::NONE) => {
                            self.cursor_right();
                            self.clear_autocomplete();
                        }
                        (KeyCode::Home, _) => {
                            self.cursor_col = 0;
                            self.clear_autocomplete();
                        }
                        (KeyCode::End, _) => {
                            self.cursor_col = self.current_line().len();
                            self.clear_autocomplete();
                        }

                        // ── Editing ─────────────────────────────────────
                        (KeyCode::Backspace, _) => {
                            self.delete_backward();
                            self.clear_autocomplete();
                        }
                        (KeyCode::Delete, _) => {
                            self.delete_forward();
                            self.clear_autocomplete();
                        }
                        (KeyCode::Tab, _) => {
                            self.autocomplete_apply();
                        }

                        // ── Character input ─────────────────────────────
                        (KeyCode::Char(c), mods) => {
                            if mods.contains(KeyModifiers::CONTROL) {
                                match c {
                                    'd' | 'D' => {
                                        // Ctrl+D on empty line = EOF
                                        if self.current_input().is_empty() {
                                            self.render_newline()?;
                                            return Ok(String::new());
                                        }
                                        // Otherwise delete forward
                                        self.delete_forward();
                                    }
                                    'u' | 'U' => {
                                        // Ctrl+U: clear current line
                                        *self.current_line_mut() = String::new();
                                        self.cursor_col = 0;
                                    }
                                    'w' | 'W' => {
                                        // Ctrl+W: delete word backward
                                        self.delete_word_backward();
                                    }
                                    'c' | 'C' => {
                                        // Ctrl+C: cancel input
                                        self.render_newline()?;
                                        return Ok(String::new());
                                    }
                                    _ => {}
                                }
                            } else {
                                self.insert_char(c);
                            }
                            self.clear_autocomplete();
                        }

                        // ── Escape ──────────────────────────────────────
                        (KeyCode::Esc, _) => {
                            if self.autocomplete_active {
                                self.clear_autocomplete();
                            }
                        }

                        _ => {}
                    }
                }
                Event::Resize(_, _) => {
                    // Terminal resized — just re-render
                }
                _ => {}
            }
        }
    }

    // ─── Internal helpers ──────────────────────────────────────────────────

    fn current_line(&self) -> &str {
        &self.lines[self.cursor_row]
    }

    fn current_line_mut(&mut self) -> &mut String {
        &mut self.lines[self.cursor_row]
    }

    fn current_input(&self) -> String {
        self.lines.join("\n")
    }

    fn cursor_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].len();
        }
    }

    fn cursor_right(&mut self) {
        let line_len = self.current_line().len();
        if self.cursor_col < line_len {
            self.cursor_col += 1;
        } else if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col = 0;
        }
    }

    fn insert_char(&mut self, c: char) {
        let line = self.current_line_mut();
        if self.cursor_col <= line.len() {
            line.insert(self.cursor_col, c);
            self.cursor_col += 1;
        }
    }

    fn delete_backward(&mut self) {
        if self.cursor_col > 0 {
            let line = self.current_line_mut();
            self.cursor_col -= 1;
            line.remove(self.cursor_col);
        } else if self.cursor_row > 0 {
            // Merge with previous line
            let tail = self.lines.remove(self.cursor_row);
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].len();
            self.lines[self.cursor_row].push_str(&tail);
        }
    }

    fn delete_forward(&mut self) {
        let line = self.current_line_mut();
        if self.cursor_col < line.len() {
            line.remove(self.cursor_col);
        } else if self.cursor_row + 1 < self.lines.len() {
            // Merge next line
            let next = self.lines.remove(self.cursor_row + 1);
            self.lines[self.cursor_row].push_str(&next);
        }
    }

    fn delete_word_backward(&mut self) {
        let line = self.current_line_mut();
        // Skip whitespace
        while self.cursor_col > 0
            && line.as_bytes().get(self.cursor_col - 1).map_or(false, |b| b.is_ascii_whitespace())
        {
            self.cursor_col -= 1;
            line.remove(self.cursor_col);
        }
        // Delete word
        while self.cursor_col > 0
            && line.as_bytes().get(self.cursor_col - 1).map_or(false, |b| !b.is_ascii_whitespace())
        {
            self.cursor_col -= 1;
            line.remove(self.cursor_col);
        }
    }

    // ─── History navigation ────────────────────────────────────────────────

    fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        if self.history_idx.is_none() {
            self.saved_input = self.current_input();
            self.history_idx = Some(self.history.len().saturating_sub(1));
        } else if let Some(idx) = self.history_idx {
            if idx > 0 {
                self.history_idx = Some(idx - 1);
            }
        }
        if let Some(idx) = self.history_idx {
            self.set_input(&self.history[idx]);
        }
    }

    fn history_next(&mut self) {
        if let Some(idx) = self.history_idx {
            if idx + 1 < self.history.len() {
                self.history_idx = Some(idx + 1);
                self.set_input(&self.history[idx + 1]);
            } else {
                self.history_idx = None;
                self.set_input(&self.saved_input);
                self.saved_input.clear();
            }
        }
    }

    fn commit_to_history(&mut self, entry: &str) {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            return;
        }
        // Dedup against last entry
        if self.history.last().map(|s| s.as_str()) == Some(trimmed) {
            return;
        }
        self.history.push(trimmed.to_string());
        if self.history.len() > self.max_history {
            let excess = self.history.len() - self.max_history;
            self.history.drain(..excess);
        }
    }

    fn set_input(&mut self, s: &str) {
        self.lines = s.split('\n').map(String::from).collect();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor_row = self.lines.len() - 1;
        self.cursor_col = self.lines[self.cursor_row].len();
    }

    // ─── Autocomplete ──────────────────────────────────────────────────────

    fn compute_autocomplete(&mut self) {
        self.autocomplete_matches.clear();
        self.autocomplete_idx = 0;
        self.autocomplete_active = false;

        let line = self.current_line();
        if !line.starts_with('/') {
            return;
        }

        let matches: Vec<&str> = SLASH_COMMANDS
            .iter()
            .filter(|c| c.starts_with(line) && **c != line)
            .copied()
            .collect();

        if !matches.is_empty() {
            self.autocomplete_matches = matches.into_iter().map(String::from).collect();
            self.autocomplete_active = true;
        }
    }

    fn autocomplete_prev(&mut self) {
        if self.autocomplete_idx > 0 {
            self.autocomplete_idx -= 1;
        } else {
            self.autocomplete_idx = self.autocomplete_matches.len().saturating_sub(1);
        }
    }

    fn autocomplete_next(&mut self) {
        if self.autocomplete_idx + 1 < self.autocomplete_matches.len() {
            self.autocomplete_idx += 1;
        } else {
            self.autocomplete_idx = 0;
        }
    }

    fn autocomplete_apply(&mut self) {
        self.compute_autocomplete();
        if let Some(matched) = self.autocomplete_matches.get(self.autocomplete_idx) {
            self.set_input(matched);
        }
        self.clear_autocomplete();
    }

    fn clear_autocomplete(&mut self) {
        self.autocomplete_matches.clear();
        self.autocomplete_idx = 0;
        self.autocomplete_active = false;
    }

    // ─── Rendering ─────────────────────────────────────────────────────────

    fn render(&mut self) -> Result<()> {
        let mut stdout = io::stdout();

        // Move to beginning of current input
        // This is approximate — in a real TUI we'd track exact position
        // For CLI use, we clear line and re-print

        // Print prompt and all lines
        for (i, line) in self.lines.iter().enumerate() {
            if i == 0 {
                execute!(stdout, Print(&self.prompt))?;
            } else {
                execute!(stdout, Print(&self.continuation_prompt))?;
            }

            // Print the line with potential syntax highlighting
            self.print_highlighted(&mut stdout, line)?;

            // Clear rest of line
            execute!(stdout, Clear(ClearType::UntilNewLine))?;

            if i + 1 < self.lines.len() {
                execute!(stdout, Print("\r\n"))?;
            }
        }

        // Show autocomplete suggestions if active
        if self.autocomplete_active {
            execute!(
                stdout,
                Print("\r\n"),
                SetForegroundColor(Color::DarkGrey),
                Print("  suggestions: "),
            )?;
            for (i, m) in self.autocomplete_matches.iter().enumerate() {
                if i == self.autocomplete_idx {
                    execute!(
                        stdout,
                        SetForegroundColor(Color::Cyan),
                        Print(format!("[{}] ", m)),
                    )?;
                } else {
                    execute!(
                        stdout,
                        SetForegroundColor(Color::DarkGrey),
                        Print(format!("{} ", m)),
                    )?;
                }
            }
            execute!(stdout, ResetColor, Clear(ClearType::UntilNewLine))?;
        }

        // Position cursor
        let cursor_line_offset = self.cursor_row as u16;
        execute!(
            stdout,
            cursor::MoveToColumn(
                (self.prompt.len() + self.cursor_col) as u16 + if self.cursor_row == 0 { 0 } else { self.continuation_prompt.len() as u16 }
            ),
        )?;

        // In practice, we'd need to move cursor up/down — simplified here
        stdout.flush()?;
        Ok(())
    }

    fn render_newline(&mut self) -> Result<()> {
        execute!(io::stdout(), Print("\r\n"))?;
        Ok(())
    }

    /// Print text with basic syntax highlighting for code blocks.
    fn print_highlighted(&self, stdout: &mut io::Stdout, text: &str) -> Result<()> {
        // Simple highlighting: strings in green, keywords in yellow
        let mut in_string = false;
        let mut in_backtick = false;
        let mut word = String::new();

        for ch in text.chars() {
            match ch {
                '"' if !in_backtick => {
                    if in_string {
                        // Close string
                        execute!(
                            stdout,
                            SetForegroundColor(Color::Green),
                            Print(format!("{}\"", word)),
                            ResetColor,
                        )?;
                        word.clear();
                        in_string = false;
                    } else {
                        // Flush word
                        self.flush_word(stdout, &word)?;
                        word.clear();
                        in_string = true;
                        execute!(stdout, SetForegroundColor(Color::Green), Print("\""))?;
                    }
                }
                '`' => {
                    in_backtick = !in_backtick;
                    execute!(
                        stdout,
                        SetForegroundColor(Color::Yellow),
                        Print("`"),
                        ResetColor,
                    )?;
                }
                c if c.is_alphanumeric() || c == '_' || c == '-' => {
                    word.push(c);
                }
                c => {
                    self.flush_word(stdout, &word)?;
                    word.clear();
                    execute!(stdout, Print(c.to_string()))?;
                }
            }
        }
        // Flush remaining
        if in_string {
            execute!(
                stdout,
                SetForegroundColor(Color::Green),
                Print(&word),
                ResetColor,
            )?;
        } else {
            self.flush_word(stdout, &word)?;
        }
        Ok(())
    }

    fn flush_word(&self, stdout: &mut io::Stdout, word: &str) -> Result<()> {
        if word.is_empty() {
            return Ok(());
        }
        const KEYWORDS: &[&str] = &[
            "fn", "pub", "if", "else", "return", "let", "mut", "const", "struct",
            "impl", "trait", "use", "as", "match", "async", "await", "mod",
        ];
        if KEYWORDS.contains(&word) {
            execute!(
                stdout,
                SetForegroundColor(Color::Magenta),
                Print(word),
                ResetColor,
            )?;
        } else {
            execute!(stdout, Print(word))?;
        }
        Ok(())
    }
}

impl Default for InputComposer {
    fn default() -> Self {
        Self::new()
    }
}
