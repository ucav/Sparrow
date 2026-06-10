// src/cmd_handlers/handle_plugins_cmd.rs
pub fn handle_plugins(
    action: sparrow::cli::PluginsAction,
    config_dir: &std::path::Path,
) -> anyhow::Result<()> {
    let plugins_dir = config_dir.join("plugins");
    match action {
        sparrow::cli::PluginsAction::List => {
            let registry = sparrow::capabilities::plugin::PluginRegistry::new(plugins_dir);
            let plugins = registry.scan();
            if plugins.is_empty() {
                println!("No plugins installed.");
            } else {
                println!("Plugins ({}):", plugins.len());
                for plugin in plugins {
                    let audit = registry.audit(&plugin);
                    println!(
                        "  {} {} | commands:{} skills:{} hooks:{} | {}",
                        plugin.manifest.name,
                        plugin.manifest.version,
                        plugin.manifest.commands.len(),
                        plugin.manifest.skills.len(),
                        plugin.manifest.hooks.len(),
                        if audit.allowed { "allowed" } else { "blocked" }
                    );
                    for warning in audit.warnings {
                        println!("    - {}", warning);
                    }
                }
            }
        }
        sparrow::cli::PluginsAction::Install { source, allow } => {
            let source_path = std::path::PathBuf::from(&source);
            let mut allowlist = Vec::new();
            if allow {
                if let Ok(plugin) = sparrow::capabilities::plugin::load_plugin(&source_path) {
                    allowlist.push(plugin.manifest.name);
                }
            }
            let registry = sparrow::capabilities::plugin::PluginRegistry::new(plugins_dir)
                .with_allowlist(allowlist);
            let plugin = if source.starts_with("http://")
                || source.starts_with("https://")
                || source.ends_with(".git")
                || source.contains("github.com")
            {
                registry.install_github(&source)?
            } else {
                registry.install_local(&source_path)?
            };
            println!("Installed plugin '{}'.", plugin.manifest.name);
        }
        sparrow::cli::PluginsAction::Rm { name } => {
            let path = plugins_dir.join(&name);
            if path.exists() {
                std::fs::remove_dir_all(path)?;
                println!("Removed plugin '{}'.", name);
            } else {
                println!("No plugin named '{}'.", name);
            }
        }
    }
    Ok(())
}
