// src/cmd_handlers/handle_init_cmd.rs — extracted from main.rs

fn handle_init() -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let sparrow_dir = cwd.join(".sparrow");
    if sparrow_dir.exists() {
        println!("Project already initialized (.sparrow/ exists)");
        return Ok(());
    }
    std::fs::create_dir_all(&sparrow_dir)?;
    std::fs::create_dir_all(sparrow_dir.join("agents"))?;
    std::fs::create_dir_all(sparrow_dir.join("skills"))?;

    // Write team config template
    std::fs::write(
        sparrow_dir.join("team.toml"),
        r#"# Sparrow team config
# This file is shared via version control.
# Individual API keys go in ~/.config/sparrow/config.toml

[routing]
preferred = "nvidia"
free_first = true

[budget]
daily_per_seat_usd = 5.0

[org]
max_autonomy = "trusted"
blocked_paths = [".env", "*.pem", "secrets/"]
"#,
    )?;

    println!("Initialized .sparrow/ in {}", cwd.display());
    println!("  .sparrow/team.toml   — shared routing + budget + org policy");
    println!("  .sparrow/agents/     — team-shared agent definitions");
    println!("  .sparrow/skills/     — team-shared skills");
    println!("\nCommit .sparrow/ to your repo to share with the team.");
    Ok(())
}

// ─── Status command ────────────────────────────────────────────────────────────
