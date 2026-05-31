// ─── Migration paths from competitor tools (§4 + WS10) ────────────────────────

use std::path::PathBuf;

pub struct Migration;

#[derive(Debug, Clone)]
pub struct MigrationResult {
    pub tool: String,
    pub agents: usize,
    pub skills: usize,
    pub cron_jobs: usize,
    pub config_entries: usize,
    pub surfaces: usize,
}

impl Migration {
    /// Import from OpenClaw (already partially implemented)
    pub fn import_openclaw(path: &PathBuf) -> anyhow::Result<MigrationResult> {
        let mut result = MigrationResult {
            tool: "openclaw".into(),
            agents: 0,
            skills: 0,
            cron_jobs: 0,
            config_entries: 0,
            surfaces: 0,
        };

        // Agents/SOUL
        let agents_dir = path.join("agents");
        if agents_dir.exists() {
            result.agents = std::fs::read_dir(&agents_dir)?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
                .count();
        }

        // Skills
        let skills_dir = path.join("skills");
        if skills_dir.exists() {
            result.skills = std::fs::read_dir(&skills_dir)?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .count();
        }

        // Cron
        let cron_file = path.join("cron.json");
        if cron_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&cron_file) {
                if let Ok(jobs) = serde_json::from_str::<Vec<serde_json::Value>>(&content) {
                    result.cron_jobs = jobs.len();
                }
            }
        }

        Ok(result)
    }

    /// Import from Claude Code
    pub fn import_claude_code(path: &PathBuf) -> anyhow::Result<MigrationResult> {
        let mut result = MigrationResult {
            tool: "claude-code".into(),
            agents: 0,
            skills: 0,
            cron_jobs: 0,
            config_entries: 0,
            surfaces: 0,
        };

        // CLAUDE.md → Sparrow SOUL
        let claude_md = path.join("CLAUDE.md");
        if claude_md.exists() {
            let content = std::fs::read_to_string(&claude_md)?;
            // Convert to Sparrow SOUL
            let soul = format!(
                "# Imported from Claude Code\nname = \"claude-code-import\"\nrole = \"assistant\"\npersonality = \"\"\"\n{}\n\"\"\"\n",
                content.lines().take(50).collect::<Vec<_>>().join("\n")
            );
            let dest = dirs::config_dir()
                .unwrap_or_default()
                .join("sparrow")
                .join("agents")
                .join("claude-code-import.soul.toml");
            std::fs::create_dir_all(dest.parent().unwrap())?;
            std::fs::write(&dest, soul)?;
            result.agents = 1;
            result.config_entries = content.lines().count();
        }

        // MCP servers → Sparrow MCP
        let mcp_config = path.join(".mcp.json");
        if mcp_config.exists() {
            result.config_entries += 1;
        }

        // .claude/settings.json → config
        let settings = path.join(".claude").join("settings.json");
        if settings.exists() {
            result.config_entries += 1;
        }

        Ok(result)
    }

    /// Import from Codex
    pub fn import_codex(path: &PathBuf) -> anyhow::Result<MigrationResult> {
        let mut result = MigrationResult {
            tool: "codex".into(),
            agents: 0,
            skills: 0,
            cron_jobs: 0,
            config_entries: 0,
            surfaces: 0,
        };

        // AGENTS.md → agent import
        let agents_md = path.join("AGENTS.md");
        if agents_md.exists() {
            let content = std::fs::read_to_string(&agents_md)?;
            result.agents = 1;
            result.config_entries = content.lines().count();
        }

        // codex.yaml → config
        let config_yaml = path.join("codex.yaml");
        if config_yaml.exists() || path.join("codex.yml").exists() {
            result.config_entries += 1;
        }

        Ok(result)
    }

    /// Import from OpenCode
    pub fn import_opencode(path: &PathBuf) -> anyhow::Result<MigrationResult> {
        let mut result = MigrationResult {
            tool: "opencode".into(),
            agents: 0,
            skills: 0,
            cron_jobs: 0,
            config_entries: 0,
            surfaces: 0,
        };

        // opencode.json → config
        let config_json = path.join("opencode.json");
        if config_json.exists() {
            let content = std::fs::read_to_string(&config_json)?;
            if let Ok(cfg) = serde_json::from_str::<serde_json::Value>(&content) {
                result.config_entries = cfg.as_object().map(|o| o.len()).unwrap_or(0);
            }
        }

        Ok(result)
    }

    /// Import from Hermes Agent
    pub fn import_hermes(path: &PathBuf) -> anyhow::Result<MigrationResult> {
        let mut result = MigrationResult {
            tool: "hermes".into(),
            agents: 0,
            skills: 0,
            cron_jobs: 0,
            config_entries: 0,
            surfaces: 0,
        };

        // agents/*.md → Sparrow SOULs
        let agents_dir = path.join("agents");
        if agents_dir.exists() {
            result.agents = std::fs::read_dir(&agents_dir)?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
                .count();
        }

        // skills/ → Sparrow skills
        let skills_dir = path.join("skills");
        if skills_dir.exists() {
            result.skills = std::fs::read_dir(&skills_dir)?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .count();
        }

        // hermes.yaml → config
        let config_yaml = path.join("hermes.yaml");
        if config_yaml.exists() {
            if let Ok(content) = std::fs::read_to_string(&config_yaml) {
                if let Ok(cfg) = serde_json::from_str::<serde_json::Value>(&content) {
                    result.config_entries = cfg.as_object().map(|o| o.len()).unwrap_or(0);
                }
            }
        }

        Ok(result)
    }

    /// Auto-detect installed tools and offer import
    pub fn detect_installed() -> Vec<String> {
        let mut found = Vec::new();
        let home = dirs::home_dir().unwrap_or_default();

        let tools: Vec<(&str, PathBuf)> = vec![
            ("openclaw", home.join(".openclaw")),
            ("claude-code", home.join(".claude")),
            ("codex", home.join(".codex")),
            ("opencode", home.join(".config").join("opencode")),
            ("hermes", home.join(".hermes")),
        ];

        for (name, path) in tools {
            if path.exists() {
                found.push(name.to_string());
            }
        }
        found
    }
}
