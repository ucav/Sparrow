// src/cmd_handlers/handle_memory_cmd.rs — extracted from main.rs

fn handle_memory(
    action: sparrow::cli::MemoryAction,
    memory: &Arc<dyn Memory>,
    state_dir: &std::path::Path,
) -> anyhow::Result<()> {
    match action {
        sparrow::cli::MemoryAction::List => {
            let facts = memory.all_facts();
            let stats = memory.memory_stats();
            println!(
                "Memory docs: MEMORY.md {}/{} chars, USER.md {}/{} chars",
                stats.memory_chars, stats.memory_limit, stats.user_chars, stats.user_limit
            );
            if facts.is_empty() {
                println!("No facts stored. Facts are auto-distilled from successful runs.");
            } else {
                println!("Stored facts ({}):", facts.len());
                for f in &facts {
                    println!("  {}  {}: {}", f.id, f.key, f.value);
                }
            }
        }
        sparrow::cli::MemoryAction::Forget { id } => {
            memory.forget(&id)?;
            println!("Fact '{}' forgotten.", id);
        }
        sparrow::cli::MemoryAction::Add { key, value } => {
            let fact = sparrow::memory::Fact {
                id: uuid::Uuid::new_v4().to_string(),
                key,
                value,
                created_at: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                updated_at: chrono::Utc::now().format("%Y-%m-%d").to_string(),
            };
            memory.remember(fact)?;
            println!("Fact added.");
        }
        sparrow::cli::MemoryAction::Replace { id, key, value } => {
            let fact = sparrow::memory::Fact {
                id: id.clone(),
                key,
                value,
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            };
            memory.remember(fact)?;
            println!("Fact '{}' replaced.", id);
        }
        sparrow::cli::MemoryAction::Recall { query, limit } => {
            let facts = memory.recall(&query, limit);
            if facts.is_empty() {
                println!("No facts match '{}'.", query);
            } else {
                println!("Matching facts ({}):", facts.len());
                for f in &facts {
                    println!("  {}  {}: {}", f.id, f.key, f.value);
                }
            }
        }
        sparrow::cli::MemoryAction::Consolidate => {
            memory.consolidate_memory()?;
            println!("Memory consolidated into bounded MEMORY.md/USER.md docs.");
        }
        sparrow::cli::MemoryAction::Docs => {
            let mut found = false;
            for kind in [
                sparrow::memory::MemoryDocKind::Memory,
                sparrow::memory::MemoryDocKind::User,
            ] {
                if let Some(doc) = memory.memory_doc(kind) {
                    found = true;
                    println!("## {} ({})", kind.as_str(), doc.updated_at);
                    println!("{}", doc.content);
                    println!();
                }
            }
            if !found {
                println!("No MEMORY.md/USER.md docs stored yet.");
            }
        }
        sparrow::cli::MemoryAction::Search { query, limit } => {
            let store =
                sparrow::runtime::session::SessionStore::open(&state_dir.join("sessions.db"))?;
            let hits = store.search(&query, limit);
            if hits.is_empty() {
                println!("No sessions match '{}'.", query);
            } else {
                for hit in hits {
                    println!(
                        "{} #{} [{}] {}",
                        hit.session_id,
                        hit.turn_index,
                        hit.role,
                        hit.text.replace('\n', " ")
                    );
                }
            }
        }
        sparrow::cli::MemoryAction::Scroll {
            session,
            around,
            before,
            after,
        } => {
            let store =
                sparrow::runtime::session::SessionStore::open(&state_dir.join("sessions.db"))?;
            if let Some(slice) = store.scroll(&session, around, before, after) {
                for (offset, msg) in slice.messages.iter().enumerate() {
                    let idx = slice.start + offset;
                    let text = msg
                        .content
                        .iter()
                        .filter_map(|block| match block {
                            sparrow::provider::ContentBlock::Text { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    println!("#{} [{}] {}", idx, msg.role, text.replace('\n', " "));
                }
            } else {
                println!("Session '{}' not found.", session);
            }
        }
        sparrow::cli::MemoryAction::Graph { action } => {
            sparrow::cmd_handlers::handle_memory_graph_cmd::handle_memory_graph(action, memory)?
        }
    }
    Ok(())
}

fn parse_json_properties(raw: &str) -> anyhow::Result<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_str(raw)
        .map_err(|e| anyhow::anyhow!("properties must be valid JSON object: {}", e))?;
    if !value.is_object() {
        anyhow::bail!("properties must be a JSON object");
    }
    Ok(value)
}

// ─── JSON NDJSON run ────────────────────────────────────────────────────────────

fn current_repo_head() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let head = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if head.is_empty() { None } else { Some(head) }
}

