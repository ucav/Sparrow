// src/cmd_handlers/handle_profile_cmd.rs
pub fn handle_profile(
    action: sparrow::cli::ProfileAction,
    config_dir: &std::path::PathBuf,
    state_dir: &std::path::PathBuf,
) -> anyhow::Result<()> {
    match action {
        sparrow::cli::ProfileAction::Create { name } => {
            let profile_dir = config_dir.join("profiles").join(&name);
            std::fs::create_dir_all(&profile_dir)?;
            // Copy default config as starting point
            let default_config = config_dir.join("config.toml");
            if default_config.exists() {
                std::fs::copy(&default_config, profile_dir.join("config.toml"))?;
            }
            std::fs::create_dir_all(state_dir.join("profiles").join(&name))?;
            println!("Profile '{}' created.", name);
            println!("Config: {:?}", profile_dir.join("config.toml"));
            println!("Use: sparrow --profile {} <command>", name);
        }
        sparrow::cli::ProfileAction::List => {
            let profiles_dir = config_dir.join("profiles");
            let active_profile = std::fs::read_to_string(config_dir.join("active_profile"))
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            if !profiles_dir.exists() {
                println!("No profiles yet. Create one with: sparrow profile create <name>");
                return Ok(());
            }
            println!("Profiles:");
            if let Ok(entries) = std::fs::read_dir(&profiles_dir) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        if let Some(name) = entry.file_name().to_str() {
                            let marker = if active_profile.as_deref() == Some(name) {
                                "*"
                            } else {
                                " "
                            };
                            println!("{} {}", marker, name);
                        }
                    }
                }
            }
        }
        sparrow::cli::ProfileAction::Use { name } => {
            let profile_config = config_dir.join("profiles").join(&name).join("config.toml");
            if !profile_config.exists() {
                anyhow::bail!("Profile '{}' not found. Create it first.", name);
            }
            std::fs::write(config_dir.join("active_profile"), &name)?;
            println!("Active profile is now '{}'.", name);
            println!("Config: {}", profile_config.display());
        }
    }
    Ok(())
}

// ─── Import command ─────────────────────────────────────────────────────────────
