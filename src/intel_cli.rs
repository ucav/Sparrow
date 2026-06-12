use crate::cli::IntelAction;
use crate::config::Config;
use anyhow::Result;

pub async fn handle_intel_action(action: IntelAction, config: &Config) -> Result<()> {
    match action {
        IntelAction::Scan {
            config: sources_file,
            source,
            limit,
            json,
        } => {
            let sources = resolve_sources(config, sources_file.as_deref(), &source)?;
            if sources.is_empty() {
                anyhow::bail!(
                    "No intel sources configured. Set intel.enabled=true with [[intel.sources]] entries, pass --config, or pass --source kind:name:url."
                );
            }
            if !config.intel.enabled && source.is_empty() && sources_file.is_none() {
                anyhow::bail!(
                    "intel.enabled=false, so no network scan was started. Use `sparrow intel report` for cached data, or explicitly pass --source/--config."
                );
            }
            let cache = sparrow_intel::default_cache_path(&config.state_dir);
            let report = sparrow_intel::scan_sources(&sources, &cache, limit).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "Intel scan: {} source(s), {} item(s) cached at {}",
                    report.scanned,
                    report.inserted_or_updated,
                    cache.display()
                );
                for item in report.items.iter().take(10) {
                    println!("- {} {} · {}", item.source, item.version, item.url);
                }
            }
        }
        IntelAction::Report { limit, json } => {
            let cache = sparrow_intel::IntelCache::open(sparrow_intel::default_cache_path(
                &config.state_dir,
            ))?;
            let digests = cache.digests(limit)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&digests)?);
            } else if digests.is_empty() {
                println!(
                    "No cached intel digests yet. Run `sparrow intel scan` with opt-in sources."
                );
            } else {
                for digest in digests {
                    println!(
                        "- {} {} · {}\n  {}\n  {}",
                        digest.source, digest.version, digest.title, digest.summary, digest.url
                    );
                }
            }
        }
        IntelAction::Backlog { limit, json } => {
            let cache = sparrow_intel::IntelCache::open(sparrow_intel::default_cache_path(
                &config.state_dir,
            ))?;
            let tickets = cache.backlog(limit)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&tickets)?);
            } else if tickets.is_empty() {
                println!(
                    "No scored intel backlog yet. Cached digests did not match Sparrow signals."
                );
            } else {
                for ticket in tickets {
                    println!(
                        "- [{}] {} · {}\n  {}",
                        ticket.score, ticket.title, ticket.reason, ticket.url
                    );
                }
            }
        }
        IntelAction::Watch {
            interval,
            config: sources_file,
            source,
        } => {
            let sources = resolve_sources(config, sources_file.as_deref(), &source)?;
            if sources.is_empty() {
                anyhow::bail!("No intel sources configured for watch.");
            }
            if !config.intel.enabled && source.is_empty() && sources_file.is_none() {
                anyhow::bail!(
                    "intel.enabled=false, so watch will not start without explicit --source/--config."
                );
            }
            let cache = sparrow_intel::default_cache_path(&config.state_dir);
            loop {
                let report = sparrow_intel::scan_sources(&sources, &cache, 5).await?;
                println!(
                    "Intel watch tick: {} source(s), {} item(s)",
                    report.scanned, report.inserted_or_updated
                );
                tokio::time::sleep(std::time::Duration::from_secs(interval.max(60))).await;
            }
        }
    }
    Ok(())
}

fn resolve_sources(
    config: &Config,
    sources_file: Option<&std::path::Path>,
    explicit: &[String],
) -> Result<Vec<sparrow_intel::SourceConfig>> {
    let mut sources = Vec::new();
    if let Some(path) = sources_file {
        sources.extend(sparrow_intel::load_sources_file(path)?);
    }
    for raw in explicit {
        sources.push(parse_source_arg(raw)?);
    }
    if sources.is_empty() {
        sources.extend(
            config
                .intel
                .sources
                .iter()
                .filter_map(|s| s.to_sparrow_intel()),
        );
    }
    Ok(sources)
}

fn parse_source_arg(raw: &str) -> Result<sparrow_intel::SourceConfig> {
    let mut parts = raw.splitn(3, ':');
    let kind_raw = parts.next().unwrap_or_default();
    let name = parts.next().unwrap_or_default();
    let url = parts.next().unwrap_or_default();
    if kind_raw.is_empty() || name.is_empty() || url.is_empty() {
        anyhow::bail!("--source must use kind:name:url");
    }
    let kind = match kind_raw {
        "github_releases" => sparrow_intel::SourceKind::GithubReleases,
        "changelog_url" => sparrow_intel::SourceKind::ChangelogUrl,
        "docs_url" => sparrow_intel::SourceKind::DocsUrl,
        _ => anyhow::bail!("unknown intel source kind: {kind_raw}"),
    };
    Ok(sparrow_intel::SourceConfig {
        name: name.to_string(),
        kind,
        url: url.to_string(),
        tags: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_source_arg_parses() {
        let src = parse_source_arg("github_releases:Codex:https://github.com/openai/codex")
            .expect("source arg should parse");
        assert_eq!(src.name, "Codex");
        assert_eq!(src.kind, sparrow_intel::SourceKind::GithubReleases);
    }

    #[tokio::test]
    async fn disabled_intel_without_explicit_source_refuses_scan_before_network() {
        let mut cfg = Config::default();
        cfg.intel.enabled = false;
        cfg.intel.sources = vec![crate::config::IntelSourceConfig {
            name: "example".into(),
            kind: "github_releases".into(),
            url: "https://github.com/openai/codex".into(),
            tags: Vec::new(),
        }];
        let result = handle_intel_action(
            IntelAction::Scan {
                config: None,
                source: Vec::new(),
                limit: 1,
                json: true,
            },
            &cfg,
        )
        .await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("intel.enabled=false")
        );
    }
}
