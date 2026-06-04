//! Code navigation tools that bring Sparrow's self-coding loop closer to a
//! frontier agent's: `glob` (find files by name pattern) and `symbols`
//! (find where a symbol is defined / who calls it). The `symbols` tool is a
//! regex-based index today; a tree-sitter AST upgrade keeps the same name/schema.

use async_trait::async_trait;
use serde_json::json;
use std::path::{Path, PathBuf};

use super::{Tool, ToolCtx, ToolResult};
use crate::event::{Block, RiskLevel};

const SKIP_DIRS: &[&str] = &[".git", "target", "node_modules", "dist", "build", ".venv"];
const MAX_RESULTS: usize = 200;

fn walk(root: &Path, out: &mut Vec<PathBuf>) {
    if out.len() >= 5000 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if SKIP_DIRS.contains(&name) {
                continue;
            }
            walk(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// Translate a glob (`*`, `**`, `?`) into a regex anchored on the full rel path.
fn glob_to_regex(pattern: &str) -> String {
    let mut re = String::from("(?i)^");
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' => {
                if chars.peek() == Some(&'*') {
                    chars.next();
                    if chars.peek() == Some(&'/') {
                        chars.next();
                    }
                    re.push_str(".*");
                } else {
                    re.push_str("[^/\\\\]*");
                }
            }
            '?' => re.push('.'),
            '.' | '+' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$' | '\\' => {
                re.push('\\');
                re.push(c);
            }
            '/' => re.push_str("[/\\\\]"),
            _ => re.push(c),
        }
    }
    re.push('$');
    re
}

// ─── Glob tool ─────────────────────────────────────────────────────────────

pub struct Glob;

#[async_trait]
impl Tool for Glob {
    fn name(&self) -> &str {
        "glob"
    }
    fn description(&self) -> &str {
        "Find files by name pattern (e.g. '**/*.rs', 'src/**/mod.rs'). Returns matching paths."
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Glob pattern, e.g. **/*.rs" }
            },
            "required": ["pattern"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::ReadOnly
    }
    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let pattern = args["pattern"].as_str().unwrap_or("");
        if pattern.is_empty() {
            return Ok(ToolResult::error("glob: 'pattern' is required"));
        }
        let re = regex::Regex::new(&glob_to_regex(pattern))
            .map_err(|e| anyhow::anyhow!("bad glob pattern: {}", e))?;
        let root = ctx.workspace_root.clone();
        let mut files = Vec::new();
        walk(&root, &mut files);
        let mut matches: Vec<String> = files
            .iter()
            .filter_map(|p| {
                let rel = p.strip_prefix(&root).unwrap_or(p);
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                if re.is_match(&rel_str) {
                    Some(rel_str)
                } else {
                    None
                }
            })
            .collect();
        matches.sort();
        matches.truncate(MAX_RESULTS);
        if matches.is_empty() {
            Ok(ToolResult::text(format!("no files match '{}'", pattern)))
        } else {
            Ok(ToolResult::ok(vec![Block::Text(matches.join("\n"))]))
        }
    }
}

// ─── Symbols tool (regex index) ────────────────────────────────────────────

pub struct Symbols;

#[async_trait]
impl Tool for Symbols {
    fn name(&self) -> &str {
        "symbols"
    }
    fn description(&self) -> &str {
        "Find where a symbol is defined (mode=definition), list a file's symbols (mode=outline), or find callers (mode=callers). Languages: rust/py/js/ts."
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "Symbol name (or file path for outline)" },
                "mode": { "type": "string", "description": "definition | outline | callers (default: definition)" }
            },
            "required": ["name"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::ReadOnly
    }
    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let name = args["name"].as_str().unwrap_or("").trim();
        let mode = args["mode"].as_str().unwrap_or("definition");
        if name.is_empty() {
            return Ok(ToolResult::error("symbols: 'name' is required"));
        }
        let root = ctx.workspace_root.clone();

        #[cfg(feature = "treesitter")]
        {
            if mode == "definition" {
                let index = crate::memory::symbol_index::SymbolIndex::build(&root);
                let hits: Vec<String> = index
                    .find_definition(name)
                    .into_iter()
                    .take(MAX_RESULTS)
                    .map(format_symbol_def)
                    .collect();
                return if hits.is_empty() {
                    Ok(ToolResult::text(format!(
                        "no definition found for '{}'",
                        name
                    )))
                } else {
                    Ok(ToolResult::ok(vec![Block::Text(hits.join("\n"))]))
                };
            }
            if mode == "outline" {
                let index = crate::memory::symbol_index::SymbolIndex::build(&root);
                let requested = Path::new(name);
                let path = if requested.is_absolute() {
                    requested
                        .strip_prefix(&root)
                        .map(PathBuf::from)
                        .unwrap_or_else(|_| requested.to_path_buf())
                } else {
                    requested.to_path_buf()
                };
                let hits: Vec<String> = index
                    .outline(&path)
                    .into_iter()
                    .take(MAX_RESULTS)
                    .map(|def| {
                        format!(
                            "{}:{}: {} {}",
                            def.file.to_string_lossy().replace('\\', "/"),
                            def.line,
                            def.kind.as_str(),
                            def.signature
                        )
                    })
                    .collect();
                return if hits.is_empty() {
                    Ok(ToolResult::text(format!("no symbols in {}", name)))
                } else {
                    Ok(ToolResult::ok(vec![Block::Text(hits.join("\n"))]))
                };
            }
        }

        // Build the regex for the requested mode.
        let esc = regex::escape(name);
        let pattern = match mode {
            "callers" => format!(r"\b{}\s*\(", esc),
            "outline" => {
                // List definitions in a single file (name = path).
                return outline(&root, name);
            }
            _ => format!(
                r"\b(fn|struct|enum|trait|type|const|static|impl|class|def|function)\b[^\n]*\b{}\b",
                esc
            ),
        };
        let re = regex::Regex::new(&pattern).map_err(|e| anyhow::anyhow!("regex error: {}", e))?;

        let mut files = Vec::new();
        walk(&root, &mut files);
        let mut hits: Vec<String> = Vec::new();
        for path in &files {
            if !is_code_file(path) {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(path) else {
                continue;
            };
            for (i, line) in content.lines().enumerate() {
                if re.is_match(line) {
                    let rel = path.strip_prefix(&root).unwrap_or(path);
                    hits.push(format!(
                        "{}:{}: {}",
                        rel.to_string_lossy().replace('\\', "/"),
                        i + 1,
                        line.trim()
                    ));
                    if hits.len() >= MAX_RESULTS {
                        break;
                    }
                }
            }
            if hits.len() >= MAX_RESULTS {
                break;
            }
        }
        if hits.is_empty() {
            Ok(ToolResult::text(format!(
                "no {} found for '{}'",
                mode, name
            )))
        } else {
            Ok(ToolResult::ok(vec![Block::Text(hits.join("\n"))]))
        }
    }
}

#[cfg(feature = "treesitter")]
fn format_symbol_def(def: &crate::memory::symbol_index::SymbolDef) -> String {
    format!(
        "{}:{}: {} {}",
        def.file.to_string_lossy().replace('\\', "/"),
        def.line,
        def.kind.as_str(),
        def.signature
    )
}

fn is_code_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("rs" | "py" | "js" | "ts" | "tsx" | "jsx" | "go" | "java" | "c" | "cpp" | "h")
    )
}

fn outline(root: &Path, file_arg: &str) -> anyhow::Result<ToolResult> {
    let path = if Path::new(file_arg).is_absolute() {
        PathBuf::from(file_arg)
    } else {
        root.join(file_arg)
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Ok(ToolResult::error(format!("cannot read {}", file_arg)));
    };
    let re = regex::Regex::new(
        r"^\s*(pub\s+)?(fn|struct|enum|trait|impl|type|const|static|class|def|function)\b.*",
    )
    .unwrap();
    let mut out = Vec::new();
    for (i, line) in content.lines().enumerate() {
        if re.is_match(line) {
            out.push(format!("{}: {}", i + 1, line.trim()));
            if out.len() >= MAX_RESULTS {
                break;
            }
        }
    }
    if out.is_empty() {
        Ok(ToolResult::text(format!("no symbols in {}", file_arg)))
    } else {
        Ok(ToolResult::ok(vec![Block::Text(out.join("\n"))]))
    }
}
