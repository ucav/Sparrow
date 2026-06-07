// ─── Code syntax highlighting via syntect ─────────────────────────────────────
// Uses syntect for syntax highlighting and emits 24-bit ANSI terminal escapes.

use anyhow::{Context, Result};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// Default syntax theme used when none is specified.
const DEFAULT_THEME: &str = "base16-ocean.dark";

/// Highlight a code string with syntax highlighting, producing an ANSI-coloured
/// String suitable for terminal output.
///
/// If `language` is empty or unrecognised, falls back to plain text.
/// If `theme` is empty, uses the built-in `base16-ocean.dark` theme.
pub fn highlight(code: &str, language: &str, theme: &str) -> Result<String> {
    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();

    let syntax = language_syntax(&ss, language);
    let theme = ts
        .themes
        .get(if theme.is_empty() {
            DEFAULT_THEME
        } else {
            theme
        })
        .unwrap_or_else(|| {
            ts.themes
                .get(DEFAULT_THEME)
                .expect("built-in theme should exist")
        });

    let mut h = HighlightLines::new(syntax, theme);
    let mut out = String::with_capacity(code.len() * 2);

    for line in LinesWithEndings::from(code) {
        let ranges = h
            .highlight_line(line, &ss)
            .context("syntax highlight failed")?;
        let escaped = syntect::util::as_24_bit_terminal_escaped(&ranges[..], false);
        out.push_str(&escaped);
    }

    Ok(out)
}

/// Highlight a code string with line numbers prepended on the left.
///
/// Line numbers are dimmed grey so they don't distract from the code itself.
pub fn highlight_with_line_numbers(code: &str, language: &str, theme: &str) -> Result<String> {
    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();

    let syntax = language_syntax(&ss, language);
    let theme = ts
        .themes
        .get(if theme.is_empty() {
            DEFAULT_THEME
        } else {
            theme
        })
        .unwrap_or_else(|| {
            ts.themes
                .get(DEFAULT_THEME)
                .expect("built-in theme should exist")
        });

    let mut h = HighlightLines::new(syntax, theme);
    let lines: Vec<&str> = LinesWithEndings::from(code).collect();
    let total = lines.len();
    let gutter_width = if total == 0 {
        1
    } else {
        total.to_string().len()
    };

    let dim_reset = "\x1b[0m";
    // Dim grey for line numbers: \x1b[38;2;100;100;100m
    let dim_prefix = "\x1b[38;2;100;100;100m";

    let mut out = String::new();
    for (i, line) in lines.into_iter().enumerate() {
        // Line number
        out.push_str(&format!(
            "{dim_prefix}{:>gutter_width$} │ {dim_reset}",
            i + 1,
            gutter_width = gutter_width
        ));

        let ranges = h
            .highlight_line(line, &ss)
            .context("syntax highlight failed")?;
        let escaped = syntect::util::as_24_bit_terminal_escaped(&ranges[..], false);
        out.push_str(&escaped);
    }

    Ok(out)
}

/// Auto-detect the language from a file extension or common name.
/// Maps language names / extensions to syntect syntax names.
pub(crate) fn language_syntax<'a>(
    ss: &'a SyntaxSet,
    language: &str,
) -> &'a syntect::parsing::SyntaxReference {
    let lang_lower = language.trim().to_lowercase();

    // Try by direct syntax name
    if !lang_lower.is_empty() {
        if let Some(s) = ss.find_syntax_by_name(&lang_lower) {
            return s;
        }
    }

    // Try by extension (strip leading '.' if present, or treat as extension)
    let ext = lang_lower.strip_prefix('.').unwrap_or(&lang_lower);
    if !ext.is_empty() {
        if let Some(s) = ss.find_syntax_by_extension(ext) {
            return s;
        }
    }

    // Common aliases
    let alias = match ext {
        "js" | "javascript" => "JavaScript",
        "ts" | "typescript" => "TypeScript",
        "py" | "python" => "Python",
        "rs" | "rust" => "Rust",
        "go" | "golang" => "Go",
        "rb" | "ruby" => "Ruby",
        "java" => "Java",
        "c" => "C",
        "cpp" | "c++" | "cxx" => "C++",
        "cs" | "csharp" | "c#" => "C#",
        "sh" | "bash" | "shell" => "Bash",
        "zsh" => "Bash",
        "fish" => "Fish",
        "ps1" | "powershell" => "PowerShell",
        "sql" => "SQL",
        "html" => "HTML",
        "css" => "CSS",
        "json" => "JSON",
        "xml" => "XML",
        "yaml" | "yml" => "YAML",
        "toml" => "TOML",
        "ini" | "cfg" | "conf" => "INI",
        "md" | "markdown" => "Markdown",
        "dockerfile" | "docker" => "Dockerfile",
        "makefile" | "make" => "Makefile",
        _ => "",
    };

    if !alias.is_empty() {
        if let Some(s) = ss.find_syntax_by_name(alias) {
            return s;
        }
    }

    // Fallback to plain text
    ss.find_syntax_plain_text()
}
