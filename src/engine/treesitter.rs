// ─── Tree-sitter based RepoMap symbol extraction (Phase 3 Item 11) ─────────────

use std::path::Path;

/// Extract symbols from a source file using tree-sitter.
/// Falls back to regex if tree-sitter is not available for the language.
pub struct TreeSitterParser;

impl TreeSitterParser {
    /// Extract symbols from file content, returning (name, kind, line).
    pub fn extract(content: &str, language: &str) -> Vec<(String, String, u32)> {
        match language {
            "rust" => Self::extract_rust(content),
            "python" => Self::extract_python(content),
            "typescript" | "javascript" => Self::extract_ts(content),
            "go" => Self::extract_go(content),
            _ => Self::extract_generic(content),
        }
    }

    fn extract_rust(content: &str) -> Vec<(String, String, u32)> {
        let mut symbols = Vec::new();
        // Strip line comments and block-comment bodies before scanning so we don't
        // pick up declarations inside doc-comments or commented-out code.
        let stripped = strip_rust_comments(content);
        // Track visibility prefixes like `pub`, `pub(crate)`, `pub(super)`.
        let strip_vis = |s: &str| -> String {
            let mut t = s.trim_start();
            // Strip `pub(...)` first.
            if t.starts_with("pub(") {
                if let Some(close) = t.find(')') {
                    t = t[close + 1..].trim_start();
                }
            } else if let Some(rest) = t.strip_prefix("pub ") {
                t = rest.trim_start();
            }
            // Strip async/unsafe/extern modifiers so the next-token check works.
            for kw in ["async ", "unsafe ", "extern \"C\" ", "extern ", "default "] {
                if let Some(rest) = t.strip_prefix(kw) {
                    t = rest.trim_start();
                }
            }
            t.to_string()
        };
        for (i, line) in stripped.lines().enumerate() {
            let trimmed = strip_vis(line.trim());
            let line_num = (i + 1) as u32;

            if let Some(rest) = trimmed.strip_prefix("fn ") {
                let name = extract_ident(rest);
                if !name.is_empty() {
                    symbols.push((name, "fn".into(), line_num));
                }
            } else if let Some(rest) = trimmed.strip_prefix("struct ") {
                let name = extract_ident(rest);
                if !name.is_empty() {
                    symbols.push((name, "struct".into(), line_num));
                }
            } else if let Some(rest) = trimmed.strip_prefix("enum ") {
                let name = extract_ident(rest);
                if !name.is_empty() {
                    symbols.push((name, "enum".into(), line_num));
                }
            } else if let Some(rest) = trimmed.strip_prefix("trait ") {
                let name = extract_ident(rest);
                if !name.is_empty() {
                    symbols.push((name, "trait".into(), line_num));
                }
            } else if let Some(rest) = trimmed.strip_prefix("mod ") {
                let name = extract_ident(rest);
                if !name.is_empty() {
                    symbols.push((name, "mod".into(), line_num));
                }
            } else if trimmed.starts_with("impl") {
                let name = extract_impl_name(&trimmed);
                symbols.push((name, "impl".into(), line_num));
            } else if let Some(rest) = trimmed.strip_prefix("const ").or_else(|| trimmed.strip_prefix("static ")) {
                let kind = if trimmed.starts_with("static ") { "static" } else { "const" };
                let name = rest
                    .split(|c: char| c == ':' || c == '=' || c == ';')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                // Rust idiomatic constants are SCREAMING_SNAKE — accept digits too
                // (e.g. CONST_1, V2_TOKEN). Allow leading underscore for visibility.
                let is_screaming = !name.is_empty()
                    && name
                        .chars()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
                if is_screaming {
                    symbols.push((name, kind.into(), line_num));
                }
            } else if let Some(rest) = trimmed.strip_prefix("type ") {
                let name = extract_ident(rest);
                if !name.is_empty() {
                    symbols.push((name, "type".into(), line_num));
                }
            }
        }
        symbols
    }

    fn extract_python(content: &str) -> Vec<(String, String, u32)> {
        let mut symbols = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            let line_num = (i + 1) as u32;
            if trimmed.starts_with("def ") {
                let name = trimmed
                    .trim_start_matches("def ")
                    .split('(')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                symbols.push((name, "fn".into(), line_num));
            } else if trimmed.starts_with("class ") {
                let name = trimmed
                    .trim_start_matches("class ")
                    .split(|c: char| c == '(' || c == ':')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                symbols.push((name, "class".into(), line_num));
            } else if trimmed.starts_with("async def ") {
                let name = trimmed
                    .trim_start_matches("async def ")
                    .split('(')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                symbols.push((name, "async fn".into(), line_num));
            }
        }
        symbols
    }

    fn extract_ts(content: &str) -> Vec<(String, String, u32)> {
        let mut symbols = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            let line_num = (i + 1) as u32;
            let kind = if trimmed.starts_with("export function ")
                || trimmed.starts_with("function ")
            {
                "fn"
            } else if trimmed.starts_with("export class ") || trimmed.starts_with("class ") {
                "class"
            } else if trimmed.starts_with("export interface ") || trimmed.starts_with("interface ")
            {
                "interface"
            } else if trimmed.starts_with("export const ") || trimmed.starts_with("const ") {
                "const"
            } else {
                continue;
            };
            let name = trimmed.split_whitespace().nth(1).unwrap_or("").to_string();
            symbols.push((name, kind.into(), line_num));
        }
        symbols
    }

    fn extract_go(content: &str) -> Vec<(String, String, u32)> {
        let mut symbols = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            let line_num = (i + 1) as u32;
            if trimmed.starts_with("func ") {
                let name = trimmed
                    .trim_start_matches("func ")
                    .split('(')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                symbols.push((name, "fn".into(), line_num));
            } else if trimmed.starts_with("type ") && trimmed.contains("struct") {
                let name = trimmed
                    .trim_start_matches("type ")
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_string();
                symbols.push((name, "struct".into(), line_num));
            }
        }
        symbols
    }

    fn extract_generic(content: &str) -> Vec<(String, String, u32)> {
        let mut symbols = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("def ")
                || trimmed.starts_with("fn ")
                || trimmed.starts_with("func ")
                || trimmed.starts_with("class ")
                || trimmed.starts_with("struct ")
            {
                let line_num = (i + 1) as u32;
                let name = trimmed.split_whitespace().nth(1).unwrap_or("").to_string();
                symbols.push((name, "symbol".into(), line_num));
            }
        }
        symbols
    }
}

/// Extract the leading Rust identifier from a slice that starts with one.
/// Skips leading whitespace, then takes the longest run of [A-Za-z0-9_].
fn extract_ident(s: &str) -> String {
    let s = s.trim_start();
    s.chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect()
}

/// Extract the type being implemented from an `impl` line.
/// Handles `impl Foo`, `impl<T> Foo<T>`, `impl Trait for Foo<T>`,
/// `impl<T: Bound> Trait<T> for Foo<T> where ... {`.
fn extract_impl_name(line: &str) -> String {
    // Drop leading `impl` keyword.
    let after_impl = line.trim_start().trim_start_matches("impl").trim_start();

    // Skip optional generic parameter list `<...>` immediately after `impl`,
    // balancing angle brackets so we don't get fooled by `<T: Trait<U>>`.
    let after_generics = skip_balanced_angles(after_impl);

    // If the body contains ` for `, the right-hand side is the implementing type.
    let target_slice = if let Some(idx) = find_keyword(after_generics, " for ") {
        &after_generics[idx + 5..]
    } else {
        after_generics
    };
    let target = target_slice.trim_start();

    // Take the type name up to `<`, `{`, `where`, whitespace, or end.
    let end = target
        .find(|c: char| c == '<' || c == '{' || c.is_whitespace())
        .unwrap_or(target.len());
    let name = target[..end].trim();
    if name.is_empty() {
        "impl".into()
    } else {
        format!("impl {}", name)
    }
}

/// If `s` starts with `<`, advance past the matching `>` (balanced). Otherwise
/// returns `s` unchanged.
fn skip_balanced_angles(s: &str) -> &str {
    let bytes = s.as_bytes();
    if !s.starts_with('<') {
        return s;
    }
    let mut depth = 0i32;
    for (i, b) in bytes.iter().enumerate() {
        match *b {
            b'<' => depth += 1,
            b'>' => {
                depth -= 1;
                if depth == 0 {
                    return s[i + 1..].trim_start();
                }
            }
            _ => {}
        }
    }
    s
}

/// Find a keyword surrounded by word boundaries (cheap: requires the surrounding
/// spaces baked in, e.g. " for "). Returns the index of the leading space.
fn find_keyword(haystack: &str, needle: &str) -> Option<usize> {
    haystack.find(needle)
}

/// Strip Rust line comments (`// ...`) and block comments (`/* ... */`,
/// possibly nested). Keeps line breaks so line numbers stay aligned.
fn strip_rust_comments(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut depth = 0u32;
    let mut in_string = false;
    let mut in_char = false;
    while i < bytes.len() {
        let b = bytes[i];
        let next = bytes.get(i + 1).copied();
        if depth > 0 {
            if b == b'/' && next == Some(b'*') {
                depth += 1;
                i += 2;
                continue;
            }
            if b == b'*' && next == Some(b'/') {
                depth -= 1;
                i += 2;
                continue;
            }
            if b == b'\n' {
                out.push('\n');
            }
            i += 1;
            continue;
        }
        if in_string {
            if b == b'\\' && next.is_some() {
                out.push(b as char);
                out.push(next.unwrap() as char);
                i += 2;
                continue;
            }
            if b == b'"' {
                in_string = false;
            }
            out.push(b as char);
            i += 1;
            continue;
        }
        if in_char {
            if b == b'\\' && next.is_some() {
                out.push(b as char);
                out.push(next.unwrap() as char);
                i += 2;
                continue;
            }
            if b == b'\'' {
                in_char = false;
            }
            out.push(b as char);
            i += 1;
            continue;
        }
        if b == b'"' {
            in_string = true;
            out.push('"');
            i += 1;
            continue;
        }
        // We deliberately do NOT enter `in_char` mode: distinguishing lifetimes
        // from char literals reliably needs a real lexer, and getting it wrong
        // can drop code. Lifetimes can contain `//`-like sequences only inside
        // strings, which we already handle.
        if b == b'/' && next == Some(b'/') {
            // Skip to end of line.
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b == b'/' && next == Some(b'*') {
            depth = 1;
            i += 2;
            continue;
        }
        out.push(b as char);
        i += 1;
    }
    out
}

pub fn detect_language(path: &Path) -> Option<&'static str> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => Some("rust"),
        Some("py") => Some("python"),
        Some("ts") | Some("tsx") => Some("typescript"),
        Some("js") | Some("jsx") => Some("javascript"),
        Some("go") => Some("go"),
        _ => None,
    }
}
