// src/cmd_handlers/handle_skills_cmd.rs
use super::prelude::*;
pub fn handle_skills(
    action: sparrow::cli::SkillsAction,
    library: &Arc<dyn SkillLibrary>,
) -> anyhow::Result<()> {
    match action {
        sparrow::cli::SkillsAction::List => {
            let skills = library.all();
            if skills.is_empty() {
                println!("No skills in library.");
                println!("Skills are automatically learned from successful runs.");
                println!("Create one manually: sparrow skills create <name>");
            } else {
                println!("Skill library ({} skills):", skills.len());
                for s in &skills {
                    let tag = if s.auto_generated { "[auto]" } else { "[user]" };
                    println!(
                        "  {} {} | triggers: {} | score: {:.2} | used: {}",
                        tag,
                        s.name,
                        s.trigger.join(", "),
                        s.score,
                        s.usage_count
                    );
                }
            }
        }
        sparrow::cli::SkillsAction::View { name } => match library.invoke(&name)? {
            Some(invocation) => {
                println!("# {}", invocation.skill.name);
                println!("{}", invocation.skill.description);
                println!("Triggers: {}", invocation.skill.trigger.join(", "));
                println!();
                println!("{}", invocation.skill.body);
                if !invocation.loaded_references.is_empty() {
                    println!("\nLoaded references:");
                    for (path, content) in invocation.loaded_references {
                        println!("## {}", path);
                        println!("{}", content);
                    }
                }
            }
            None => println!("No skill named '{}'.", name),
        },
        sparrow::cli::SkillsAction::Create { name } => {
            let skill = sparrow::capabilities::Skill {
                name: name.clone(),
                description: format!("User-created skill: {}", name),
                trigger: vec![name.to_lowercase()],
                body: format!("# {}\n\nAdd skill content here.", name),
                source_file: format!("{}.skill.md", name),
                usage_count: 0,
                created_at: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                score: 0.5,
                auto_generated: false,
                references: Vec::new(),
                templates: Vec::new(),
                scripts: Vec::new(),
                assets: Vec::new(),
            };
            library.add(skill)?;
            println!(
                "Skill '{}' created. Edit: ~/.config/sparrow/skills/{}/SKILL.md",
                name, name
            );
        }
        sparrow::cli::SkillsAction::Install { source } => {
            let skill = load_skill_from_source(&source)?;
            let name = skill.name.clone();
            library.add(skill)?;
            println!("Installed skill '{}'.", name);
        }
        sparrow::cli::SkillsAction::Update { name } => {
            let Some(skill) = library.get(&name) else {
                println!("No skill named '{}'.", name);
                return Ok(());
            };
            library.add(skill)?;
            println!("Skill '{}' refreshed.", name);
        }
        sparrow::cli::SkillsAction::Prune => {
            let removed = library.prune(0.2)?;
            println!(
                "Curator pruned {} low-score auto-generated skill(s).",
                removed
            );
            let skills = library.all();
            println!("Library now has {} skills.", skills.len());
        }
        sparrow::cli::SkillsAction::Rm { name } => {
            if library.remove(&name)? {
                println!("Removed skill '{}'.", name);
            } else {
                println!(
                    "No skill named '{}'. Run 'sparrow skills list' to see names.",
                    name
                );
            }
        }
    }
    Ok(())
}

pub fn load_skill_from_source(source: &str) -> anyhow::Result<sparrow::capabilities::Skill> {
    let source = source.trim();

    // `gh:user/repo[/sub/path]` shorthand — expands to a GitHub clone URL and
    // an optional directory inside the repo holding the SKILL.md.
    let (clone_source, subpath): (String, Option<String>) =
        if let Some(rest) = source.strip_prefix("gh:") {
            let mut parts = rest.splitn(3, '/');
            let user = parts.next().filter(|s| !s.is_empty());
            let repo = parts.next().filter(|s| !s.is_empty());
            let sub = parts.next().map(String::from);
            match (user, repo) {
                (Some(u), Some(r)) => (format!("https://github.com/{}/{}", u, r), sub),
                _ => anyhow::bail!(
                    "invalid gh: source `{}` — expected gh:user/repo[/path/to/skill]",
                    source
                ),
            }
        } else {
            (source.to_string(), None)
        };
    let clone_source = clone_source.as_str();

    let temp_dir;
    let path = if clone_source.starts_with("http://")
        || clone_source.starts_with("https://")
        || clone_source.ends_with(".git")
        || clone_source.contains("github.com")
    {
        temp_dir = std::env::temp_dir().join(format!("sparrow-skill-{}", uuid::Uuid::new_v4()));
        let status = std::process::Command::new("git")
            .args([
                "clone",
                "--depth",
                "1",
                clone_source,
                temp_dir.to_string_lossy().as_ref(),
            ])
            .status()?;
        if !status.success() {
            anyhow::bail!("git clone failed for skill source {}", clone_source);
        }
        match &subpath {
            Some(sub) => temp_dir.join(sub),
            None => temp_dir.clone(),
        }
    } else {
        temp_dir = std::path::PathBuf::new();
        std::path::PathBuf::from(clone_source)
    };

    let skill_file = if path.is_dir() {
        path.join("SKILL.md")
    } else {
        path.clone()
    };
    let content = std::fs::read_to_string(&skill_file)?;
    let source_file = skill_file
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "SKILL.md".into());
    let skill = sparrow::capabilities::Skill::from_markdown(&content, &source_file)
        .ok_or_else(|| anyhow::anyhow!("could not parse skill from {}", skill_file.display()))?;
    if !temp_dir.as_os_str().is_empty() {
        let _ = std::fs::remove_dir_all(temp_dir);
    }
    Ok(skill)
}
