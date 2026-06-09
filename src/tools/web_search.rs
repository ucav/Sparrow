//! Web search tool — DuckDuckGo-based search with structured results.

use std::process::Command;

/// Search the web and return results.
pub fn web_search(query: &str, max_results: usize) -> anyhow::Result<Vec<SearchResult>> {
    // Try DuckDuckGo via curl (no API key needed)
    let output = Command::new("curl")
        .args([
            "-s",
            "-A",
            "sparrow/0.5",
            "--max-time",
            "10",
            &format!(
                "https://api.duckduckgo.com/?q={}&format=json&no_html=1",
                urlencoding(query)
            ),
        ])
        .output()?;

    if output.status.success() {
        let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        return parse_duckduckgo(&json, max_results);
    }

    // Fallback: try SearXNG (self-hosted, no API key)
    if let Ok(output) = Command::new("curl")
        .args([
            "-s",
            "--max-time",
            "10",
            &format!(
                "https://searx.be/search?q={}&format=json",
                urlencoding(query)
            ),
        ])
        .output()
    {
        if output.status.success() {
            let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
            return parse_searx(&json, max_results);
        }
    }

    // Last resort: suggest manual search
    Ok(vec![SearchResult {
        title: format!("Search: {}", query),
        url: format!("https://duckduckgo.com/?q={}", urlencoding(query)),
        snippet: "No results. Open the URL in your browser to search manually.".into(),
    }])
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

fn parse_duckduckgo(json: &serde_json::Value, max: usize) -> anyhow::Result<Vec<SearchResult>> {
    let mut results = Vec::new();
    if let Some(items) = json.get("RelatedTopics").and_then(|t| t.as_array()) {
        for item in items.iter().take(max) {
            if let (Some(text), Some(url)) = (
                item.get("Text").and_then(|t| t.as_str()),
                item.get("FirstURL").and_then(|u| u.as_str()),
            ) {
                results.push(SearchResult {
                    title: text.chars().take(80).collect(),
                    url: url.to_string(),
                    snippet: text.to_string(),
                });
            }
        }
    }
    Ok(results)
}

fn parse_searx(json: &serde_json::Value, max: usize) -> anyhow::Result<Vec<SearchResult>> {
    let mut results = Vec::new();
    if let Some(items) = json.get("results").and_then(|r| r.as_array()) {
        for item in items.iter().take(max) {
            results.push(SearchResult {
                title: item
                    .get("title")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string(),
                url: item
                    .get("url")
                    .and_then(|u| u.as_str())
                    .unwrap_or("")
                    .to_string(),
                snippet: item
                    .get("content")
                    .or(item.get("snippet"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_string(),
            });
        }
    }
    Ok(results)
}

fn urlencoding(s: &str) -> String {
    s.replace(' ', "+")
        .replace('&', "%26")
        .replace('=', "%3D")
        .replace('#', "%23")
}
