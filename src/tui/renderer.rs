//! CLI Rich Rendering Engine for Sparrow.
//!
//! Provides terminal-native rendering of markdown, code with syntax highlighting,
//! diffs, JSON, tables, and key-value pairs.

pub struct RenderConfig {
    pub width: u16,
    pub colors_enabled: bool,
    pub syntax_theme: String,
    pub line_numbers: bool,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            width: console::Term::stdout().size().1,
            colors_enabled: true,
            syntax_theme: "base16-ocean.dark".into(),
            line_numbers: true,
        }
    }
}

pub struct TermRenderer {
    pub config: RenderConfig,
}

impl TermRenderer {
    pub fn new(config: RenderConfig) -> Self {
        Self { config }
    }

    pub fn render_markdown(&self, md: &str) -> String {
        crate::tui::formatters::markdown::render_markdown_with_theme(md, &self.config.syntax_theme)
    }

    pub fn render_code(&self, code: &str, language: &str) -> String {
        let lang = if language.is_empty() {
            None
        } else {
            Some(language)
        };
        match lang {
            Some(l) => crate::tui::formatters::code::highlight_with_line_numbers(
                code,
                l,
                &self.config.syntax_theme,
            )
            .unwrap_or_else(|_| code.to_string()),
            None => crate::tui::formatters::code::highlight(code, "", &self.config.syntax_theme)
                .unwrap_or_else(|_| code.to_string()),
        }
    }

    pub fn render_diff(&self, diff: &str) -> String {
        crate::tui::formatters::diff::format_diff(diff)
    }

    pub fn render_json(&self, json: &str) -> String {
        crate::tui::formatters::json::format_json(json)
    }

    pub fn render_table(&self, headers: &[&str], rows: &[Vec<String>]) -> String {
        crate::tui::formatters::table::render_table(
            headers,
            rows,
            Some(self.config.width as usize),
            None,
        )
    }

    pub fn render_key_value(&self, pairs: &[(&str, &str)]) -> String {
        if pairs.is_empty() {
            return String::new();
        }
        let max_key = pairs.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
        let mut out = String::new();
        for (key, value) in pairs {
            out.push_str(&format!("  {:>width$} : {}\n", key, value, width = max_key));
        }
        out
    }
}
