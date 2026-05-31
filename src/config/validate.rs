use crate::config::Config;

/// Validate config and return human-readable warnings/errors
pub fn validate_config(config: &Config) -> Vec<ConfigIssue> {
    let mut issues = Vec::new();

    // Budget sanity
    if config.budget.daily_usd <= 0.0 {
        issues.push(ConfigIssue::warn(
            "budget.daily_usd is 0 — no cloud providers will be used",
        ));
    }
    if config.budget.session_usd <= 0.0 {
        issues.push(ConfigIssue::warn(
            "budget.session_usd is 0 — runs will stop immediately",
        ));
    }
    if config.budget.daily_usd > 100.0 {
        issues.push(ConfigIssue::warn(
            "budget.daily_usd is very high ($100+). Consider setting a reasonable cap",
        ));
    }

    // Provider check
    if config.providers.is_empty() {
        issues.push(ConfigIssue::info("No providers configured. Add one with 'sparrow auth add' or set *_API_KEY env vars. Ollama will be tried as fallback."));
    }
    for (name, pconfig) in &config.providers {
        if pconfig.models.is_empty() {
            issues.push(ConfigIssue::warn(&format!(
                "Provider '{}' has no models configured",
                name
            )));
        }
        if pconfig.adapter.is_empty() {
            issues.push(ConfigIssue::error(&format!(
                "Provider '{}' has no adapter set",
                name
            )));
        }
    }

    // Autonomy sanity
    if config.routing.free_first && config.providers.is_empty() {
        issues.push(ConfigIssue::info(
            "routing.free_first is enabled but no free providers configured. Ollama will be tried.",
        ));
    }

    // Sandbox
    if config.defaults.sandbox == "local-hardened" && !cfg!(target_os = "linux") {
        issues.push(ConfigIssue::warn(
            "sandbox 'local-hardened' requires Linux. Falling back to 'local'.",
        ));
    }

    // Skills dir
    if !config.skills.dir.exists() {
        issues.push(ConfigIssue::info(&format!(
            "Skills directory does not exist: {}. It will be created automatically.",
            config.skills.dir.display()
        )));
    }

    issues
}

#[derive(Debug, Clone)]
pub enum IssueLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct ConfigIssue {
    pub level: IssueLevel,
    pub message: String,
}

impl ConfigIssue {
    pub fn info(msg: &str) -> Self {
        Self {
            level: IssueLevel::Info,
            message: msg.into(),
        }
    }
    pub fn warn(msg: &str) -> Self {
        Self {
            level: IssueLevel::Warning,
            message: msg.into(),
        }
    }
    pub fn error(msg: &str) -> Self {
        Self {
            level: IssueLevel::Error,
            message: msg.into(),
        }
    }

    pub fn icon(&self) -> &str {
        match self.level {
            IssueLevel::Info => "ℹ",
            IssueLevel::Warning => "⚠",
            IssueLevel::Error => "✗",
        }
    }
}

/// Ping a provider to check if it's reachable and the API key works
pub async fn ping_provider(
    name: &str,
    base_url: &str,
    api_key: &str,
    adapter: &str,
) -> ProviderHealth {
    let url = match adapter {
        "ollama" => format!("{}/api/tags", base_url.trim_end_matches('/')),
        "openai-compatible" => format!("{}/models", base_url.trim_end_matches('/')),
        "anthropic-messages" => format!("{}/v1/messages", base_url.trim_end_matches('/')),
        _ => format!("{}/models", base_url.trim_end_matches('/')),
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    let mut req = client.get(&url);
    if adapter != "ollama" && !api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", api_key));
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            ProviderHealth {
                name: name.into(),
                reachable: status < 500,
                status_code: status,
                latency_ms: 0,
                message: if status == 200 {
                    "OK".into()
                } else {
                    format!("HTTP {}", status)
                },
            }
        }
        Err(e) => ProviderHealth {
            name: name.into(),
            reachable: false,
            status_code: 0,
            latency_ms: 0,
            message: format!("{}", e),
        },
    }
}

#[derive(Debug, Clone)]
pub struct ProviderHealth {
    pub name: String,
    pub reachable: bool,
    pub status_code: u16,
    pub latency_ms: u64,
    pub message: String,
}
