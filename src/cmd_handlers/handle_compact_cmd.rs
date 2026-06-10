// src/cmd_handlers/handle_compact_cmd.rs

pub fn handle_compact(
    task: Option<String>,
    out: Option<std::path::PathBuf>,
    json: bool,
) -> anyhow::Result<()> {
    use sparrow::context::HandoffDoc;

    let task_str = task.unwrap_or_else(|| "ad-hoc handoff".into());
    let doc = HandoffDoc::new(task_str);

    let default_path = std::path::PathBuf::from(".sparrow/handoff").join(format!(
        "{}.md",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
    ));
    let path = out.unwrap_or(default_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let md = doc.to_markdown();
    std::fs::write(&path, &md)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": true,
                "path": path.to_string_lossy(),
                "doc": doc,
            }))?
        );
    } else {
        println!("handoff written: {}", path.display());
        println!("---");
        println!("{}", md);
    }
    Ok(())
}
