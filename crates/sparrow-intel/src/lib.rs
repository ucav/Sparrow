use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest as ShaDigest, Sha256};
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    GithubReleases,
    ChangelogUrl,
    DocsUrl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceConfig {
    pub name: String,
    pub kind: SourceKind,
    pub url: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcesFile {
    #[serde(default)]
    pub source: Vec<SourceConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelItem {
    pub source: String,
    pub version: String,
    pub date: String,
    pub body: String,
    pub url: String,
    pub etag: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelDigest {
    pub source: String,
    pub title: String,
    pub summary: String,
    pub version: String,
    pub date: String,
    pub url: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacklogTicket {
    pub title: String,
    pub source: String,
    pub score: u32,
    pub reason: String,
    pub url: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanReport {
    pub scanned: usize,
    pub inserted_or_updated: usize,
    pub items: Vec<IntelItem>,
}

pub fn load_sources_file(path: &Path) -> Result<Vec<SourceConfig>> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("could not read intel sources file {}", path.display()))?;
    let parsed: SourcesFile = toml::from_str(&raw)
        .with_context(|| format!("could not parse intel sources file {}", path.display()))?;
    Ok(parsed.source)
}

pub struct IntelCache {
    path: std::path::PathBuf,
}

impl IntelCache {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let cache = Self { path };
        cache.init()?;
        Ok(cache)
    }

    fn conn(&self) -> Result<Connection> {
        Ok(Connection::open(&self.path)?)
    }

    fn init(&self) -> Result<()> {
        let conn = self.conn()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS intel_items (
                source TEXT NOT NULL,
                version TEXT NOT NULL,
                date TEXT NOT NULL,
                body TEXT NOT NULL,
                url TEXT NOT NULL,
                etag TEXT,
                tags TEXT NOT NULL DEFAULT '[]',
                updated_at INTEGER NOT NULL,
                UNIQUE(source, version, url)
            );",
        )?;
        Ok(())
    }

    pub fn upsert_items(&self, items: &[IntelItem]) -> Result<usize> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let mut changed = 0usize;
        for item in items {
            let tags = serde_json::to_string(&item.tags)?;
            changed += tx.execute(
                "INSERT INTO intel_items (source, version, date, body, url, etag, tags, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, strftime('%s','now'))
                 ON CONFLICT(source, version, url) DO UPDATE SET
                   date=excluded.date,
                   body=excluded.body,
                   etag=excluded.etag,
                   tags=excluded.tags,
                   updated_at=excluded.updated_at",
                params![
                    item.source,
                    item.version,
                    item.date,
                    item.body,
                    item.url,
                    item.etag,
                    tags
                ],
            )?;
        }
        tx.commit()?;
        Ok(changed)
    }

    pub fn items(&self, limit: usize) -> Result<Vec<IntelItem>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT source, version, date, body, url, etag, tags
             FROM intel_items
             ORDER BY date DESC, updated_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], row_to_item)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn digests(&self, limit: usize) -> Result<Vec<IntelDigest>> {
        Ok(self
            .items(limit)?
            .into_iter()
            .map(|item| IntelDigest {
                title: digest_title(&item),
                summary: summarize(&item.body, 260),
                version: item.version,
                date: item.date,
                url: item.url,
                source: item.source,
                tags: item.tags,
            })
            .collect())
    }

    pub fn backlog(&self, limit: usize) -> Result<Vec<BacklogTicket>> {
        let mut tickets: Vec<_> = self
            .digests(limit.saturating_mul(3).max(limit))?
            .into_iter()
            .map(|digest| {
                let (score, reason) = score_digest(&digest);
                BacklogTicket {
                    title: format!("Évaluer {} {}", digest.source, digest.version),
                    source: digest.source,
                    score,
                    reason,
                    url: digest.url,
                    tags: digest.tags,
                }
            })
            .filter(|t| t.score > 0)
            .collect();
        tickets.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.title.cmp(&b.title)));
        tickets.truncate(limit);
        Ok(tickets)
    }

    pub fn etag_for_url(&self, url: &str) -> Result<Option<String>> {
        let conn = self.conn()?;
        Ok(conn
            .query_row(
                "SELECT etag FROM intel_items WHERE url=?1 AND etag IS NOT NULL LIMIT 1",
                [url],
                |row| row.get(0),
            )
            .optional()?)
    }
}

fn row_to_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<IntelItem> {
    let tags_raw: String = row.get(6)?;
    let tags = serde_json::from_str(&tags_raw).unwrap_or_default();
    Ok(IntelItem {
        source: row.get(0)?,
        version: row.get(1)?,
        date: row.get(2)?,
        body: row.get(3)?,
        url: row.get(4)?,
        etag: row.get(5)?,
        tags,
    })
}

pub async fn scan_sources(
    sources: &[SourceConfig],
    cache_path: impl AsRef<Path>,
    limit_per_source: usize,
) -> Result<ScanReport> {
    let cache = IntelCache::open(cache_path)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(format!("sparrow-intel/{}", env!("CARGO_PKG_VERSION")))
        .build()?;
    let mut all = Vec::new();
    for source in sources {
        let mut items = fetch_source(&client, source, limit_per_source).await?;
        all.append(&mut items);
    }
    let changed = cache.upsert_items(&all)?;
    Ok(ScanReport {
        scanned: sources.len(),
        inserted_or_updated: changed,
        items: all,
    })
}

async fn fetch_source(
    client: &reqwest::Client,
    source: &SourceConfig,
    limit: usize,
) -> Result<Vec<IntelItem>> {
    match source.kind {
        SourceKind::GithubReleases => fetch_github_releases(client, source, limit).await,
        SourceKind::ChangelogUrl | SourceKind::DocsUrl => fetch_text_url(client, source).await,
    }
}

async fn fetch_github_releases(
    client: &reqwest::Client,
    source: &SourceConfig,
    limit: usize,
) -> Result<Vec<IntelItem>> {
    let api = github_releases_api(&source.url)?;
    let resp = client.get(api).send().await?.error_for_status()?;
    let releases: Vec<serde_json::Value> = resp.json().await?;
    Ok(releases
        .into_iter()
        .take(limit)
        .map(|release| {
            let version = release
                .get("tag_name")
                .and_then(|v| v.as_str())
                .unwrap_or("release")
                .to_string();
            let name = release
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or(&version);
            let body = release
                .get("body")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let date = release
                .get("published_at")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let url = release
                .get("html_url")
                .and_then(|v| v.as_str())
                .unwrap_or(&source.url)
                .to_string();
            IntelItem {
                source: source.name.clone(),
                version: version.clone(),
                date,
                body: format!("{name}\n\n{body}"),
                url,
                etag: None,
                tags: source.tags.clone(),
            }
        })
        .collect())
}

async fn fetch_text_url(client: &reqwest::Client, source: &SourceConfig) -> Result<Vec<IntelItem>> {
    let resp = client.get(&source.url).send().await?.error_for_status()?;
    let etag = resp
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|h| h.to_str().ok())
        .map(str::to_string);
    let body = resp.text().await?;
    let version = short_hash(&body);
    let date = Utc::now().to_rfc3339();
    Ok(vec![IntelItem {
        source: source.name.clone(),
        version,
        date,
        body,
        url: source.url.clone(),
        etag,
        tags: source.tags.clone(),
    }])
}

fn github_releases_api(raw: &str) -> Result<String> {
    if raw.contains("api.github.com/repos/") {
        return Ok(raw.to_string());
    }
    let parsed = url::Url::parse(raw)?;
    let host = parsed.host_str().unwrap_or_default();
    if host != "github.com" {
        anyhow::bail!("github_releases source must point at github.com or api.github.com");
    }
    let parts: Vec<_> = parsed
        .path_segments()
        .map(|s| s.collect::<Vec<_>>())
        .unwrap_or_default();
    if parts.len() < 2 {
        anyhow::bail!("github_releases source must include owner/repo");
    }
    Ok(format!(
        "https://api.github.com/repos/{}/{}/releases",
        parts[0], parts[1]
    ))
}

fn short_hash(body: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body.as_bytes());
    format!("{:.12x}", hasher.finalize())
}

fn digest_title(item: &IntelItem) -> String {
    item.body
        .lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| summarize(l, 90))
        .unwrap_or_else(|| item.version.clone())
}

fn summarize(text: &str, max: usize) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.len() <= max {
        compact
    } else {
        format!(
            "{}...",
            compact
                .chars()
                .take(max.saturating_sub(3))
                .collect::<String>()
        )
    }
}

fn score_digest(digest: &IntelDigest) -> (u32, String) {
    const SIGNALS: &[(&str, u32, &str)] = &[
        ("agent", 20, "agentic workflow"),
        ("tool", 14, "tooling/API"),
        ("mcp", 18, "MCP compatibility"),
        ("permission", 14, "permissions"),
        ("sandbox", 14, "sandbox safety"),
        ("approval", 12, "approvals"),
        ("checkpoint", 12, "checkpoint/replay"),
        ("replay", 12, "checkpoint/replay"),
        ("webview", 10, "cockpit UI"),
        ("performance", 10, "performance"),
        ("routing", 10, "model routing"),
        ("memory", 8, "memory/context"),
        ("context", 8, "memory/context"),
        ("release", 6, "release intelligence"),
    ];
    let hay = format!(
        "{} {} {} {}",
        digest.title,
        digest.summary,
        digest.source,
        digest.tags.join(" ")
    )
    .to_lowercase();
    let mut score = 0;
    let mut reasons = Vec::new();
    for (needle, weight, reason) in SIGNALS {
        if hay.contains(needle) {
            score += *weight;
            if !reasons.contains(reason) {
                reasons.push(*reason);
            }
        }
    }
    (score.min(100), reasons.join(", "))
}

pub fn default_cache_path(state_dir: &Path) -> std::path::PathBuf {
    state_dir.join("intel.sqlite")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(body: &str) -> IntelItem {
        IntelItem {
            source: "test".into(),
            version: "v1".into(),
            date: "2026-06-12T00:00:00Z".into(),
            body: body.into(),
            url: "https://example.test/release".into(),
            etag: None,
            tags: vec!["agent".into()],
        }
    }

    #[test]
    fn cache_round_trips_digests_and_backlog() {
        let dir = tempfile::tempdir().unwrap();
        let cache = IntelCache::open(dir.path().join("intel.sqlite")).unwrap();
        cache
            .upsert_items(&[item("Agent tool sandbox release with replay support")])
            .unwrap();
        let digests = cache.digests(10).unwrap();
        assert_eq!(digests.len(), 1);
        let backlog = cache.backlog(10).unwrap();
        assert_eq!(backlog.len(), 1);
        assert!(backlog[0].score >= 40);
    }

    #[test]
    fn parses_sources_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sources.toml");
        std::fs::write(
            &path,
            r#"
[[source]]
name = "Codex"
kind = "github_releases"
url = "https://github.com/openai/codex"
tags = ["agent", "cli"]
"#,
        )
        .unwrap();
        let sources = load_sources_file(&path).unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].kind, SourceKind::GithubReleases);
    }

    #[test]
    fn github_api_url_is_derived_from_repo_url() {
        let api = github_releases_api("https://github.com/openai/codex").unwrap();
        assert_eq!(api, "https://api.github.com/repos/openai/codex/releases");
    }
}
