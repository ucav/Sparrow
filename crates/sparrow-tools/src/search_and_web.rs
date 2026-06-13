use async_trait::async_trait;
use serde_json::json;

use super::{Tool, ToolCtx, ToolResult, resolve_workspace_path};
use sparrow_core::event::{Block, RiskLevel};

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

/// Reject non-http(s) schemes and any URL whose host resolves to a private,
/// loopback, link-local, multicast, or unspecified address (SSRF defence).
/// Also rejects bare IPs in those ranges and the AWS/GCP metadata endpoints.
pub(crate) fn validate_public_url(url: &str) -> Result<(), &'static str> {
    resolve_public_url(url).map(|_| ())
}

/// Like [`validate_public_url`], but on success also returns the validated
/// socket addresses for the host.
///
/// Callers should pin their HTTP client to exactly these addresses (e.g. via
/// `reqwest::ClientBuilder::resolve_to_addrs`). Validating the hostname and
/// then letting the client re-resolve at connect time leaves a DNS-rebinding
/// TOCTOU window: an attacker domain can answer with a public IP during this
/// check and `127.0.0.1` (or `169.254.169.254`) microseconds later when the
/// request actually dials. Pinning closes that window — we connect only to the
/// IPs we just screened.
///
/// For bare-IP literals there is nothing to rebind, so the returned vec simply
/// carries that single address. When a hostname cannot be resolved here we
/// return `Ok(vec![])` rather than failing: resolution may be transient and the
/// client will surface a real connection error — but with an empty pin set the
/// caller MUST fall back to its own (still redirect-guarded) resolution.
pub(crate) fn resolve_public_url(url: &str) -> Result<Vec<std::net::SocketAddr>, &'static str> {
    let parsed = url::Url::parse(url).map_err(|_| "invalid URL")?;
    match parsed.scheme() {
        "http" | "https" => {}
        _ => return Err("only http(s) is allowed"),
    }
    let host = parsed.host_str().ok_or("missing host")?;

    // Block obvious well-known metadata / loopback hostnames.
    let lc = host.to_ascii_lowercase();
    if matches!(
        lc.as_str(),
        "localhost" | "ip6-localhost" | "ip6-loopback" | "metadata.google.internal" | "metadata"
    ) || lc.ends_with(".localhost")
        || lc.ends_with(".local")
        || lc.ends_with(".internal")
    {
        return Err("host points to local/internal network");
    }

    let port = parsed.port_or_known_default().unwrap_or(0);

    // If the host parses as an IP literal, check the ranges directly.
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        if is_blocked_ip(&ip) {
            return Err("IP belongs to a private/loopback/link-local range");
        }
        return Ok(vec![std::net::SocketAddr::new(ip, port)]);
    }

    // Hostname: best-effort DNS check. We can't await here without making the
    // fn async, so we resolve synchronously via the std API. A single resolution
    // is cheap and prevents the most common SSRF payloads (`127.0.0.1`-aliasing
    // domains, hosts file tricks, etc.). Every returned address is screened, and
    // the screened set is handed back so the caller can pin to it.
    let mut validated = Vec::new();
    if let Ok(addrs) = std::net::ToSocketAddrs::to_socket_addrs(&(host, port)) {
        for sa in addrs {
            if is_blocked_ip(&sa.ip()) {
                return Err("hostname resolves to a private/loopback IP");
            }
            validated.push(sa);
        }
    }
    Ok(validated)
}

fn is_blocked_ip(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_multicast()
                || v4.is_unspecified()
                || v4.octets() == [169, 254, 169, 254] // AWS/GCP/Azure metadata
                // Carrier-grade NAT 100.64.0.0/10
                || (v4.octets()[0] == 100 && (v4.octets()[1] & 0xC0) == 0x40)
        }
        std::net::IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                // fc00::/7 (unique local), fe80::/10 (link-local)
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
                // IPv4-mapped: re-check the embedded v4
                || v6.to_ipv4_mapped().map(|m| is_blocked_ip(&std::net::IpAddr::V4(m))).unwrap_or(false)
        }
    }
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

        let validated_addrs = match resolve_public_url(url) {
            Ok(addrs) => addrs,
            Err(why) => return Ok(ToolResult::error(format!("Refused URL ({}): {}", why, url))),
        };

        let mut builder = reqwest::Client::builder()
            .user_agent("sparrow/0.1")
            .timeout(std::time::Duration::from_secs(30))
            // Belt-and-suspenders: re-validate after redirects to block private-IP redirect attacks.
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if validate_public_url(attempt.url().as_str()).is_err() {
                    attempt.stop()
                } else if attempt.previous().len() >= 5 {
                    attempt.stop()
                } else {
                    attempt.follow()
                }
            }));

        // Pin the connection to the exact IPs we just screened so a rebinding
        // DNS server can't swap in a loopback/metadata address between the check
        // above and the actual dial. Only applies when the host is a name we
        // resolved here; bare-IP literals and unresolvable names fall through to
        // reqwest's own resolver (still covered by the redirect guard).
        if let Some(host) = url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_owned)) {
            if !validated_addrs.is_empty() {
                builder = builder.resolve_to_addrs(&host, &validated_addrs);
            }
        }

        let client = builder.build()?;

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

#[cfg(test)]
mod ssrf_tests {
    use super::{resolve_public_url, validate_public_url};

    #[test]
    fn rejects_non_http_schemes() {
        assert!(validate_public_url("file:///etc/passwd").is_err());
        assert!(validate_public_url("ftp://example.com/x").is_err());
        assert!(validate_public_url("gopher://example.com").is_err());
    }

    #[test]
    fn rejects_loopback_and_metadata_literals() {
        for url in [
            "http://127.0.0.1/",
            "http://127.0.0.1:8080/admin",
            "http://[::1]/",
            "http://169.254.169.254/latest/meta-data/",
            "http://0.0.0.0/",
            "http://10.0.0.5/",
            "http://192.168.1.1/",
            "http://172.16.4.2/",
            // Carrier-grade NAT 100.64.0.0/10
            "http://100.64.1.1/",
        ] {
            assert!(validate_public_url(url).is_err(), "should reject {url}");
        }
    }

    #[test]
    fn rejects_local_hostnames() {
        for url in [
            "http://localhost/",
            "http://foo.localhost/",
            "http://metadata.google.internal/",
            "http://db.internal/",
            "http://printer.local/",
        ] {
            assert!(validate_public_url(url).is_err(), "should reject {url}");
        }
    }

    #[test]
    fn public_ip_literal_pins_to_itself() {
        // A public IP literal needs no DNS and cannot be rebound: the pin set is
        // exactly that address.
        let addrs = resolve_public_url("http://93.184.216.34/").expect("public IP allowed");
        assert_eq!(addrs.len(), 1);
        assert_eq!(addrs[0].ip().to_string(), "93.184.216.34");
        assert_eq!(addrs[0].port(), 80);
    }

    #[test]
    fn https_default_port_is_443() {
        let addrs = resolve_public_url("https://8.8.8.8/").expect("public IP allowed");
        assert_eq!(addrs[0].port(), 443);
    }
}
