//! Claude Code drop-in compatibility.
//!
//! Reads existing Claude Code config so users can migrate to Sparrow without
//! moving files. Specifically:
//!
//! - `~/.claude/CLAUDE.md`             → imported as user-level memory
//! - `~/.claude/commands/*.md`         → registered as slash commands
//! - `~/.claude/agents/*.md`           → loaded as SOUL agents
//! - `~/.claude/settings.json`         → mapped to `Sparrow` config (permissions, hooks)
//! - `.claude/CLAUDE.md` in cwd        → imported as project-level memory
//! - `.claude/commands/*.md` in cwd    → registered as project slash commands
//!
//! All reads are best-effort and never overwrite the user's Sparrow config.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClaudeImport {
    pub user_memory: Option<String>,
    pub project_memory: Option<String>,
    pub commands: Vec<ClaudeCommand>,
    pub agents: Vec<ClaudeAgentFile>,
    pub settings: Option<ClaudeSettings>,
    pub source_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeCommand {
    pub name: String,
    pub body: String,
    pub source: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeAgentFile {
    pub name: String,
    pub body: String,
    pub source: PathBuf,
}

/// Subset of Claude Code settings.json we honor when importing.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClaudeSettings {
    #[serde(default)]
    pub permissions: Option<serde_json::Value>,
    #[serde(default)]
    pub hooks: Option<serde_json::Value>,
    #[serde(default)]
    pub env: Option<serde_json::Value>,
}

/// Discover Claude Code config on disk and return what we can import.
///
/// Looks in:
///   - `$HOME/.claude/` for user-level config
///   - `<cwd>/.claude/` for project-level config
pub fn discover(home: &Path, cwd: &Path) -> ClaudeImport {
    let mut out = ClaudeImport::default();

    let user_root = home.join(".claude");
    if user_root.is_dir() {
        out.source_paths.push(user_root.clone());
        out.user_memory = read_optional(user_root.join("CLAUDE.md"));
        out.commands
            .extend(load_dir(&user_root.join("commands"), "command"));
        out.agents
            .extend(load_dir_as_agents(&user_root.join("agents")));
        out.settings = read_optional(user_root.join("settings.json"))
            .and_then(|s| serde_json::from_str(&s).ok());
    }

    let project_root = cwd.join(".claude");
    if project_root.is_dir() {
        out.source_paths.push(project_root.clone());
        out.project_memory = read_optional(project_root.join("CLAUDE.md"));
        out.commands
            .extend(load_dir(&project_root.join("commands"), "command"));
        out.agents
            .extend(load_dir_as_agents(&project_root.join("agents")));
    }

    out
}

fn read_optional(p: PathBuf) -> Option<String> {
    if p.is_file() {
        std::fs::read_to_string(&p).ok()
    } else {
        None
    }
}

fn load_dir(dir: &Path, _kind: &str) -> Vec<ClaudeCommand> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(body) = std::fs::read_to_string(&path) else {
            continue;
        };
        out.push(ClaudeCommand {
            name: stem.to_string(),
            body,
            source: path,
        });
    }
    out
}

fn load_dir_as_agents(dir: &Path) -> Vec<ClaudeAgentFile> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(body) = std::fs::read_to_string(&path) else {
            continue;
        };
        out.push(ClaudeAgentFile {
            name: stem.to_string(),
            body,
            source: path,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn discover_picks_up_user_claude_md() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let cdir = home.join(".claude");
        fs::create_dir_all(&cdir).unwrap();
        fs::write(cdir.join("CLAUDE.md"), "# rules\nbe concise").unwrap();

        let cwd = tempfile::tempdir().unwrap();
        let imported = discover(home, cwd.path());
        assert!(imported.user_memory.as_deref().unwrap().contains("rules"));
        assert!(imported.project_memory.is_none());
    }

    #[test]
    fn discover_loads_commands_and_agents() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let cmds = home.join(".claude").join("commands");
        let agents = home.join(".claude").join("agents");
        fs::create_dir_all(&cmds).unwrap();
        fs::create_dir_all(&agents).unwrap();
        fs::write(cmds.join("hello.md"), "/hello body").unwrap();
        fs::write(agents.join("planner.md"), "---\nname: planner\n---\nbody").unwrap();

        let cwd = tempfile::tempdir().unwrap();
        let imported = discover(home, cwd.path());
        assert_eq!(imported.commands.len(), 1);
        assert_eq!(imported.commands[0].name, "hello");
        assert_eq!(imported.agents.len(), 1);
        assert_eq!(imported.agents[0].name, "planner");
    }

    #[test]
    fn discover_returns_empty_when_no_claude_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let imported = discover(tmp.path(), tmp.path());
        assert!(imported.user_memory.is_none());
        assert!(imported.commands.is_empty());
        assert!(imported.agents.is_empty());
    }
}
