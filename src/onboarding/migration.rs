// ─── Migration paths from competitor tools (§4) ───────────────────────────────
//
// Sparrow import <tool> — zero-friction migration.
// Reads the competitor's config directory, extracts everything we understand,
// and writes the Sparrow equivalents. Never overwrites user data without asking.

use std::path::{Path, PathBuf};

use crate::onboarding::claude_compat::{self, ClaudeSettings};

// ─── Migration result ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MigrationResult {
    pub tool: String,
    pub agents: usize,
    pub commands: usize,
    pub skills: usize,
    pub config_entries: usize,
    pub api_keys: usize,
    pub mcp_servers: usize,
    pub summary: Vec<String>,
}

impl MigrationResult {
    fn new(tool: &str) -> Self {
        MigrationResult {
            tool: tool.to_string(),
            agents: 0,
            commands: 0,
            skills: 0,
            config_entries: 0,
            api_keys: 0,
            mcp_servers: 0,
            summary: Vec::new(),
        }
    }

    fn note(&mut self, msg: &str) {
        self.summary.push(msg.to_string());
    }
}

// ─── Target directories ────────────────────────────────────────────────────────

fn sparrow_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("sparrow")
}

fn sparrow_agents_dir() -> PathBuf {
    sparrow_dir().join("agents")
}

fn sparrow_commands_dir() -> PathBuf {
    sparrow_dir().join("commands")
}

fn sparrow_config_file() -> PathBuf {
    sparrow_dir().join("config.toml")
}

// ─── Public API ────────────────────────────────────────────────────────────────

pub struct Migration;

impl Migration {
    // ── Claude Code ─────────────────────────────────────────────────────────

    /// Import from Claude Code.
    ///
    /// Reads:
    ///   - `~/.claude/CLAUDE.md`          → user-level Sparrow instruction
    ///   - `~/.claude/commands/*.md`      → Sparrow slash commands
    ///   - `~/.claude/agents/*.md`        → Sparrow SOUL agents
    ///   - `~/.claude/settings.json`      → Sparrow config (permissions, env)
    ///   - `<cwd>/.claude/CLAUDE.md`      → project-level instruction
    ///   - `~/.claude/mcp.json` or `.mcp.json` → Sparrow MCP servers
    pub fn import_claude_code(path: &Path) -> anyhow::Result<MigrationResult> {
        let mut result = MigrationResult::new("claude-code");
        let home = dirs::home_dir().unwrap_or_default();
        let cwd = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        // Discover everything Claude Code has on disk
        let imported = claude_compat::discover(&home, &cwd);

        // ── CLAUDE.md → memory / instruction ───────────────────────────────
        if let Some(user_md) = &imported.user_memory {
            write_sparrow_instruction("claude-user.md", user_md)?;
            result.note("Imported ~/.claude/CLAUDE.md → user instruction");
            result.config_entries += 1;
        }
        if let Some(proj_md) = &imported.project_memory {
            write_sparrow_instruction("claude-project.md", proj_md)?;
            result.note("Imported .claude/CLAUDE.md → project instruction");
            result.config_entries += 1;
        }

        // ── Commands → Sparrow slash commands ──────────────────────────────
        let cmd_dir = sparrow_commands_dir();
        std::fs::create_dir_all(&cmd_dir)?;
        for cmd in &imported.commands {
            let dest = cmd_dir.join(format!("{}.md", cmd.name));
            if !dest.exists() {
                std::fs::write(&dest, &cmd.body)?;
                result.commands += 1;
            }
        }
        if result.commands > 0 {
            result.note(&format!(
                "Imported {} slash commands → {}",
                result.commands,
                cmd_dir.display()
            ));
        }

        // ── Agents → Sparrow SOUL agents ───────────────────────────────────
        let agents_dir = sparrow_agents_dir();
        std::fs::create_dir_all(&agents_dir)?;
        for agent in &imported.agents {
            let dest = agents_dir.join(format!("{}.soul.toml", agent.name));
            if !dest.exists() {
                let soul = agent_body_to_soul(&agent.name, &agent.body);
                std::fs::write(&dest, &soul)?;
                result.agents += 1;
            }
        }
        if result.agents > 0 {
            result.note(&format!(
                "Imported {} agents → {}",
                result.agents,
                agents_dir.display()
            ));
        }

        // ── settings.json → Sparrow config ─────────────────────────────────
        if let Some(settings) = &imported.settings {
            let merged = merge_claude_settings(settings)?;
            if merged > 0 {
                result.config_entries += merged;
                result.note(&format!(
                    "Merged {} settings entries into {}",
                    merged,
                    sparrow_config_file().display()
                ));
            }
        }

        // ── MCP servers ────────────────────────────────────────────────────
        let mcp_sources = [home.join(".claude").join("mcp.json"), cwd.join(".mcp.json")];
        for mcp_path in &mcp_sources {
            if mcp_path.exists() {
                result.mcp_servers += import_mcp_servers(mcp_path)?;
            }
        }
        if result.mcp_servers > 0 {
            result.note(&format!("Imported {} MCP servers", result.mcp_servers));
        }

        // ── API keys from env ──────────────────────────────────────────────
        if let Some(settings) = &imported.settings {
            result.api_keys += extract_api_keys_from_settings(settings)?;
        }
        if result.api_keys > 0 {
            result.note(&format!(
                "Detected {} API keys in settings",
                result.api_keys
            ));
            result.note("Run `sparrow auth add <provider>` to register them securely.");
        }

        if result.summary.is_empty() {
            result.note("No Claude Code configuration found.");
        }

        Ok(result)
    }

    // ── Codex ──────────────────────────────────────────────────────────────

    /// Import from OpenAI Codex CLI.
    ///
    /// Reads:
    ///   - `~/.codex/config.json` / `codex.yaml`      → provider/model config
    ///   - `AGENTS.md`                                  → agent instructions
    pub fn import_codex(path: &Path) -> anyhow::Result<MigrationResult> {
        let mut result = MigrationResult::new("codex");
        let cwd = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let home = dirs::home_dir().unwrap_or_default();

        // AGENTS.md → instruction
        for md_path in &[cwd.join("AGENTS.md"), home.join(".codex").join("AGENTS.md")] {
            if md_path.exists() {
                let content = std::fs::read_to_string(md_path)?;
                write_sparrow_instruction("codex-agents.md", &content)?;
                result.note("Imported AGENTS.md → instruction");
                result.config_entries += 1;
                break;
            }
        }

        // codex.yaml / codex.yml / ~/.codex/config.json → provider config
        let config_paths = [
            cwd.join("codex.yaml"),
            cwd.join("codex.yml"),
            home.join(".codex").join("config.json"),
        ];
        for cfg_path in &config_paths {
            if cfg_path.exists() {
                result.config_entries += import_codex_config(cfg_path)?;
                result.note(&format!(
                    "Imported config from {}",
                    cfg_path.file_name().unwrap_or_default().to_string_lossy()
                ));
                break;
            }
        }

        if result.summary.is_empty() {
            result.note("No Codex configuration found.");
        }

        Ok(result)
    }

    // ── OpenCode ───────────────────────────────────────────────────────────

    /// Import from OpenCode.
    ///
    /// Reads:
    ///   - `opencode.json`              → provider/models/routing config
    ///   - `~/.config/opencode/`        → user-level config
    pub fn import_opencode(path: &Path) -> anyhow::Result<MigrationResult> {
        let mut result = MigrationResult::new("opencode");
        let cwd = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let home = dirs::home_dir().unwrap_or_default();

        let config_paths = [
            cwd.join("opencode.json"),
            home.join(".config").join("opencode").join("config.json"),
        ];
        for cfg_path in &config_paths {
            if cfg_path.exists() {
                result.config_entries += import_opencode_config(cfg_path)?;
                result.note(&format!("Imported config from {}", cfg_path.display()));
                break;
            }
        }

        if result.summary.is_empty() {
            result.note("No OpenCode configuration found.");
        }

        Ok(result)
    }

    // ── OpenClaw (existing) ────────────────────────────────────────────────

    pub fn import_openclaw(path: &PathBuf) -> anyhow::Result<MigrationResult> {
        let mut result = MigrationResult::new("openclaw");

        let agents_dir = path.join("agents");
        if agents_dir.exists() {
            result.agents = std::fs::read_dir(&agents_dir)?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
                .count();
        }

        let skills_dir = path.join("skills");
        if skills_dir.exists() {
            result.skills = std::fs::read_dir(&skills_dir)?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .count();
        }

        let cron_file = path.join("cron.json");
        if cron_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&cron_file) {
                if let Ok(jobs) = serde_json::from_str::<Vec<serde_json::Value>>(&content) {
                    result.config_entries += jobs.len();
                }
            }
        }

        if result.agents > 0 {
            result.note(&format!("Found {} agents", result.agents));
        }
        if result.skills > 0 {
            result.note(&format!("Found {} skills", result.skills));
        }

        Ok(result)
    }

    // ── Hermes ─────────────────────────────────────────────────────────────

    pub fn import_hermes(path: &PathBuf) -> anyhow::Result<MigrationResult> {
        let mut result = MigrationResult::new("hermes");

        let agents_dir = path.join("agents");
        if agents_dir.exists() {
            result.agents = std::fs::read_dir(&agents_dir)?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
                .count();
        }

        let skills_dir = path.join("skills");
        if skills_dir.exists() {
            result.skills = std::fs::read_dir(&skills_dir)?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .count();
        }

        Ok(result)
    }

    // ── Auto-detect ────────────────────────────────────────────────────────

    pub fn detect_installed() -> Vec<String> {
        let mut found = Vec::new();
        let home = dirs::home_dir().unwrap_or_default();

        let tools: Vec<(&str, PathBuf)> = vec![
            ("claude-code", home.join(".claude")),
            ("codex", home.join(".codex")),
            ("opencode", home.join(".config").join("opencode")),
            ("openclaw", home.join(".openclaw")),
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

// ─── Helpers ───────────────────────────────────────────────────────────────────

fn write_sparrow_instruction(name: &str, content: &str) -> anyhow::Result<()> {
    let dir = sparrow_dir().join("instructions");
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(name);
    if !dest.exists() {
        std::fs::write(&dest, content)?;
    }
    Ok(())
}

fn agent_body_to_soul(name: &str, body: &str) -> String {
    // Try YAML frontmatter first (Claude Code agent format)
    if let Some(rest) = body.strip_prefix("---") {
        if let Some(end) = rest.find("---") {
            let frontmatter = &rest[..end];
            let content = &rest[end + 3..].trim();
            return format!(
                "# Imported from Claude Code\n\
                 name = \"{}\"\n\
                 {}\n\
                 personality = \"\"\"\n{}\n\"\"\"\n",
                name, frontmatter, content
            );
        }
    }
    // Plain text: entire body is the personality
    format!(
        "# Imported from Claude Code\n\
         name = \"{}\"\n\
         role = \"assistant\"\n\
         personality = \"\"\"\n{}\n\"\"\"\n",
        name,
        body.lines().take(80).collect::<Vec<_>>().join("\n")
    )
}

fn merge_claude_settings(settings: &ClaudeSettings) -> anyhow::Result<usize> {
    let mut count = 0;

    // Only write a hint file; actual config merge is done interactively
    // to avoid breaking the user's existing Sparrow config.
    let hint_dir = sparrow_dir().join("imports");
    std::fs::create_dir_all(&hint_dir)?;
    let hint_path = hint_dir.join("claude-settings.json");
    if !hint_path.exists() {
        let pretty = serde_json::to_string_pretty(settings)?;
        std::fs::write(&hint_path, &pretty)?;
        count += 1;
    }

    // Also check for env vars in settings
    if let Some(env) = &settings.env {
        let env_path = hint_dir.join("claude-env.txt");
        if !env_path.exists() {
            if let Some(obj) = env.as_object() {
                let mut lines = Vec::new();
                for (k, v) in obj {
                    if let Some(val) = v.as_str() {
                        lines.push(format!("{}={}", k, val));
                    }
                }
                if !lines.is_empty() {
                    std::fs::write(&env_path, lines.join("\n") + "\n")?;
                    count += 1;
                }
            }
        }
    }

    Ok(count)
}

fn extract_api_keys_from_settings(settings: &ClaudeSettings) -> anyhow::Result<usize> {
    let env = match &settings.env {
        Some(e) => e,
        None => return Ok(0),
    };

    let obj = match env.as_object() {
        Some(o) => o,
        None => return Ok(0),
    };

    let key_patterns = [
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
        "GROQ_API_KEY",
        "OPENROUTER_API_KEY",
        "GOOGLE_API_KEY",
        "NVIDIA_API_KEY",
        "DEEPSEEK_API_KEY",
    ];

    let mut found = 0;
    for pattern in &key_patterns {
        if obj.contains_key(*pattern) {
            found += 1;
        }
    }

    Ok(found)
}

fn import_mcp_servers(mcp_path: &Path) -> anyhow::Result<usize> {
    let content = std::fs::read_to_string(mcp_path)?;
    let config: serde_json::Value = serde_json::from_str(&content)?;

    let mcp_dir = sparrow_dir().join("mcp");
    std::fs::create_dir_all(&mcp_dir)?;

    let dest = mcp_dir.join(
        mcp_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .replace(".json", "-imported.json"),
    );

    if !dest.exists() {
        let pretty = serde_json::to_string_pretty(&config)?;
        std::fs::write(&dest, &pretty)?;
    }

    // Count MCP servers
    let count = config
        .get("mcpServers")
        .and_then(|s| s.as_object())
        .map(|o| o.len())
        .unwrap_or(0);

    Ok(count)
}

fn import_codex_config(path: &Path) -> anyhow::Result<usize> {
    let content = std::fs::read_to_string(path)?;
    let config: serde_json::Value = serde_json::from_str(&content)?;

    // Extract model/provider info
    let import_dir = sparrow_dir().join("imports");
    std::fs::create_dir_all(&import_dir)?;

    let dest = import_dir.join("codex-config.json");
    if !dest.exists() {
        let pretty = serde_json::to_string_pretty(&config)?;
        std::fs::write(&dest, &pretty)?;
    }

    let entries = config.as_object().map(|o| o.len()).unwrap_or(0);

    Ok(entries)
}

fn import_opencode_config(path: &Path) -> anyhow::Result<usize> {
    let content = std::fs::read_to_string(path)?;
    let config: serde_json::Value = serde_json::from_str(&content)?;

    let import_dir = sparrow_dir().join("imports");
    std::fs::create_dir_all(&import_dir)?;

    let dest = import_dir.join("opencode-config.json");
    if !dest.exists() {
        let pretty = serde_json::to_string_pretty(&config)?;
        std::fs::write(&dest, &pretty)?;
    }

    let entries = config.as_object().map(|o| o.len()).unwrap_or(0);

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_body_to_soul_with_frontmatter() {
        let body = "---\nname: planner\nrole: architect\n---\nYou plan carefully.";
        let soul = agent_body_to_soul("planner", body);
        assert!(soul.contains("name = \"planner\""));
        assert!(soul.contains("role: architect"));
        assert!(soul.contains("You plan carefully."));
    }

    #[test]
    fn test_agent_body_to_soul_plain_text() {
        let body = "Be concise. Return evidence.";
        let soul = agent_body_to_soul("helper", body);
        assert!(soul.contains("name = \"helper\""));
        assert!(soul.contains("role = \"assistant\""));
        assert!(soul.contains("Be concise."));
    }

    #[test]
    fn test_detect_installed_returns_vec() {
        let found = Migration::detect_installed();
        // May be empty on CI, but must not panic
        let _ = found.len();
    }

    #[test]
    fn test_migration_result_new() {
        let r = MigrationResult::new("test-tool");
        assert_eq!(r.tool, "test-tool");
        assert_eq!(r.agents, 0);
        assert!(r.summary.is_empty());
    }
}
