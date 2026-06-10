// src/commands/import_cmd.rs — sparrow import command handler

use sparrow::onboarding::migration::Migration;

pub fn handle_full_import(source: sparrow::cli::ImportSource) -> anyhow::Result<()> {
    let cwd = || std::env::current_dir().unwrap_or_default();

    let imports: Vec<(
        &'static str,
        sparrow::onboarding::migration::MigrationResult,
    )> = match source {
        sparrow::cli::ImportSource::ClaudeCode { path } => {
            let src = path.unwrap_or_else(cwd);
            vec![("claude-code", Migration::import_claude_code(&src)?)]
        }
        sparrow::cli::ImportSource::Codex { path } => {
            let src = path.unwrap_or_else(cwd);
            vec![("codex", Migration::import_codex(&src)?)]
        }
        sparrow::cli::ImportSource::OpenCode { path } => {
            let src = path.unwrap_or_else(cwd);
            vec![("opencode", Migration::import_opencode(&src)?)]
        }
        sparrow::cli::ImportSource::Openclaw { path } => {
            let src =
                path.unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".openclaw"));
            vec![("openclaw", Migration::import_openclaw(&src)?)]
        }
        sparrow::cli::ImportSource::Auto => {
            let found = Migration::detect_installed();
            if found.is_empty() {
                println!(
                    "No supported tools detected. Supported: claude-code, codex, opencode, openclaw, hermes"
                );
                return Ok(());
            }
            println!("Detected: {}", found.join(", "));
            let cwd = cwd();
            let home = dirs::home_dir().unwrap_or_default();
            let mut all = Vec::new();
            for tool in &found {
                let imported = match tool.as_str() {
                    "claude-code" => {
                        Migration::import_claude_code(&cwd).map(|r| ("claude-code", r))
                    }
                    "codex" => Migration::import_codex(&cwd).map(|r| ("codex", r)),
                    "opencode" => Migration::import_opencode(&cwd).map(|r| ("opencode", r)),
                    "openclaw" => {
                        Migration::import_openclaw(&home.join(".openclaw")).map(|r| ("openclaw", r))
                    }
                    "hermes" => {
                        Migration::import_hermes(&home.join(".hermes")).map(|r| ("hermes", r))
                    }
                    other => {
                        println!("  (skipping {}: not supported for auto-import)", other);
                        continue;
                    }
                };
                match imported {
                    Ok(pair) => all.push(pair),
                    Err(e) => eprintln!("  ⚠ import from {} failed: {}", tool, e),
                }
            }
            all
        }
    };

    for (tool_name, result) in &imports {
        println!("═══ Import from {} ═══", tool_name);
        for line in &result.summary {
            println!("  {}", line);
        }
        if result.agents > 0 || result.commands > 0 || result.config_entries > 0 {
            println!(
                "\nDone: {} agents, {} commands, {} config entries imported.",
                result.agents, result.commands, result.config_entries
            );
        }
    }
    Ok(())
}
