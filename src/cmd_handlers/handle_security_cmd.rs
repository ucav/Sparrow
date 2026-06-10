// src/cmd_handlers/handle_security_cmd.rs

pub fn handle_security(
    action: sparrow::cli::SecurityAction,
    config: &sparrow::config::Config,
) -> anyhow::Result<()> {
    match action {
        sparrow::cli::SecurityAction::Audit { json } => {
            let audit = sparrow::security::SecurityAudit::run(config, &config.hooks);
            if json {
                println!("{}", audit.to_json());
            } else {
                println!("{}", audit.summary());
                for f in &audit.findings {
                    let tag = match f.severity {
                        sparrow::security::Severity::Critical => "CRIT",
                        sparrow::security::Severity::Warning => "WARN",
                        sparrow::security::Severity::Info => "INFO",
                    };
                    println!("  [{}] {}: {}", tag, f.category, f.message);
                    if !f.recommendation.is_empty() {
                        println!("        → {}", f.recommendation);
                    }
                }
            }
        }
    }
    Ok(())
}

// ─── Context compaction / handoff ───────────────────────────────────────────────
