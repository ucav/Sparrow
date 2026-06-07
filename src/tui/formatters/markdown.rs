// ─── Markdown → terminal formatting ───────────────────────────────────────────
// Uses pulldown-cmark to parse markdown and emits ANSI-styled terminal output.
// Headers become bold, code blocks highlighted, lists indented, links shown as
// [text](url), bold/italic via ANSI styles.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// ANSI style constants for markdown elements.
pub struct MarkdownStyles {
    pub h1: &'static str,
    pub h2: &'static str,
    pub h3: &'static str,
    pub bold: &'static str,
    pub italic: &'static str,
    pub code_inline: &'static str,
    pub code_block_lang: &'static str,
    pub link_url: &'static str,
    pub list_marker: &'static str,
    pub blockquote: &'static str,
    pub hr: &'static str,
    pub text: &'static str,
    pub reset: &'static str,
}

impl Default for MarkdownStyles {
    fn default() -> Self {
        Self {
            h1: "\x1b[1;38;2;242;169;60m",  // bold amber
            h2: "\x1b[1;38;2;111;166;230m", // bold blue
            h3: "\x1b[1;38;2;78;201;176m",  // bold teal
            bold: "\x1b[1m",
            italic: "\x1b[3m",
            code_inline: "\x1b[48;2;22;18;13;38;2;242;201;76m", // gold on dark bg
            code_block_lang: "\x1b[38;2;137;125;108m",          // dim
            link_url: "\x1b[38;2;111;166;230m",                 // blue
            list_marker: "\x1b[38;2;242;169;60m",               // amber
            blockquote: "\x1b[38;2;137;125;108m",               // dim
            hr: "\x1b[38;2;92;83;70m",                          // dimmer
            text: "",                                           // default terminal colour
            reset: "\x1b[0m",
        }
    }
}

/// Default syntax theme for fenced code blocks.
const DEFAULT_SYNTAX_THEME: &str = "base16-ocean.dark";

/// Convert a markdown string to ANSI-formatted terminal text.
///
/// Headers (# → bold), fenced code blocks (``` → syntax highlighted), lists,
/// bold/italic, inline code, blockquotes, and horizontal rules are all styled.
pub fn render_markdown(md: &str) -> String {
    render_markdown_with_theme(md, DEFAULT_SYNTAX_THEME)
}

/// Render markdown with a specific syntax highlighting theme for code blocks.
pub fn render_markdown_with_theme(md: &str, syntax_theme: &str) -> String {
    let styles = MarkdownStyles::default();
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(md, options);

    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let theme = ts
        .themes
        .get(syntax_theme)
        .or_else(|| ts.themes.get(DEFAULT_SYNTAX_THEME))
        .expect("built-in theme should exist");

    let mut out = String::with_capacity(md.len() * 2);
    let mut in_code_block = false;
    let mut code_lang: Option<String> = None;
    let mut current_link_url: Option<String> = None;
    let mut code_buf = String::new();
    let mut list_depth: usize = 0;
    let mut ordered_index: Option<u64> = None;

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading {
                    level,
                    id: _,
                    classes: _,
                    attrs: _,
                } => {
                    let style = match level {
                        pulldown_cmark::HeadingLevel::H1 => styles.h1,
                        pulldown_cmark::HeadingLevel::H2 => styles.h2,
                        _ => styles.h3,
                    };
                    out.push_str(style);
                    // Print # markers
                    let hashes: String = match level {
                        pulldown_cmark::HeadingLevel::H1 => "# ".into(),
                        pulldown_cmark::HeadingLevel::H2 => "## ".into(),
                        pulldown_cmark::HeadingLevel::H3 => "### ".into(),
                        pulldown_cmark::HeadingLevel::H4 => "#### ".into(),
                        pulldown_cmark::HeadingLevel::H5 => "##### ".into(),
                        pulldown_cmark::HeadingLevel::H6 => "###### ".into(),
                    };
                    out.push_str(&hashes);
                }
                Tag::CodeBlock(kind) => {
                    in_code_block = true;
                    code_buf.clear();
                    if let pulldown_cmark::CodeBlockKind::Fenced(lang) = kind {
                        let lang_str = lang.to_string();
                        if !lang_str.is_empty() {
                            code_lang = Some(lang_str);
                        } else {
                            code_lang = None;
                        }
                    } else {
                        code_lang = None;
                    }

                    // Print language tag
                    out.push('\n');
                    if let Some(ref lang) = code_lang {
                        out.push_str(&format!(
                            "{style}  ┌─ {lang} ─────────────────────────────{reset}\n",
                            style = styles.code_block_lang,
                            reset = styles.reset
                        ));
                    } else {
                        out.push_str(&format!(
                            "{style}  ┌─ code ───────────────────────────────{reset}\n",
                            style = styles.code_block_lang,
                            reset = styles.reset
                        ));
                    }
                }
                Tag::List(order) => {
                    list_depth += 1;
                    if let Some(start) = order {
                        ordered_index = Some(start);
                    } else {
                        ordered_index = None;
                    }
                }
                Tag::Item => {
                    out.push_str(styles.list_marker);
                    let indent = "  ".repeat(list_depth.saturating_sub(1));
                    out.push_str(&indent);
                    if let Some(idx) = ordered_index.as_mut() {
                        out.push_str(&format!("{idx}. "));
                        *idx += 1;
                    } else {
                        out.push_str("• ");
                    }
                    out.push_str(styles.reset);
                }
                Tag::BlockQuote(_) => {
                    out.push_str(styles.blockquote);
                }
                Tag::Strong => {
                    out.push_str(styles.bold);
                }
                Tag::Emphasis => {
                    out.push_str(styles.italic);
                }
                Tag::Strikethrough => {
                    // Strikethrough via ANSI is not widely supported; use dim
                    out.push_str(styles.blockquote);
                }
                Tag::Link {
                    link_type: _,
                    dest_url: _,
                    title: _,
                    id: _,
                } => {
                    // Links will be rendered when we hit the End tag
                }
                Tag::Image {
                    link_type: _,
                    dest_url: _,
                    title: _,
                    id: _,
                } => {
                    out.push_str(styles.blockquote);
                    out.push_str("[img: ");
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Heading(_) => {
                    out.push_str(styles.reset);
                    out.push('\n');
                    // Underline for h1, h2
                }
                TagEnd::CodeBlock => {
                    in_code_block = false;
                    // Highlight the code buffer
                    let lang = code_lang.as_deref().unwrap_or("");
                    let highlighted = highlight_code_block(&code_buf, lang, theme, &ss);
                    // Indent each line
                    for line in highlighted.lines() {
                        out.push_str("  │ ");
                        out.push_str(line);
                        out.push('\n');
                    }
                    out.push_str(&format!(
                        "{style}  └──────────────────────────────────────────{reset}\n",
                        style = styles.code_block_lang,
                        reset = styles.reset
                    ));
                    code_buf.clear();
                    code_lang = None;
                }
                TagEnd::List(_) => {
                    list_depth = list_depth.saturating_sub(1);
                    ordered_index = None;
                }
                TagEnd::Item => {
                    out.push('\n');
                }
                TagEnd::BlockQuote(_) => {
                    out.push_str(styles.reset);
                }
                TagEnd::Strong => {
                    out.push_str(styles.reset);
                }
                TagEnd::Emphasis => {
                    out.push_str(styles.reset);
                }
                TagEnd::Strikethrough => {
                    out.push_str(styles.reset);
                }
                TagEnd::Link => {
                    // URL was captured from start tag, just add closing marker
                    out.push_str(styles.blockquote);
                    out.push(' ');
                    out.push_str(styles.link_url);
                    if let Some(ref url) = current_link_url {
                        out.push('(');
                        out.push_str(url);
                        out.push(')');
                    }
                    out.push_str(styles.reset);
                    current_link_url = None;
                }
                TagEnd::Image => {
                    out.push_str(styles.blockquote);
                    out.push(']');
                    out.push_str(styles.reset);
                    current_link_url = None;
                }
                _ => {}
            },
            Event::Text(text) | Event::Code(text) => {
                if in_code_block {
                    code_buf.push_str(&text);
                } else {
                    out.push_str(&text);
                }
            }
            Event::Html(raw) => {
                // Strip HTML tags, keep content
                let stripped = strip_html_tags(&raw);
                if !stripped.is_empty() {
                    out.push_str(&stripped);
                }
            }
            Event::InlineHtml(raw) => {
                out.push_str(styles.blockquote);
                out.push_str(&raw);
                out.push_str(styles.reset);
            }
            Event::InlineMath(raw) | Event::DisplayMath(raw) => {
                out.push_str(styles.code_inline);
                out.push_str(&raw);
                out.push_str(styles.reset);
            }
            Event::FootnoteReference(name) => {
                out.push_str(styles.link_url);
                out.push_str(&format!("[^{}]", name));
                out.push_str(styles.reset);
            }
            Event::SoftBreak => {
                out.push(' ');
            }
            Event::HardBreak => {
                out.push('\n');
            }
            Event::Rule => {
                out.push('\n');
                out.push_str(styles.hr);
                out.push_str(&"─".repeat(60));
                out.push_str(styles.reset);
                out.push('\n');
            }
            Event::TaskListMarker(checked) => {
                out.push_str(styles.list_marker);
                if checked {
                    out.push_str("[x] ");
                } else {
                    out.push_str("[ ] ");
                }
                out.push_str(styles.reset);
            }
        }
    }

    out
}

/// Simple HTML tag stripper for inline HTML in markdown.
fn strip_html_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            out.push(ch);
        }
    }
    out
}

/// Highlight a code block string using syntect with line numbers.
fn highlight_code_block(
    code: &str,
    language: &str,
    theme: &syntect::highlighting::Theme,
    ss: &SyntaxSet,
) -> String {
    use syntect::easy::HighlightLines;

    let syntax = super::code::language_syntax(ss, language);

    let mut h = HighlightLines::new(syntax, theme);
    let mut out = String::with_capacity(code.len() * 2);

    for line in LinesWithEndings::from(code) {
        let Ok(ranges) = h.highlight_line(line, ss) else {
            out.push_str(line);
            continue;
        };
        let escaped = syntect::util::as_24_bit_terminal_escaped(&ranges[..], false);
        out.push_str(&escaped);
    }

    out.trim_end_matches('\n').to_string()
}
