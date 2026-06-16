use crate::capabilities::{Skill, SkillLibrary, is_unfit_for_skill};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashCommandSource {
    Builtin,
    Project(PathBuf),
    User(PathBuf),
    Skill(String),
    Plugin(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashCommand {
    pub name: String,
    pub description: String,
    pub body: String,
    pub source: SlashCommandSource,
}

const BUILTINS: &[(&str, &str, &str)] = &[
    (
        "help",
        "Show available Sparrow slash commands.",
        "List commands and short usage.",
    ),
    (
        "plan",
        "Create a read-only plan before running a task.",
        "Usage: /plan <task>",
    ),
    (
        "permissions",
        "Inspect or change permission policy.",
        "Open permissions workflow.",
    ),
    (
        "memory",
        "Inspect or manage persistent memory.",
        "Open memory workflow.",
    ),
    (
        "compact",
        "Compact current context into a durable handoff.",
        "Create a session summary.",
    ),
    (
        "model",
        "Inspect or change routing/model configuration.",
        "Open model workflow.",
    ),
    (
        "agents",
        "List and mention configured agents.",
        "Open agent workflow.",
    ),
    (
        "agent",
        "Manage persistent Sparrow agents.",
        "Usage: /agent <create|list|show|delete|edit|export|import|default|route|doctor|materialize> ...",
    ),
    (
        "sessions",
        "List or resume saved sessions.",
        "Open session workflow.",
    ),
    (
        "export",
        "Export transcript, events, and artifacts.",
        "Export current run/session.",
    ),
    (
        "run",
        "Run an agentic task from the WebView.",
        "Usage: /run <task>",
    ),
    (
        "launch",
        "Start the first-run setup if needed, then open the WebView cockpit.",
        "Terminal: sparrow launch [--port 9339] [--tui]",
    ),
    (
        "models",
        "List configured providers and discovered models.",
        "Usage: /models",
    ),
    (
        "config",
        "Open provider and routing configuration.",
        "Usage: /config",
    ),
    (
        "tools",
        "List available toolsets and tool schemas.",
        "Usage: /tools",
    ),
    (
        "security",
        "Show the current security and audit state.",
        "Usage: /security",
    ),
    (
        "status",
        "Show active run, budget, and session status.",
        "Usage: /status",
    ),
    (
        "plugins",
        "List installed Sparrow plugins.",
        "Usage: /plugins",
    ),
    (
        "skills",
        "List or manage reusable Sparrow skills.",
        "Usage: /skills list",
    ),
    (
        "agents",
        "List and mention configured agents.",
        "Usage: /agents",
    ),
    (
        "sessions",
        "List or resume saved sessions.",
        "Usage: /sessions",
    ),
    (
        "routing",
        "Inspect routing preferences and fallbacks.",
        "Usage: /routing",
    ),
    (
        "route",
        "Configure intelligent auto-routing.",
        "Usage: /route <show|set|reset|prefer|discover>",
    ),
    ("auth", "Manage provider credentials.", "Usage: /auth list"),
    (
        "schedule",
        "Schedule a periodic Sparrow task.",
        "Usage: /schedule <task> --cron <expr>",
    ),
    (
        "github",
        "Run GitHub workflow helpers.",
        "Usage: /github <action>",
    ),
    (
        "checkpoint",
        "List available rollback checkpoints.",
        "Usage: /checkpoint list",
    ),
    (
        "rewind",
        "Rewind the workspace to a checkpoint.",
        "Usage: /rewind <checkpoint-id>",
    ),
    (
        "replay",
        "Replay a previous Sparrow run transcript.",
        "Usage: /replay <run-id>",
    ),
    ("mcp", "Manage MCP connectors.", "Usage: /mcp <action>"),
    (
        "profile",
        "Manage Sparrow profiles.",
        "Usage: /profile <list|show|switch|create|delete> ...",
    ),
    (
        "import",
        "Import configuration from another agent CLI.",
        "Usage: /import <openclaw>",
    ),
    (
        "learn",
        "Open the interactive Sparrow tutorial.",
        "Usage: /learn",
    ),
    (
        "init",
        "Initialize .sparrow configuration in this project.",
        "Usage: /init",
    ),
    (
        "doctor",
        "Run diagnostics for providers, config, tools, and workspace.",
        "Usage: /doctor",
    ),
    (
        "update",
        "Check for a Sparrow self-update.",
        "Usage: /update",
    ),
    (
        "setup",
        "Run the first-launch provider and routing setup.",
        "Usage: /setup",
    ),
    ("clear", "Clear the WebView transcript.", "Usage: /clear"),
    (
        "reset",
        "Reset the current WebView conversation.",
        "Usage: /reset",
    ),
    ("stop", "Stop the current run.", "Usage: /stop"),
    (
        "upload",
        "Attach files to the next message.",
        "Use the paperclip button or drag files into the WebView.",
    ),
    (
        "console",
        "Launch the WebView console from a terminal.",
        "Terminal only: `/console` is blocked inside the WebView to avoid nesting.",
    ),
    (
        "tui",
        "Launch the terminal TUI.",
        "Terminal only: `/tui` is blocked inside the WebView because it is interactive.",
    ),
    (
        "chat",
        "Launch interactive multi-turn terminal chat.",
        "Terminal only: `/chat` is blocked inside the WebView because it is interactive.",
    ),
    (
        "daemon",
        "Run the headless Sparrow runtime daemon.",
        "Terminal only: `/daemon` is blocked inside the WebView because it keeps running.",
    ),
];

pub fn builtin_commands() -> Vec<SlashCommand> {
    BUILTINS
        .iter()
        .map(|(name, description, body)| SlashCommand {
            name: (*name).into(),
            description: (*description).into(),
            body: (*body).into(),
            source: SlashCommandSource::Builtin,
        })
        .collect()
}

pub fn command_dirs(project_root: &Path, config_dir: &Path) -> Vec<(PathBuf, SlashCommandSource)> {
    vec![
        (
            project_root.join(".sparrow").join("commands"),
            SlashCommandSource::Project(project_root.join(".sparrow").join("commands")),
        ),
        (
            config_dir.join("commands"),
            SlashCommandSource::User(config_dir.join("commands")),
        ),
    ]
}

pub fn load_markdown_commands(project_root: &Path, config_dir: &Path) -> Vec<SlashCommand> {
    let mut out = Vec::new();
    for (dir, source) in command_dirs(project_root, config_dir) {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path
                .extension()
                .map(|ext| ext.eq_ignore_ascii_case("md"))
                .unwrap_or(false)
            {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            if let Some(cmd) = parse_markdown_command(&path, &content, source.clone()) {
                out.push(cmd);
            }
        }
    }
    out
}

pub fn skill_commands(library: &dyn SkillLibrary) -> Vec<SlashCommand> {
    library
        .all()
        .into_iter()
        .filter(|skill| {
            !is_unfit_for_skill(&skill.name)
                && !is_unfit_for_skill(&skill.description)
                && !is_unfit_for_skill(&skill.body)
        })
        .map(skill_to_command)
        .collect()
}

pub fn all_commands(
    project_root: &Path,
    config_dir: &Path,
    skills: Option<&dyn SkillLibrary>,
) -> Vec<SlashCommand> {
    let mut by_name = BTreeMap::new();
    for cmd in builtin_commands() {
        by_name.insert(cmd.name.clone(), cmd);
    }
    if let Some(skills) = skills {
        for cmd in skill_commands(skills) {
            by_name.insert(cmd.name.clone(), cmd);
        }
    }
    for cmd in plugin_commands(project_root, config_dir) {
        by_name.insert(cmd.name.clone(), cmd);
    }
    for cmd in load_markdown_commands(project_root, config_dir) {
        by_name.insert(cmd.name.clone(), cmd);
    }
    by_name.into_values().collect()
}

pub fn plugin_commands(project_root: &Path, config_dir: &Path) -> Vec<SlashCommand> {
    let dirs = [
        project_root.join(".sparrow").join("plugins"),
        config_dir.join("plugins"),
    ];
    let mut out = Vec::new();
    for dir in dirs {
        let registry = crate::capabilities::plugin::PluginRegistry::new(dir);
        for plugin in registry.scan() {
            let audit = registry.audit(&plugin);
            if !audit.allowed {
                continue;
            }
            for command in &plugin.manifest.commands {
                out.push(SlashCommand {
                    name: crate::capabilities::plugin::namespace(
                        &plugin.manifest.name,
                        &command.name,
                    ),
                    description: if command.description.is_empty() {
                        format!("Plugin command from {}", plugin.manifest.name)
                    } else {
                        command.description.clone()
                    },
                    body: command.body.clone(),
                    source: SlashCommandSource::Plugin(plugin.manifest.name.clone()),
                });
            }
            for skill in &plugin.manifest.skills {
                out.push(SlashCommand {
                    name: crate::capabilities::plugin::namespace(
                        &plugin.manifest.name,
                        &skill.name,
                    ),
                    description: format!("Plugin skill from {}", plugin.manifest.name),
                    body: format!("Invoke plugin skill '{}'.", skill.name),
                    source: SlashCommandSource::Plugin(plugin.manifest.name.clone()),
                });
            }
        }
    }
    out
}

fn skill_to_command(skill: Skill) -> SlashCommand {
    SlashCommand {
        name: slug(&skill.name),
        description: if skill.description.is_empty() {
            format!("Invoke skill '{}'.", skill.name)
        } else {
            skill.description.clone()
        },
        body: skill.body.clone(),
        source: SlashCommandSource::Skill(skill.name),
    }
}

fn parse_markdown_command(
    path: &Path,
    content: &str,
    source: SlashCommandSource,
) -> Option<SlashCommand> {
    let stem = path.file_stem()?.to_string_lossy();
    let mut name = slug(&stem);
    let mut description = String::new();
    let mut body = String::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") && description.is_empty() {
            description = trimmed.trim_start_matches("# ").trim().to_string();
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("name:") {
            name = slug(rest.trim().trim_start_matches('/'));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("description:") {
            description = rest.trim().to_string();
            continue;
        }
        if trimmed.starts_with("---") {
            continue;
        }
        body.push_str(line);
        body.push('\n');
    }

    if name.is_empty() {
        return None;
    }
    Some(SlashCommand {
        name,
        description: if description.is_empty() {
            format!("Project command from {}", path.display())
        } else {
            description
        },
        body: body.trim().to_string(),
        source,
    })
}

fn slug(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.trim().trim_start_matches('/').chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::{Skill, SkillLibrary};
    use std::sync::Mutex;

    struct TestSkills(Mutex<Vec<Skill>>);

    impl SkillLibrary for TestSkills {
        fn relevant(&self, _ctx: &str, _limit: usize) -> Vec<Skill> {
            vec![]
        }
        fn add(&self, skill: Skill) -> anyhow::Result<()> {
            self.0.lock().unwrap().push(skill);
            Ok(())
        }
        fn all(&self) -> Vec<Skill> {
            self.0.lock().unwrap().clone()
        }
        fn curate(&self) -> anyhow::Result<()> {
            Ok(())
        }
        fn prune(&self, _min_score: f64) -> anyhow::Result<usize> {
            Ok(0)
        }
        fn get(&self, _name: &str) -> Option<Skill> {
            None
        }
        fn invoke(
            &self,
            _name: &str,
        ) -> anyhow::Result<Option<crate::capabilities::SkillInvocation>> {
            Ok(None)
        }
        fn remove(&self, _name: &str) -> anyhow::Result<bool> {
            Ok(false)
        }
    }

    #[test]
    fn user_command_overrides_builtin_and_project() {
        let base =
            std::env::temp_dir().join(format!("sparrow-command-test-{}", std::process::id()));
        let root = base.join("project");
        let config = base.join("config");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(root.join(".sparrow/commands")).unwrap();
        std::fs::create_dir_all(config.join("commands")).unwrap();
        std::fs::write(
            config.join("commands/plan.md"),
            "description: user plan\nuser",
        )
        .unwrap();
        std::fs::write(
            root.join(".sparrow/commands/plan.md"),
            "description: project plan\nproject",
        )
        .unwrap();

        let commands = all_commands(&root, &config, None);
        let plan = commands.iter().find(|c| c.name == "plan").unwrap();
        assert_eq!(plan.description, "user plan");
        assert_eq!(plan.body, "user");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn skill_is_exposed_as_slash_command() {
        let skills = TestSkills(Mutex::new(vec![Skill {
            name: "Fix CI".into(),
            description: "Repair CI failures.".into(),
            trigger: vec!["ci".into()],
            body: "inspect logs".into(),
            source_file: "fix-ci/SKILL.md".into(),
            usage_count: 0,
            created_at: "2026-06-02".into(),
            score: 0.8,
            auto_generated: false,
            references: Vec::new(),
            templates: Vec::new(),
            scripts: Vec::new(),
            assets: Vec::new(),
            manifest_version: None,
            allowed_tools: Vec::new(),
        }]));
        let commands = all_commands(Path::new("."), Path::new("."), Some(&skills));
        assert!(commands.iter().any(|c| c.name == "fix-ci"));
    }

    #[test]
    fn poisoned_skill_is_not_exposed_as_slash_command() {
        let skills = TestSkills(Mutex::new(vec![
            Skill {
                name: "code-review".into(),
                description: "Reusable pattern learned from: compte les fichiers .rs du projet"
                    .into(),
                trigger: vec!["review".into()],
                body: "## Approach\n.claude/worktrees/tmp/src/main.rs".into(),
                source_file: "code-review/SKILL.md".into(),
                usage_count: 0,
                created_at: "2026-06-15".into(),
                score: 0.3,
                auto_generated: true,
                references: Vec::new(),
                templates: Vec::new(),
                scripts: Vec::new(),
                assets: Vec::new(),
                manifest_version: None,
                allowed_tools: Vec::new(),
            },
            Skill {
                name: "Fix CI".into(),
                description: "Repair CI failures.".into(),
                trigger: vec!["ci".into()],
                body: "inspect logs".into(),
                source_file: "fix-ci/SKILL.md".into(),
                usage_count: 0,
                created_at: "2026-06-02".into(),
                score: 0.8,
                auto_generated: false,
                references: Vec::new(),
                templates: Vec::new(),
                scripts: Vec::new(),
                assets: Vec::new(),
                manifest_version: None,
                allowed_tools: Vec::new(),
            },
        ]));
        let commands = all_commands(Path::new("."), Path::new("."), Some(&skills));
        assert!(!commands.iter().any(|c| c.name == "code-review"));
        assert!(commands.iter().any(|c| c.name == "fix-ci"));
    }

    #[test]
    fn webview_catalog_exposes_cli_top_level_commands_with_usage() {
        let commands = builtin_commands();
        for name in [
            "doctor", "setup", "launch", "init", "profile", "import", "agent",
        ] {
            let cmd = commands
                .iter()
                .find(|cmd| cmd.name == name)
                .unwrap_or_else(|| panic!("missing builtin slash command `{name}`"));
            assert!(!cmd.description.trim().is_empty());
            assert!(!cmd.body.trim().is_empty());
        }
    }
}
