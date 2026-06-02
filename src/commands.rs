use crate::capabilities::{Skill, SkillLibrary};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashCommandSource {
    Builtin,
    Project(PathBuf),
    User(PathBuf),
    Skill(String),
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
        "sessions",
        "List or resume saved sessions.",
        "Open session workflow.",
    ),
    (
        "export",
        "Export transcript, events, and artifacts.",
        "Export current run/session.",
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
    library.all().into_iter().map(skill_to_command).collect()
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
    for cmd in load_markdown_commands(project_root, config_dir) {
        by_name.insert(cmd.name.clone(), cmd);
    }
    by_name.into_values().collect()
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
        }]));
        let commands = all_commands(Path::new("."), Path::new("."), Some(&skills));
        assert!(commands.iter().any(|c| c.name == "fix-ci"));
    }
}
