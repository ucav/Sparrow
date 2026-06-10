use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::event::{AutonomyLevel, Decision, RiskLevel};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionMode {
    ReadOnly,
    Plan,
    Supervised,
    Trusted,
    Autonomous,
    EmergencyStop,
}

impl PermissionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            PermissionMode::ReadOnly => "read-only",
            PermissionMode::Plan => "plan",
            PermissionMode::Supervised => "supervised",
            PermissionMode::Trusted => "trusted",
            PermissionMode::Autonomous => "autonomous",
            PermissionMode::EmergencyStop => "emergency-stop",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_lowercase().as_str() {
            "read-only" | "readonly" | "read_only" => Some(Self::ReadOnly),
            "plan" => Some(Self::Plan),
            "supervised" => Some(Self::Supervised),
            "trusted" => Some(Self::Trusted),
            "autonomous" => Some(Self::Autonomous),
            "emergency-stop" | "emergency" | "stop" | "kill" => Some(Self::EmergencyStop),
            _ => None,
        }
    }

    pub fn autonomy_level(&self) -> AutonomyLevel {
        match self {
            PermissionMode::Autonomous => AutonomyLevel::Autonomous,
            PermissionMode::Trusted => AutonomyLevel::Trusted,
            PermissionMode::ReadOnly
            | PermissionMode::Plan
            | PermissionMode::Supervised
            | PermissionMode::EmergencyStop => AutonomyLevel::Supervised,
        }
    }
}

impl Default for PermissionMode {
    fn default() -> Self {
        Self::Supervised
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionList {
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub ask: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
}

impl Default for PermissionList {
    fn default() -> Self {
        Self {
            allow: Vec::new(),
            ask: Vec::new(),
            deny: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathPermissions {
    #[serde(default)]
    pub allow: Vec<PathBuf>,
    #[serde(default = "default_denied_paths")]
    pub deny: Vec<PathBuf>,
}

impl Default for PathPermissions {
    fn default() -> Self {
        Self {
            allow: Vec::new(),
            deny: default_denied_paths(),
        }
    }
}

fn default_denied_paths() -> Vec<PathBuf> {
    vec![
        PathBuf::from(".git"),
        PathBuf::from(".env"),
        PathBuf::from(".env.local"),
        PathBuf::from(".ssh"),
        PathBuf::from("id_rsa"),
        PathBuf::from("id_ed25519"),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionConfig {
    #[serde(default)]
    pub mode: PermissionMode,
    #[serde(default)]
    pub tools: PermissionList,
    #[serde(default)]
    pub paths: PathPermissions,
    #[serde(default)]
    pub providers: PermissionList,
    #[serde(default)]
    pub surfaces: PermissionList,
}

impl Default for PermissionConfig {
    fn default() -> Self {
        Self {
            mode: PermissionMode::Supervised,
            tools: PermissionList::default(),
            paths: PathPermissions::default(),
            providers: PermissionList::default(),
            surfaces: PermissionList::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PermissionContext<'a> {
    pub tool_name: &'a str,
    pub risk: RiskLevel,
    pub args: &'a serde_json::Value,
    pub workspace_root: &'a Path,
    pub provider: Option<&'a str>,
    pub surface: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionVerdict {
    pub decision: Decision,
    pub reason: String,
}

impl PermissionConfig {
    pub fn evaluate(&self, ctx: &PermissionContext<'_>) -> PermissionVerdict {
        if matches!(self.mode, PermissionMode::EmergencyStop) {
            return verdict(Decision::Deny, "emergency stop blocks every action");
        }

        if matches!(self.mode, PermissionMode::Plan) {
            return verdict(
                Decision::Deny,
                "plan mode is read-only and executes no tools",
            );
        }

        if matches!(self.mode, PermissionMode::ReadOnly) && ctx.risk != RiskLevel::ReadOnly {
            return verdict(
                Decision::Deny,
                "read-only permission mode blocks mutating, exec, network, and destructive tools",
            );
        }

        if matches_pattern(&self.tools.deny, ctx.tool_name) {
            return verdict(
                Decision::Deny,
                format!("tool '{}' is denied by permissions", ctx.tool_name),
            );
        }
        if matches_pattern(&self.tools.ask, ctx.tool_name) {
            return verdict(
                Decision::AskUser,
                format!("tool '{}' requires approval by permissions", ctx.tool_name),
            );
        }
        if matches_pattern(&self.tools.allow, ctx.tool_name) {
            return verdict(
                Decision::Allow,
                format!(
                    "tool '{}' is explicitly allowed by permissions",
                    ctx.tool_name
                ),
            );
        }

        if let Some(provider) = ctx.provider {
            if matches_pattern(&self.providers.deny, provider) {
                return verdict(
                    Decision::Deny,
                    format!("provider '{}' is denied by permissions", provider),
                );
            }
            if matches_pattern(&self.providers.ask, provider) {
                return verdict(
                    Decision::AskUser,
                    format!("provider '{}' requires approval by permissions", provider),
                );
            }
        }

        if let Some(surface) = ctx.surface {
            if matches_pattern(&self.surfaces.deny, surface) {
                return verdict(
                    Decision::Deny,
                    format!("surface '{}' is denied by permissions", surface),
                );
            }
            if matches_pattern(&self.surfaces.ask, surface) {
                return verdict(
                    Decision::AskUser,
                    format!("surface '{}' requires approval by permissions", surface),
                );
            }
        }

        for path in paths_from_args(ctx.args) {
            let absolute = resolve_path(ctx.workspace_root, &path);
            if self
                .paths
                .deny
                .iter()
                .any(|rule| path_matches(ctx.workspace_root, rule, &absolute))
            {
                return verdict(
                    Decision::Deny,
                    format!("path '{}' is denied by permissions", path.display()),
                );
            }
        }

        verdict(Decision::Allow, "permissions allow autonomy gate to decide")
    }
}

fn verdict(decision: Decision, reason: impl Into<String>) -> PermissionVerdict {
    PermissionVerdict {
        decision,
        reason: reason.into(),
    }
}

fn matches_pattern(patterns: &[String], value: &str) -> bool {
    patterns.iter().any(|pattern| {
        let pattern = pattern.trim();
        pattern == "*"
            || pattern.eq_ignore_ascii_case(value)
            || value
                .to_lowercase()
                .contains(pattern.trim_matches('*').to_lowercase().as_str())
    })
}

fn paths_from_args(args: &serde_json::Value) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_paths(args, &mut paths);
    paths
}

fn collect_paths(value: &serde_json::Value, paths: &mut Vec<PathBuf>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                let key = key.to_lowercase();
                let pathish = matches!(
                    key.as_str(),
                    "path" | "file" | "filename" | "target" | "source" | "dest" | "destination"
                ) || key.ends_with("_path")
                    || key.ends_with("_file");
                if pathish {
                    if let Some(text) = value.as_str() {
                        paths.push(PathBuf::from(text));
                    }
                }
                collect_paths(value, paths);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_paths(item, paths);
            }
        }
        _ => {}
    }
}

fn resolve_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn path_matches(root: &Path, rule: &Path, candidate: &Path) -> bool {
    let rule = resolve_path(root, rule);
    let rule_text = normalize_path(&rule);
    let candidate_text = normalize_path(candidate);
    candidate_text == rule_text || candidate_text.starts_with(&(rule_text + "/"))
}

fn normalize_path(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join("/")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_blocks_mutating_tools() {
        let cfg = PermissionConfig {
            mode: PermissionMode::ReadOnly,
            ..PermissionConfig::default()
        };
        let verdict = cfg.evaluate(&PermissionContext {
            tool_name: "edit",
            risk: RiskLevel::Mutating,
            args: &serde_json::json!({"path":"src/main.rs"}),
            workspace_root: Path::new("/home/dev/project"),
            provider: None,
            surface: Some("cli"),
        });
        assert_eq!(verdict.decision, Decision::Deny);
    }

    #[test]
    fn denied_sensitive_paths_win() {
        let cfg = PermissionConfig::default();
        let verdict = cfg.evaluate(&PermissionContext {
            tool_name: "fs_write",
            risk: RiskLevel::Mutating,
            args: &serde_json::json!({"path":".git/config"}),
            workspace_root: Path::new("/home/dev/project"),
            provider: None,
            surface: None,
        });
        assert_eq!(verdict.decision, Decision::Deny);
    }

    #[test]
    fn ask_tool_requires_user() {
        let mut cfg = PermissionConfig::default();
        cfg.tools.ask.push("exec".into());
        let verdict = cfg.evaluate(&PermissionContext {
            tool_name: "exec",
            risk: RiskLevel::Exec,
            args: &serde_json::json!({"cmd":"cargo test"}),
            workspace_root: Path::new("/home/dev/project"),
            provider: None,
            surface: None,
        });
        assert_eq!(verdict.decision, Decision::AskUser);
    }
}
