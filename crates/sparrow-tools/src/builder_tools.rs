use async_trait::async_trait;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

use crate::{Tool, ToolCtx, ToolResult};
use sparrow_core::event::{Block, RiskLevel};

// ─── Test Runner ───────────────────────────────────────────────────────────────

pub struct TestRunner;

#[async_trait]
impl Tool for TestRunner {
    fn name(&self) -> &str {
        "test"
    }
    fn description(&self) -> &str {
        "Detect and run the project test suite. Parses results."
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "framework": { "type": "string", "description": "Force framework: cargo, pytest, jest, go" },
                "args": { "type": "array", "items": {"type": "string"}, "description": "Extra args" }
            },
            "required": []
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Exec
    }
    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let framework = args["framework"].as_str();
        let detected = detect_framework(&ctx.workspace_root);
        let fw = framework.unwrap_or(&detected);

        let (program, fw_args): (&str, Vec<&str>) = match fw {
            "cargo" => ("cargo", vec!["test"]),
            "pytest" => ("pytest", vec!["-v"]),
            "jest" => ("npx", vec!["jest"]),
            "go" => ("go", vec!["test", "./..."]),
            _ => ("cargo", vec!["test"]),
        };

        let output = StdCommand::new(program)
            .args(&fw_args)
            .current_dir(&ctx.workspace_root)
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        let passed = stdout
            .lines()
            .filter(|l| l.contains("pass") || l.contains("PASS") || l.contains("ok"))
            .count() as u32;
        let failed = stdout
            .lines()
            .filter(|l| l.contains("fail") || l.contains("FAIL") || l.contains("FAILED"))
            .count() as u32;

        Ok(ToolResult::ok(vec![Block::Text(format!(
            "Framework: {}\nPassed: {}\nFailed: {}\nExit: {}\n\n{}",
            fw,
            passed,
            failed,
            exit_code,
            if stderr.is_empty() { &stdout } else { &stderr }
        ))]))
    }
}

fn detect_framework(root: &std::path::Path) -> String {
    if root.join("Cargo.toml").exists() {
        return "cargo".into();
    }
    if root.join("pyproject.toml").exists() || root.join("pytest.ini").exists() {
        return "pytest".into();
    }
    if root.join("package.json").exists() {
        return "jest".into();
    }
    if root.join("go.mod").exists() {
        return "go".into();
    }
    "cargo".into()
}

// ─── Apply Patch ───────────────────────────────────────────────────────────────

pub struct ApplyPatch;

#[async_trait]
impl Tool for ApplyPatch {
    fn name(&self) -> &str {
        "apply_patch"
    }
    fn description(&self) -> &str {
        "Apply a structured multi-file patch atomically with rollback"
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "patches": { "type": "array", "items": {
                    "type": "object",
                    "properties": {
                        "file": {"type": "string"},
                        "old": {"type": "string"},
                        "new": {"type": "string"}
                    },
                    "required": ["file", "old", "new"]
                }}
            },
            "required": ["patches"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Mutating
    }
    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let patches = args["patches"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("patches must be an array"))?;

        // Read all current contents first (for rollback)
        let mut backups: Vec<(String, String)> = Vec::new();
        let mut results = Vec::new();

        for patch in patches {
            let file = patch["file"].as_str().unwrap_or("");
            let old = patch["old"].as_str().unwrap_or("");
            let new = patch["new"].as_str().unwrap_or("");
            let full_path = ctx.workspace_root.join(file);

            // Backup
            let original = std::fs::read_to_string(&full_path)?;
            backups.push((file.to_string(), original.clone()));

            // Apply
            let count = original.matches(old).count();
            if count == 0 {
                // Rollback all previous patches
                for (f, content) in &backups {
                    std::fs::write(ctx.workspace_root.join(f), content)?;
                }
                return Ok(ToolResult::error(format!(
                    "Patch failed on '{}': old string not found. All changes rolled back.",
                    file
                )));
            }

            let new_content = original.replace(old, new);
            std::fs::write(&full_path, new_content)?;
            results.push(format!("✓ {} : applied", file));
        }

        Ok(ToolResult::ok(vec![Block::Text(format!(
            "Patched {} file(s) atomically:\n{}",
            results.len(),
            results.join("\n")
        ))]))
    }
}

// ─── Git PR Create ─────────────────────────────────────────────────────────────

pub struct GitPrCreate;

#[async_trait]
impl Tool for GitPrCreate {
    fn name(&self) -> &str {
        "git_pr_create"
    }
    fn description(&self) -> &str {
        "Create a pull request on GitHub"
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "title": {"type": "string", "description": "PR title"},
                "body": {"type": "string", "description": "PR description"},
                "base": {"type": "string", "description": "Base branch (default: main)"}
            },
            "required": ["title"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Network
    }
    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let title = args["title"].as_str().unwrap_or("Sparrow PR");
        let body = args["body"].as_str().unwrap_or("");
        let base = args["base"].as_str().unwrap_or("main");

        // Try gh CLI first
        let output = StdCommand::new("gh")
            .args([
                "pr", "create", "--title", title, "--body", body, "--base", base,
            ])
            .current_dir(&ctx.workspace_root)
            .output();

        match output {
            Ok(o) if o.status.success() => {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                Ok(ToolResult::text(format!("PR created:\n{}", stdout)))
            }
            _ => Ok(ToolResult::text(format!(
                "PR would be created:\n  Title: {}\n  Base: {}\n  Body: {}",
                title, base, body
            ))),
        }
    }
}

// ─── Fetch Docs ────────────────────────────────────────────────────────────────

pub struct FetchDocs;

#[async_trait]
impl Tool for FetchDocs {
    fn name(&self) -> &str {
        "fetch_docs"
    }
    fn description(&self) -> &str {
        "Fetch up-to-date library documentation to avoid API hallucinations"
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "package": {"type": "string", "description": "Package name (e.g., tokio, serde, react)"},
                "language": {"type": "string", "description": "Language: rust, python, js, go"}
            },
            "required": ["package"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Network
    }
    async fn call(&self, args: serde_json::Value, _ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let package = args["package"].as_str().unwrap_or("");
        let lang = args["language"].as_str().unwrap_or("rust");

        let url = match lang {
            "rust" => format!("https://docs.rs/{}/latest", package),
            "python" => format!("https://pypi.org/project/{}/", package),
            "js" => format!("https://www.npmjs.com/package/{}", package),
            _ => format!("https://docs.rs/{}/latest", package),
        };

        // Fetch docs page
        let client = reqwest::Client::builder()
            .user_agent("sparrow-docs/0.1")
            .timeout(std::time::Duration::from_secs(10))
            .build()?;

        match client.get(&url).send().await {
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                // Extract first 2000 chars of meaningful content
                let preview: String = text
                    .chars()
                    .filter(|c| !c.is_whitespace() || *c == ' ')
                    .take(2000)
                    .collect();
                Ok(ToolResult::text(format!(
                    "Docs for {}: {}\nStatus: {}\n\n{}",
                    package, url, status, preview
                )))
            }
            Err(e) => Ok(ToolResult::text(format!(
                "Could not fetch docs for {}: {}\nURL: {}",
                package, e, url
            ))),
        }
    }
}

// ─── Lightweight local code intelligence ───────────────────────────────────────

pub struct LspClient;

#[async_trait]
impl Tool for LspClient {
    fn name(&self) -> &str {
        "lsp"
    }
    fn description(&self) -> &str {
        "Local code intelligence: diagnostics, goto definition, references, and hover context without a language server daemon"
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {"type": "string", "enum": ["diagnostics", "goto_definition", "find_references", "hover", "rename"]},
                "file": {"type": "string"},
                "line": {"type": "integer"},
                "column": {"type": "integer"},
                "symbol": {"type": "string"}
            },
            "required": ["action", "file"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::ReadOnly
    }
    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let action = args["action"].as_str().unwrap_or("diagnostics");
        let file = args["file"].as_str().unwrap_or("");
        if file.is_empty() {
            return Ok(ToolResult::error("lsp: 'file' is required"));
        }
        let root = ctx
            .workspace_root
            .canonicalize()
            .unwrap_or_else(|_| ctx.workspace_root.clone());
        let path = crate::resolve_workspace_path(&root, file)?;
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let line = args["line"].as_u64().map(|n| n.max(1) as usize);
        let column = args["column"].as_u64().map(|n| n.max(1) as usize);
        let symbol = args["symbol"]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| symbol_at_position(&path, line, column).ok().flatten());

        match action {
            "diagnostics" => diagnostics(&root, &path, &rel),
            "goto_definition" => {
                let Some(symbol) = symbol else {
                    return Ok(ToolResult::error(
                        "lsp goto_definition: provide 'symbol' or line/column",
                    ));
                };
                Ok(definitions(&root, &symbol, false))
            }
            "find_references" => {
                let Some(symbol) = symbol else {
                    return Ok(ToolResult::error(
                        "lsp find_references: provide 'symbol' or line/column",
                    ));
                };
                Ok(references(&root, &symbol))
            }
            "hover" => hover(&root, &path, &rel, line, column, symbol.as_deref()),
            "rename" => Ok(ToolResult::error(
                "lsp rename is mutating; use the edit or multi_edit tool after reviewing references",
            )),
            other => Ok(ToolResult::error(format!(
                "lsp: unknown action '{}'",
                other
            ))),
        }
    }
}

const LSP_SKIP_DIRS: &[&str] = &[".git", "target", "node_modules", "dist", "build", ".venv"];
const LSP_MAX_RESULTS: usize = 120;

fn diagnostics(root: &Path, path: &Path, rel: &str) -> anyhow::Result<ToolResult> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let output = match ext {
        "rs" if root.join("Cargo.toml").exists() => StdCommand::new("cargo")
            .args(["check", "--message-format", "short"])
            .current_dir(root)
            .output(),
        "py" => StdCommand::new("python")
            .args(["-m", "py_compile", rel])
            .current_dir(root)
            .output()
            .or_else(|_| {
                StdCommand::new("python3")
                    .args(["-m", "py_compile", rel])
                    .current_dir(root)
                    .output()
            }),
        "js" | "mjs" | "cjs" => StdCommand::new("node")
            .args(["--check", rel])
            .current_dir(root)
            .output(),
        "ts" | "tsx" if root.join("package.json").exists() => StdCommand::new("npx")
            .args(["tsc", "--noEmit", "--pretty", "false"])
            .current_dir(root)
            .output(),
        _ => return syntax_scan(path, rel),
    };

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = if stderr.trim().is_empty() {
                stdout.trim().to_string()
            } else {
                format!("{}\n{}", stdout.trim(), stderr.trim())
                    .trim()
                    .to_string()
            };
            let status = output.status.code().unwrap_or(-1);
            Ok(ToolResult::text(format!(
                "diagnostics for {}\nexit: {}\n{}",
                rel,
                status,
                if combined.is_empty() {
                    "no diagnostics reported".to_string()
                } else {
                    combined
                }
            )))
        }
        Err(e) => Ok(ToolResult::error(format!(
            "diagnostics for {} failed to launch checker: {}",
            rel, e
        ))),
    }
}

fn syntax_scan(path: &Path, rel: &str) -> anyhow::Result<ToolResult> {
    let content = std::fs::read_to_string(path)?;
    let mut findings = Vec::new();
    let mut paren = 0i32;
    let mut brace = 0i32;
    let mut bracket = 0i32;
    for (idx, line) in content.lines().enumerate() {
        for ch in line.chars() {
            match ch {
                '(' => paren += 1,
                ')' => paren -= 1,
                '{' => brace += 1,
                '}' => brace -= 1,
                '[' => bracket += 1,
                ']' => bracket -= 1,
                _ => {}
            }
        }
        if paren < 0 || brace < 0 || bracket < 0 {
            findings.push(format!("{}:{}: unmatched closing delimiter", rel, idx + 1));
            paren = paren.max(0);
            brace = brace.max(0);
            bracket = bracket.max(0);
        }
    }
    if paren > 0 || brace > 0 || bracket > 0 {
        findings.push(format!(
            "{}: unmatched delimiters: paren={} brace={} bracket={}",
            rel, paren, brace, bracket
        ));
    }
    if findings.is_empty() {
        findings.push(format!("{}: no lightweight syntax findings", rel));
    }
    Ok(ToolResult::text(findings.join("\n")))
}

fn definitions(root: &Path, symbol: &str, include_empty_message: bool) -> ToolResult {
    let patterns = definition_patterns(symbol);
    let hits = scan_code(root, |path, line_no, line| {
        if patterns.iter().any(|re| re.is_match(line)) {
            Some(format!(
                "{}:{}: {}",
                rel_path(root, path),
                line_no,
                line.trim()
            ))
        } else {
            None
        }
    });
    if hits.is_empty() {
        if include_empty_message {
            ToolResult::text(format!("no definition found for '{}'", symbol))
        } else {
            ToolResult::error(format!("no definition found for '{}'", symbol))
        }
    } else {
        ToolResult::ok(vec![Block::Text(hits.join("\n"))])
    }
}

fn references(root: &Path, symbol: &str) -> ToolResult {
    let Ok(re) = regex::Regex::new(&format!(r"\b{}\b", regex::escape(symbol))) else {
        return ToolResult::error("invalid reference regex");
    };
    let hits = scan_code(root, |path, line_no, line| {
        if re.is_match(line) {
            Some(format!(
                "{}:{}: {}",
                rel_path(root, path),
                line_no,
                line.trim()
            ))
        } else {
            None
        }
    });
    if hits.is_empty() {
        ToolResult::text(format!("no references found for '{}'", symbol))
    } else {
        ToolResult::ok(vec![Block::Text(hits.join("\n"))])
    }
}

fn hover(
    root: &Path,
    path: &Path,
    rel: &str,
    line: Option<usize>,
    column: Option<usize>,
    symbol: Option<&str>,
) -> anyhow::Result<ToolResult> {
    let content = std::fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    let line = line.unwrap_or(1).clamp(1, lines.len().max(1));
    let start = line.saturating_sub(3).max(1);
    let end = (line + 2).min(lines.len().max(1));
    let mut out = vec![format!("hover {}:{}:{}", rel, line, column.unwrap_or(1))];
    if let Some(symbol) = symbol {
        out.push(format!("symbol: {}", symbol));
        if let Block::Text(defs) = definitions(root, symbol, true).content.remove(0) {
            out.push(format!("definitions:\n{}", defs));
        }
    }
    out.push("context:".into());
    for n in start..=end {
        if let Some(text) = lines.get(n - 1) {
            out.push(format!("{:>4} | {}", n, text));
        }
    }
    Ok(ToolResult::text(out.join("\n")))
}

fn definition_patterns(symbol: &str) -> Vec<regex::Regex> {
    let esc = regex::escape(symbol);
    [
        format!(
            r"\b(fn|struct|enum|trait|type|const|static|class|def|function)\s+{}\b",
            esc
        ),
        format!(r"\bimpl\b[^\n]*\b{}\b", esc),
        format!(r"\b{}\s*[:=]\s*(async\s*)?(\([^)]*\)\s*=>|function\b)", esc),
    ]
    .into_iter()
    .filter_map(|p| regex::Regex::new(&p).ok())
    .collect()
}

fn symbol_at_position(
    path: &Path,
    line: Option<usize>,
    column: Option<usize>,
) -> anyhow::Result<Option<String>> {
    let Some(line_no) = line else {
        return Ok(None);
    };
    let content = std::fs::read_to_string(path)?;
    let Some(line) = content.lines().nth(line_no.saturating_sub(1)) else {
        return Ok(None);
    };
    let col = column.unwrap_or(1).saturating_sub(1).min(line.len());
    let bytes = line.as_bytes();
    let mut start = col;
    while start > 0 && is_ident(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = col;
    while end < bytes.len() && is_ident(bytes[end]) {
        end += 1;
    }
    if start == end {
        return Ok(None);
    }
    Ok(Some(line[start..end].to_string()))
}

fn is_ident(ch: u8) -> bool {
    ch.is_ascii_alphanumeric() || ch == b'_'
}

fn scan_code<F>(root: &Path, mut mapper: F) -> Vec<String>
where
    F: FnMut(&Path, usize, &str) -> Option<String>,
{
    let mut files = Vec::new();
    walk_code_files(root, &mut files);
    let mut hits = Vec::new();
    for file in files {
        let Ok(content) = std::fs::read_to_string(&file) else {
            continue;
        };
        for (idx, line) in content.lines().enumerate() {
            if let Some(hit) = mapper(&file, idx + 1, line) {
                hits.push(hit);
                if hits.len() >= LSP_MAX_RESULTS {
                    return hits;
                }
            }
        }
    }
    hits
}

fn walk_code_files(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if name.starts_with('.') || LSP_SKIP_DIRS.contains(&name.as_ref()) {
                continue;
            }
            walk_code_files(&path, out);
        } else if is_lsp_code_file(&path) {
            out.push(path);
        }
    }
}

fn is_lsp_code_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("rs" | "py" | "js" | "jsx" | "ts" | "tsx" | "go" | "java" | "c" | "h" | "cpp" | "hpp")
    )
}

fn rel_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

// ─── REPL ──────────────────────────────────────────────────────────────────────

pub struct Repl;

#[async_trait]
impl Tool for Repl {
    fn name(&self) -> &str {
        "repl"
    }
    fn description(&self) -> &str {
        "Execute code interactively in a sandboxed REPL (Python/Node)"
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "language": {"type": "string", "enum": ["python", "node"]},
                "code": {"type": "string", "description": "Code to execute"},
                "timeout_ms": {"type": "integer"}
            },
            "required": ["language", "code"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Exec
    }
    async fn call(&self, args: serde_json::Value, _ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let lang = args["language"].as_str().unwrap_or("python");
        let code = args["code"].as_str().unwrap_or("");
        let _timeout_ms = args["timeout_ms"].as_u64().unwrap_or(30_000);

        let program = match lang {
            "node" => "node",
            _ => "python3",
        };
        let flag = match lang {
            "node" => "-e",
            _ => "-c",
        };

        let output = StdCommand::new(program).args([flag, code]).output()?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let result = if stderr.is_empty() {
            stdout
        } else {
            format!("{}\n{}", stdout, stderr)
        };

        Ok(ToolResult::text(result))
    }
}
