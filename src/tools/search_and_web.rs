use async_trait::async_trait;
use serde_json::json;

use super::{Tool, ToolCtx, ToolResult, resolve_workspace_path};
use crate::event::{Block, RiskLevel};

// ─── Ripgrep search ─────────────────────────────────────────────────────────────

pub struct Search;

#[async_trait]
impl Tool for Search {
    fn name(&self) -> &str {
        "search"
    }
    fn description(&self) -> &str {
        "Search code in the workspace using ripgrep (regex patterns)"
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Regex pattern to search for" },
                "path": { "type": "string", "description": "Directory or file to search (default: workspace root)" },
                "include": { "type": "string", "description": "File pattern filter (e.g. '*.rs')" },
                "max_results": { "type": "integer", "description": "Max results (default: 50)" }
            },
            "required": ["pattern"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::ReadOnly
    }
    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let pattern = args["pattern"].as_str().unwrap_or("");
        let path = args["path"].as_str().unwrap_or(".");
        let include = args["include"].as_str();
        let max_results = args["max_results"].as_u64().unwrap_or(50) as usize;

        let search_path = resolve_workspace_path(&ctx.workspace_root, path)?;

        let mut cmd = std::process::Command::new("rg");
        cmd.arg("--line-number")
            .arg("--no-heading")
            .arg("--color=never")
            .arg("-M")
            .arg(max_results.to_string())
            .arg(pattern)
            .arg(&search_path);

        if let Some(inc) = include {
            cmd.arg("--glob").arg(inc);
        }

        match cmd.output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                if stdout.is_empty() {
                    Ok(ToolResult::text("No matches found."))
                } else {
                    Ok(ToolResult::text(stdout))
                }
            }
            Err(e) => {
                // Fallback: basic string search
                if e.kind() == std::io::ErrorKind::NotFound {
                    let mut results = Vec::new();
                    basic_grep(&search_path, pattern, include, &mut results, 0, max_results)?;
                    if results.is_empty() {
                        Ok(ToolResult::text(
                            "No matches found (rg not installed, used basic search).",
                        ))
                    } else {
                        Ok(ToolResult::text(results.join("\n")))
                    }
                } else {
                    Err(e.into())
                }
            }
        }
    }
}

fn basic_grep(
    dir: &std::path::Path,
    pattern: &str,
    include: Option<&str>,
    results: &mut Vec<String>,
    depth: usize,
    max: usize,
) -> std::io::Result<()> {
    if depth > 10 || results.len() >= max {
        return Ok(());
    }
    if dir.is_dir() {
        let entries = std::fs::read_dir(dir)?;
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            if path.is_dir() {
                basic_grep(&path, pattern, include, results, depth + 1, max)?;
            } else if path.is_file() {
                if let Some(inc) = include {
                    if !name.contains(inc) && !inc.contains('*') {
                        continue;
                    }
                }
                if results.len() >= max {
                    break;
                }
                if let Ok(content) = std::fs::read_to_string(&path) {
                    for (i, line) in content.lines().enumerate() {
                        if results.len() >= max {
                            break;
                        }
                        if line.to_lowercase().contains(&pattern.to_lowercase()) {
                            let rel = path.strip_prefix(dir).unwrap_or(&path).display();
                            results.push(format!("{}:{}: {}", rel, i + 1, line.trim()));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

// ─── Web search ─────────────────────────────────────────────────────────────────

pub struct WebSearch;

#[async_trait]
impl Tool for WebSearch {
    fn name(&self) -> &str {
        "web_search"
    }
    fn description(&self) -> &str {
        "Search the web for information"
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query" },
                "num_results": { "type": "integer", "description": "Number of results (default: 5)" }
            },
            "required": ["query"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Network
    }
    async fn call(&self, args: serde_json::Value, _ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let query = args["query"].as_str().unwrap_or("");
        let num = args["num_results"].as_u64().unwrap_or(5);

        let client = reqwest::Client::builder()
            .user_agent("sparrow/0.1")
            .build()?;

        // Use DuckDuckGo Lite as a free search backend
        let resp = client
            .get("https://lite.duckduckgo.com/lite/")
            .query(&[("q", query)])
            .send()
            .await?;

        let html = resp.text().await?;

        // Simple extraction of result snippets
        let mut results = Vec::new();
        for line in html.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("<a") && trimmed.contains("class=\"result-link\"") {
                if let Some(url) = extract_href(trimmed) {
                    if results.len() < num as usize {
                        results.push(format!("🔗 {}", url));
                    }
                }
            }
            if trimmed.starts_with("<td") && trimmed.contains("class=\"result-snippet\"") {
                let snippet = strip_html(trimmed);
                if !snippet.is_empty() && results.len() <= num as usize {
                    results.push(format!("   {}", snippet));
                }
            }
        }

        if results.is_empty() {
            Ok(ToolResult::text(format!(
                "No web results for: {}. Try a more specific query.",
                query
            )))
        } else {
            Ok(ToolResult::text(results.join("\n")))
        }
    }
}

fn extract_href(line: &str) -> Option<String> {
    let start = line.find("href=\"")? + 6;
    let end = line[start..].find('"')?;
    let mut url = line[start..start + end].to_string();
    // Clean up DuckDuckGo redirect URLs
    if url.starts_with("//") {
        url = format!("https:{}", url);
    }
    if url.contains("duckduckgo.com/l/?uddg=") {
        if let Some(real) = url.split("uddg=").nth(1) {
            if let Ok(decoded) = urlencoding(&real) {
                url = decoded;
            }
        }
    }
    Some(url)
}

fn strip_html(s: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(c);
        }
    }
    result.trim().to_string()
}

fn urlencoding(s: &str) -> Result<String, ()> {
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%' && i + 2 < chars.len() {
            let hex = &s[i + 1..i + 3];
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                result.push(byte as char);
                i += 3;
                continue;
            }
        }
        if chars[i] == '+' {
            result.push(' ');
        } else {
            result.push(chars[i]);
        }
        i += 1;
    }
    Ok(result)
}

// ─── Web fetch ──────────────────────────────────────────────────────────────────

pub struct WebFetch;

#[async_trait]
impl Tool for WebFetch {
    fn name(&self) -> &str {
        "web_fetch"
    }
    fn description(&self) -> &str {
        "Fetch and read content from a URL"
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "URL to fetch" },
                "format": { "type": "string", "enum": ["text", "markdown", "html"], "description": "Output format (default: text)" }
            },
            "required": ["url"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Network
    }
    async fn call(&self, args: serde_json::Value, _ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let url = args["url"].as_str().unwrap_or("");
        let format = args["format"].as_str().unwrap_or("text");

        let client = reqwest::Client::builder()
            .user_agent("sparrow/0.1")
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        let resp = client.get(url).send().await?;
        let status = resp.status();
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_string();

        let bytes = resp.bytes().await?;

        let text = match format {
            "html" => String::from_utf8_lossy(&bytes).to_string(),
            _ => {
                // Simple HTML to text conversion
                let raw = String::from_utf8_lossy(&bytes).to_string();
                let stripped = strip_html(&raw);
                // Truncate very long content
                if stripped.len() > 50_000 {
                    format!(
                        "{}...\n\n[truncated: {} bytes total]",
                        &stripped[..50_000],
                        stripped.len()
                    )
                } else {
                    stripped
                }
            }
        };

        Ok(ToolResult::ok(vec![Block::Text(format!(
            "URL: {}\nStatus: {}\nType: {}\n\n{}",
            url, status, content_type, text
        ))]))
    }
}
