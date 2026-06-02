use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::config::Config;
use crate::hooks::Hook;
use crate::permissions::PermissionMode;
use crate::redaction::RedactionFilter;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityFinding {
    pub severity: Severity,
    pub category: String,
    pub message: String,
    pub detail: String,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAudit {
    pub score: u32,
    pub findings: Vec<SecurityFinding>,
    pub checked_at: String,
}

impl SecurityAudit {
    pub fn run(config: &Config, hooks: &[Hook]) -> Self {
        let mut findings = Vec::new();

        Self::check_permissions(config, &mut findings);
        Self::check_gateway_senders(config, &mut findings);
        Self::check_dangerous_tools(config, &mut findings);
        Self::check_plugins(config, &mut findings);
        Self::check_hooks(config, hooks, &mut findings);
        Self::check_secrets_in_repo(&mut findings);
        Self::check_sandbox_exec(config, &mut findings);

        let score = Self::compute_score(&findings);

        SecurityAudit {
            score,
            findings,
            checked_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    fn check_permissions(config: &Config, findings: &mut Vec<SecurityFinding>) {
        let perms = &config.permissions;

        if perms.paths.deny.is_empty() {
            findings.push(SecurityFinding {
                severity: Severity::Warning,
                category: "permissions".into(),
                message: "No denied paths configured".into(),
                detail: "The permissions config has no denied paths, meaning sensitive files like .git, .env, .ssh are not explicitly blocked.".into(),
                recommendation: "Add default denied paths: .git, .env, .ssh, id_rsa, id_ed25519".into(),
            });
        }

        if matches!(perms.mode, PermissionMode::Autonomous) {
            findings.push(SecurityFinding {
                severity: Severity::Critical,
                category: "permissions".into(),
                message: "Autonomous mode without tool restrictions".into(),
                detail: "Permission mode is 'autonomous' but no tools are explicitly denied. Dangerous tools like exec could run unrestricted.".into(),
                recommendation: "Add dangerous tools to deny list or switch to a more restrictive permission mode".into(),
            });
        }
    }

    fn check_gateway_senders(config: &Config, findings: &mut Vec<SecurityFinding>) {
        let surfaces = &config.surfaces;

        for (name, surface) in [
            ("telegram", surfaces.telegram.as_ref()),
            ("discord", surfaces.discord.as_ref()),
            ("slack", surfaces.slack.as_ref()),
        ] {
            if let Some(s) = surface {
                if s.enabled && s.allow_users.is_empty() {
                    findings.push(SecurityFinding {
                        severity: Severity::Critical,
                        category: "gateway".into(),
                        message: format!("Gateway {} accepts all users", name),
                        detail: format!(
                            "The {} gateway surface is enabled but has no allow_users list, meaning any user can send messages.",
                            name
                        ),
                        recommendation: format!(
                            "Add allow_users list to {} surface config to restrict access",
                            name
                        ),
                    });
                }
            }
        }

        if let Some(e) = surfaces.email.as_ref() {
            if e.enabled && e.allowed_to.is_empty() {
                findings.push(SecurityFinding {
                    severity: Severity::Critical,
                    category: "gateway".into(),
                    message: "Gateway email accepts all recipients".into(),
                    detail: "The email surface is enabled but has no allowed_to list, meaning replies can be sent to any address.".into(),
                    recommendation: "Add allowed_to to surfaces.email config to restrict recipients".into(),
                });
            }
        }
    }

    fn check_dangerous_tools(config: &Config, findings: &mut Vec<SecurityFinding>) {
        let tools = &config.permissions.tools;

        let dangerous = ["exec", "terminal", "destructive"];
        for tool_name in &dangerous {
            if tools.deny.iter().any(|t| t == tool_name) {
                continue;
            }
            if !tools.allow.iter().any(|t| t == tool_name) {
                findings.push(SecurityFinding {
                    severity: Severity::Warning,
                    category: "tools".into(),
                    message: format!("Dangerous tool '{}' not explicitly denied", tool_name),
                    detail: format!(
                        "Tool '{}' is classified as dangerous but is not in the deny list.",
                        tool_name
                    ),
                    recommendation: format!(
                        "Add '{}' to tools.deny or use permission rules to restrict it",
                        tool_name
                    ),
                });
            }
        }
    }

    fn check_plugins(config: &Config, findings: &mut Vec<SecurityFinding>) {
        let plugins_dir = config.config_dir.join("plugins");

        if !plugins_dir.exists() {
            return;
        }

        if let Ok(entries) = std::fs::read_dir(&plugins_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let manifest_path = path.join("plugin.toml");
                    if manifest_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                            if !content.contains("allowlist") {
                                findings.push(SecurityFinding {
                                    severity: Severity::Warning,
                                    category: "plugins".into(),
                                    message: format!(
                                        "Plugin '{}' has no allowlist",
                                        path.file_name().unwrap_or_default().to_string_lossy()
                                    ),
                                    detail: "Plugin manifest does not define an allowlist.".into(),
                                    recommendation:
                                        "Add an allowlist section to the plugin manifest".into(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    fn check_hooks(_config: &Config, hooks: &[Hook], findings: &mut Vec<SecurityFinding>) {
        let redaction = RedactionFilter::new();

        for hook in hooks {
            if !hook.enabled {
                continue;
            }

            let cmd = &hook.command;
            let lower = cmd.to_lowercase();
            let suspicious = [
                "rm -rf",
                "curl |",
                "wget |",
                "eval ",
                "exec(",
                "powershell -enc",
                "base64 -d",
            ];

            for pattern in &suspicious {
                if lower.contains(pattern) {
                    findings.push(SecurityFinding {
                        severity: Severity::Critical,
                        category: "hooks".into(),
                        message: format!("Suspicious hook command: {}", cmd),
                        detail: format!(
                            "Hook '{}' contains suspicious pattern '{}'.",
                            hook.id, pattern
                        ),
                        recommendation: "Review and sanitize hook commands".into(),
                    });
                }
            }

            if redaction.contains_secret(cmd) {
                findings.push(SecurityFinding {
                    severity: Severity::Critical,
                    category: "hooks".into(),
                    message: format!("Secret found in hook: {}", hook.id),
                    detail: "Hook command contains what appears to be a secret or API key.".into(),
                    recommendation:
                        "Remove secrets from hook commands and use environment variables".into(),
                });
            }
        }
    }

    fn check_secrets_in_repo(findings: &mut Vec<SecurityFinding>) {
        let repo_root = Path::new(".");

        let secret_patterns = [
            "sk-ant-", "ghp_", "gho_", "ghu_", "ghs_", "ghr_", "xai-", "nvapi-", "hf_", "gsk_",
        ];

        Self::scan_directory_for_secrets(repo_root, &secret_patterns, findings);
    }

    fn scan_directory_for_secrets(
        dir: &Path,
        patterns: &[&str],
        findings: &mut Vec<SecurityFinding>,
    ) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        for pattern in patterns {
                            if content.contains(pattern) {
                                findings.push(SecurityFinding {
                                    severity: Severity::Critical,
                                    category: "secrets".into(),
                                    message: format!(
                                        "Potential secret in {}",
                                        path.display()
                                    ),
                                    detail: format!(
                                        "File contains pattern '{}': {}",
                                        pattern,
                                        content.lines().find(|l| l.contains(pattern)).unwrap_or("")
                                    ),
                                    recommendation: "Remove secrets from source files and use environment variables".into(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    fn check_sandbox_exec(config: &Config, findings: &mut Vec<SecurityFinding>) {
        let sandbox = &config.defaults.sandbox;
        let permissions = &config.permissions;

        if sandbox == "local" && permissions.mode == PermissionMode::Autonomous {
            let tools_deny = &permissions.tools.deny;
            if !tools_deny.iter().any(|t| t == "exec" || t == "terminal") {
                findings.push(SecurityFinding {
                    severity: Severity::Critical,
                    category: "sandbox".into(),
                    message: "Exec tool exposed without sandbox".into(),
                    detail: "The exec tool is not denied and sandbox mode is 'local', allowing unrestricted command execution.".into(),
                    recommendation: "Add 'exec' and 'terminal' to permissions.tools.deny or enable sandbox mode".into(),
                });
            }
        }
    }

    fn compute_score(findings: &[SecurityFinding]) -> u32 {
        if findings.is_empty() {
            return 100;
        }

        let mut score: i32 = 100;
        for finding in findings {
            match finding.severity {
                Severity::Info => {}
                Severity::Warning => score -= 5,
                Severity::Critical => score -= 15,
            }
        }

        score.max(0) as u32
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn summary(&self) -> String {
        let critical = self
            .findings
            .iter()
            .filter(|f| matches!(f.severity, Severity::Critical))
            .count();
        let warnings = self
            .findings
            .iter()
            .filter(|f| matches!(f.severity, Severity::Warning))
            .count();
        let info = self
            .findings
            .iter()
            .filter(|f| matches!(f.severity, Severity::Info))
            .count();

        format!(
            "Security Audit: score {}/100 | {} critical, {} warnings, {} info",
            self.score, critical, warnings, info
        )
    }
}
