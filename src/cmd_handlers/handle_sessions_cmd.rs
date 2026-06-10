// src/cmd_handlers/handle_sessions_cmd.rs — extracted from main.rs

fn handle_sessions(
    action: sparrow::cli::SessionAction,
    state_dir: &std::path::Path,
) -> anyhow::Result<()> {
    let store = sparrow::runtime::session::SessionStore::open(&state_dir.join("sessions.db"))?;
    match action {
        sparrow::cli::SessionAction::List => {
            let sessions = store.list();
            if sessions.is_empty() {
                println!("No sessions stored.");
            } else {
                println!("Sessions ({}):", sessions.len());
                for session in sessions {
                    println!(
                        "  {} | status:{} | updated:{} | {} bytes",
                        session.id,
                        session.status,
                        session.updated_at,
                        session.messages_json.len()
                    );
                }
            }
        }
        sparrow::cli::SessionAction::Export { id, path } => {
            let Some(session) = store.load(&id) else {
                anyhow::bail!("session '{}' not found", id);
            };
            let output = path.unwrap_or_else(|| {
                state_dir.join(format!("session-{}.json", sanitize_file_component(&id)))
            });
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&output, serde_json::to_string_pretty(&session)?)?;
            println!("Exported session '{}' to {}", id, output.display());
        }
        sparrow::cli::SessionAction::Cleanup { older_than_days } => {
            let cutoff = chrono::Utc::now().timestamp() - (older_than_days as i64 * 86_400);
            let mut removed = 0usize;
            for session in store.list() {
                if session.updated_at < cutoff {
                    store.delete(&session.id)?;
                    removed += 1;
                }
            }
            println!(
                "Removed {} session(s) older than {} day(s).",
                removed, older_than_days
            );
        }
        sparrow::cli::SessionAction::Search { query, limit } => {
            let fts = sparrow::memory::fts::SessionSearch::open(state_dir.join("sessions_fts.db"))?;
            let hits = fts.search(&query, limit)?;
            if hits.is_empty() {
                println!("No results for \"{}\".", query);
            } else {
                println!("Results for \"{}\" ({}):\n", query, hits.len());
                for hit in &hits {
                    let title = hit.title.as_deref().unwrap_or("(untitled)");
                    println!("  {} — {}", hit.session_id, title);
                    println!("    {}", hit.snippet);
                    println!();
                }
            }
        }
    }
    Ok(())
}

// ─── Profile commands ───────────────────────────────────────────────────────────
