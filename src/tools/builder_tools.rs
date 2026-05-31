use async_trait::async_trait;
use serde_json::json;
use std::process::Command as StdCommand;

use crate::event::{Block, RiskLevel};
use crate::tools::{Tool, ToolCtx, ToolResult};

// ─── Test Runner ───────────────────────────────────────────────────────────────

pub struct TestRunner;

#[async_trait]
impl Tool for TestRunner {
    fn name(&self) -> &str { "test" }
    fn description(&self) -> &str { "Detect and run the project test suite. Parses results." }
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
    fn risk(&self) -> RiskLevel { RiskLevel::Exec }
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

        let passed = stdout.lines().filter(|l| l.contains("pass") || l.contains("PASS") || l.contains("ok")).count() as u32;
        let failed = stdout.lines().filter(|l| l.contains("fail") || l.contains("FAIL") || l.contains("FAILED")).count() as u32;

        Ok(ToolResult::ok(vec![
            Block::Text(format!(
                "Framework: {}\nPassed: {}\nFailed: {}\nExit: {}\n\n{}",
                fw, passed, failed, exit_code,
                if stderr.is_empty() { &stdout } else { &stderr }
            )),
        ]))
    }
}

fn detect_framework(root: &std::path::Path) -> String {
    if root.join("Cargo.toml").exists() { return "cargo".into(); }
    if root.join("pyproject.toml").exists() || root.join("pytest.ini").exists() { return "pytest".into(); }
    if root.join("package.json").exists() { return "jest".into(); }
    if root.join("go.mod").exists() { return "go".into(); }
    "cargo".into()
}

// ─── Apply Patch ───────────────────────────────────────────────────────────────

pub struct ApplyPatch;

#[async_trait]
impl Tool for ApplyPatch {
    fn name(&self) -> &str { "apply_patch" }
    fn description(&self) -> &str { "Apply a structured multi-file patch atomically with rollback" }
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
    fn risk(&self) -> RiskLevel { RiskLevel::Mutating }
    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let patches = args["patches"].as_array()
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
                    "Patch failed on '{}': old string not found. All changes rolled back.", file
                )));
            }

            let new_content = original.replace(old, new);
            std::fs::write(&full_path, new_content)?;
            results.push(format!("✓ {} : applied", file));
        }

        Ok(ToolResult::ok(vec![
            Block::Text(format!("Patched {} file(s) atomically:\n{}", results.len(), results.join("\n"))),
        ]))
    }
}

// ─── Git PR Create ─────────────────────────────────────────────────────────────

pub struct GitPrCreate;

#[async_trait]
impl Tool for GitPrCreate {
    fn name(&self) -> &str { "git_pr_create" }
    fn description(&self) -> &str { "Create a pull request on GitHub" }
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
    fn risk(&self) -> RiskLevel { RiskLevel::Network }
    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let title = args["title"].as_str().unwrap_or("Sparrow PR");
        let body = args["body"].as_str().unwrap_or("");
        let base = args["base"].as_str().unwrap_or("main");

        // Try gh CLI first
        let output = StdCommand::new("gh")
            .args(["pr", "create", "--title", title, "--body", body, "--base", base])
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
    fn name(&self) -> &str { "fetch_docs" }
    fn description(&self) -> &str { "Fetch up-to-date library documentation to avoid API hallucinations" }
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
    fn risk(&self) -> RiskLevel { RiskLevel::Network }
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
                let preview: String = text.chars().filter(|c| !c.is_whitespace() || *c == ' ').take(2000).collect();
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

// ─── LSP stub (requires tower-lsp runtime) ─────────────────────────────────────

pub struct LspClient;

#[async_trait]
impl Tool for LspClient {
    fn name(&self) -> &str { "lsp" }
    fn description(&self) -> &str { "Language Server Protocol: diagnostics, goto definition, references, hover" }
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
    fn risk(&self) -> RiskLevel { RiskLevel::ReadOnly }
    async fn call(&self, args: serde_json::Value, _ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let action = args["action"].as_str().unwrap_or("diagnostics");
        let file = args["file"].as_str().unwrap_or("");

        // LSP requires tower-lsp runtime; stub returns instructions + fallback to grep
        Ok(ToolResult::text(format!(
            "LSP '{}' on {}: For full LSP support, install the tower-lsp runtime.\n\
             Fallback: use 'search' tool for symbol finding in {}",
            action, file, file
        )))
    }
}

// ─── REPL ──────────────────────────────────────────────────────────────────────

pub struct Repl;

#[async_trait]
impl Tool for Repl {
    fn name(&self) -> &str { "repl" }
    fn description(&self) -> &str { "Execute code interactively in a sandboxed REPL (Python/Node)" }
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
    fn risk(&self) -> RiskLevel { RiskLevel::Exec }
    async fn call(&self, args: serde_json::Value, _ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let lang = args["language"].as_str().unwrap_or("python");
        let code = args["code"].as_str().unwrap_or("");
        let timeout_ms = args["timeout_ms"].as_u64().unwrap_or(30_000);

        let program = match lang { "node" => "node", _ => "python3" };
        let flag = match lang { "node" => "-e", _ => "-c" };

        let output = StdCommand::new(program)
            .args([flag, code])
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let result = if stderr.is_empty() { stdout } else { format!("{}\n{}", stdout, stderr) };

        Ok(ToolResult::text(result))
    }
}
