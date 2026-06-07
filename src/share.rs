//! `sparrow share` — Share the latest session transcript as a GitHub Gist.
//!
//! Reads the latest session transcript from `state/transcripts/`, formats it
//! as a clean Markdown gist, and uploads it to GitHub Gist using either the
//! `gh` CLI or a direct API call. Returns the shareable URL.

use std::path::PathBuf;
use std::process::Command;

/// Run the share command.
///
/// Reads the latest transcript, formats it as a Markdown gist, and uploads it.
/// If `include_demo_gif` is true, adds a placeholder for a demo GIF at the top.
pub async fn run_share(state_dir: &std::path::Path, include_demo_gif: bool) -> anyhow::Result<()> {
    println!("🔗 Sparrow Share — partage ta session\n");

    // 1. Find the latest transcript
    let transcripts_dir = state_dir.join("transcripts");
    let latest = find_latest_transcript(&transcripts_dir)?;

    println!("📄 Transcript : {}", latest.display());

    // 2. Read and format the transcript
    let raw = std::fs::read_to_string(&latest)?;
    let formatted = format_transcript(&raw, &latest, include_demo_gif)?;

    // 3. Upload to GitHub Gist
    let filename = latest
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "sparrow-session.md".into());

    let url = upload_gist(&filename, &formatted).await?;

    // 4. Print result
    println!();
    println!("══════════════════════════════════════════════════");
    println!("  ✅ Gist créé avec succès !");
    println!("══════════════════════════════════════════════════");
    println!();
    println!("  🔗 {url}");
    println!();
    println!("  Partagé depuis Sparrow — https://github.com/ucav/Sparrow");
    println!();

    Ok(())
}

// ─── Find latest transcript ──────────────────────────────────────────────────

fn find_latest_transcript(transcripts_dir: &std::path::Path) -> anyhow::Result<PathBuf> {
    if !transcripts_dir.exists() {
        anyhow::bail!(
            "Aucun transcript trouvé. Le dossier {} n'existe pas.\n\
             → Lance une tâche Sparrow d'abord : sparrow run \"explique ce projet\"",
            transcripts_dir.display()
        );
    }

    let mut entries: Vec<_> = std::fs::read_dir(transcripts_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "json" || ext == "jsonl")
                .unwrap_or(false)
        })
        .collect();

    if entries.is_empty() {
        anyhow::bail!(
            "Aucun transcript trouvé dans {}.\n\
             → Lance une tâche Sparrow d'abord !",
            transcripts_dir.display()
        );
    }

    // Sort by modification time, newest first
    entries.sort_by(|a, b| {
        b.metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            .cmp(
                &a.metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            )
    });

    Ok(entries[0].path())
}

// ─── Format transcript ───────────────────────────────────────────────────────

fn format_transcript(
    raw: &str,
    path: &std::path::Path,
    include_demo_gif: bool,
) -> anyhow::Result<String> {
    let timestamp = path
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|t| {
            let dt: chrono::DateTime<chrono::Local> = t.into();
            dt.format("%Y-%m-%d %H:%M:%S").to_string()
        })
        .unwrap_or_else(|| "date inconnue".into());

    let mut md = String::new();

    // Header
    md.push_str(&format!("# 🐦 Sparrow Session — {timestamp}\n\n"));

    if include_demo_gif {
        md.push_str("> 🎬 *Démo GIF à venir — regarde Sparrow en action !*\n\n");
    }

    md.push_str(&format!(
        "Session générée par [Sparrow](https://github.com/ucav/Sparrow) v{version}.\n\n",
        version = env!("CARGO_PKG_VERSION"),
    ));
    md.push_str("---\n\n");

    // Try to parse the transcript as NDJSON or JSON
    let parsed = if raw.trim().starts_with('{') {
        // Single JSON object (old format)
        format_single_json_transcript(raw)
    } else if raw.trim().starts_with('[') {
        // JSON array
        format_json_array_transcript(raw)
    } else {
        // NDJSON (one JSON object per line)
        format_ndjson_transcript(raw)
    };

    match parsed {
        Ok(formatted) => md.push_str(&formatted),
        Err(_) => {
            // Fall back to raw transcript in a code block
            md.push_str("## Transcript brut\n\n");
            md.push_str("```json\n");
            // Truncate if too long (gists have limits)
            if raw.len() > 500_000 {
                md.push_str(&raw[..500_000]);
                md.push_str("\n\n// ... (transcript tronqué — trop volumineux)\n");
            } else {
                md.push_str(raw);
            }
            md.push_str("\n```\n");
        }
    }

    // Footer
    md.push_str("\n---\n");
    md.push_str("*Partagé avec Sparrow — l'agent CLI qui code avec toi.*\n");
    md.push_str("*[Installer Sparrow](https://github.com/ucav/Sparrow) | cargo install sparrow*\n");

    Ok(md)
}

/// Format a single JSON transcript object.
fn format_single_json_transcript(raw: &str) -> anyhow::Result<String> {
    let v: serde_json::Value = serde_json::from_str(raw)?;
    let mut md = String::new();

    if let Some(task) = v.get("task").and_then(|t| t.as_str()) {
        md.push_str(&format!("## 📋 Tâche\n\n{task}\n\n"));
    }

    if let Some(events) = v.get("events").and_then(|e| e.as_array()) {
        md.push_str("## 📝 Événements\n\n");
        for event in events {
            render_event_to_markdown(&mut md, event);
        }
    }

    if let Some(usage) = v.get("usage") {
        md.push_str("## 📊 Statistiques\n\n");
        if let Some(input) = usage.get("input_tokens").and_then(|t| t.as_u64()) {
            md.push_str(&format!("- **Tokens d'entrée** : {input}\n"));
        }
        if let Some(output) = usage.get("output_tokens").and_then(|t| t.as_u64()) {
            md.push_str(&format!("- **Tokens de sortie** : {output}\n"));
        }
        if let Some(cost) = usage.get("cost_usd").and_then(|c| c.as_f64()) {
            md.push_str(&format!("- **Coût estimé** : ${cost:.4}\n"));
        }
        md.push('\n');
    }

    Ok(md)
}

/// Format a JSON array of event objects.
fn format_json_array_transcript(raw: &str) -> anyhow::Result<String> {
    let events: Vec<serde_json::Value> = serde_json::from_str(raw)?;
    let mut md = String::from("## 📝 Événements\n\n");

    for event in &events {
        render_event_to_markdown(&mut md, event);
    }

    Ok(md)
}

/// Format NDJSON (one JSON object per line).
fn format_ndjson_transcript(raw: &str) -> anyhow::Result<String> {
    let mut md = String::from("## 📝 Événements\n\n");
    let mut task_found = false;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Ok(event) = serde_json::from_str::<serde_json::Value>(trimmed) {
            // Extract task from first relevant event
            if !task_found {
                if let Some(task) = event
                    .get("task")
                    .or_else(|| event.get("description"))
                    .and_then(|t| t.as_str())
                {
                    md.push_str(&format!("## 📋 Tâche\n\n{task}\n\n"));
                    task_found = true;
                }
            }
            render_event_to_markdown(&mut md, &event);
        }
    }

    if md == "## 📝 Événements\n\n" {
        md.push_str("*Aucun événement parsable trouvé.*\n\n");
    }

    Ok(md)
}

/// Render a single event value to markdown.
fn render_event_to_markdown(md: &mut String, event: &serde_json::Value) {
    let event_type = event
        .get("type")
        .or_else(|| event.get("event"))
        .and_then(|t| t.as_str())
        .unwrap_or("event");

    match event_type {
        "RunStarted" | "run_started" => {
            let task = event
                .get("task")
                .and_then(|t| t.as_str())
                .unwrap_or("(tâche)");
            md.push_str(&format!("### 🚀 Démarrage : {task}\n\n"));
        }
        "TextDelta" | "text_delta" | "assistant" => {
            if let Some(text) = event
                .get("text")
                .or_else(|| event.get("content"))
                .and_then(|t| t.as_str())
            {
                if text.len() > 200 {
                    md.push_str(&format!(
                        "<details>\n<summary>💬 {}</summary>\n\n{}\n\n</details>\n\n",
                        &text[..text.len().min(80)],
                        text,
                    ));
                } else {
                    md.push_str(&format!("💬 {text}\n\n"));
                }
            }
        }
        "ToolUseProposed" | "tool_use" => {
            let name = event
                .get("name")
                .or_else(|| event.get("tool"))
                .and_then(|n| n.as_str())
                .unwrap_or("?");
            let input = event
                .get("input")
                .or_else(|| event.get("args"))
                .map(|i| format!("{i}"))
                .unwrap_or_default();
            md.push_str(&format!(
                "<details>\n<summary>🔧 Outil : `{name}`</summary>\n\n```json\n{input}\n```\n\n</details>\n\n"
            ));
        }
        "ToolOutput" | "tool_output" | "tool_result" => {
            let output = event
                .get("output")
                .or_else(|| event.get("content"))
                .or_else(|| event.get("result"))
                .map(|o| format!("{o}"))
                .unwrap_or_default();
            let truncated = if output.len() > 500 {
                format!("{}...", &output[..500])
            } else {
                output
            };
            md.push_str(&format!("✅ Résultat :\n```\n{truncated}\n```\n\n"));
        }
        "Error" | "error" => {
            let msg = event
                .get("message")
                .or_else(|| event.get("error"))
                .and_then(|m| m.as_str())
                .unwrap_or("erreur inconnue");
            md.push_str(&format!("❌ **Erreur** : {msg}\n\n"));
        }
        "RunFinished" | "run_finished" | "Done" => {
            let reason = event
                .get("reason")
                .or_else(|| event.get("stop_reason"))
                .and_then(|r| r.as_str())
                .unwrap_or("terminé");
            md.push_str(&format!("### 🏁 Terminé ({reason})\n\n"));
        }
        _ => {
            // Generic event: show as JSON
            let pretty = serde_json::to_string_pretty(event).unwrap_or_default();
            if pretty.len() < 200 {
                md.push_str(&format!("📌 `{event_type}` :\n```json\n{pretty}\n```\n\n"));
            }
        }
    }
}

// ─── GitHub Gist upload ──────────────────────────────────────────────────────

/// Upload content as a GitHub Gist.
///
/// Tries the `gh` CLI first, then falls back to a direct GitHub API call if
/// `GITHUB_TOKEN` is set.
async fn upload_gist(filename: &str, content: &str) -> anyhow::Result<String> {
    // 1. Try `gh` CLI
    if gh_available() && gh_authenticated() {
        match upload_via_gh_cli(filename, content) {
            Ok(url) => return Ok(url),
            Err(e) => {
                eprintln!("⚠️  Échec gh CLI : {e}");
                eprintln!("   → Tentative via API directe...");
            }
        }
    }

    // 2. Try direct API call with GITHUB_TOKEN
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        if !token.is_empty() {
            return upload_via_api(filename, content, &token).await;
        }
    }

    // 3. No way to upload
    anyhow::bail!(
        "Impossible de créer un Gist.\n\
         → Installe gh CLI : https://cli.github.com\n\
         → Puis : gh auth login\n\
         → Ou définis GITHUB_TOKEN : export GITHUB_TOKEN=\"ghp_...\""
    )
}

fn gh_available() -> bool {
    Command::new("gh")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn gh_authenticated() -> bool {
    Command::new("gh")
        .args(["auth", "status"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn upload_via_gh_cli(filename: &str, content: &str) -> anyhow::Result<String> {
    // Write content to a temp file for gh CLI
    let tmp = std::env::temp_dir().join(format!("sparrow-gist-{}.md", uuid::Uuid::new_v4()));
    std::fs::write(&tmp, content)?;

    let output = Command::new("gh")
        .args([
            "gist",
            "create",
            "--public",
            "--filename",
            filename,
            "--desc",
            &format!(
                "🐦 Sparrow Session — shared via sparrow share (v{})",
                env!("CARGO_PKG_VERSION")
            ),
        ])
        .arg(&tmp)
        .output()?;

    // Clean up temp file
    let _ = std::fs::remove_file(&tmp);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("gh gist create a échoué : {stderr}");
    }

    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if url.is_empty() {
        anyhow::bail!("gh gist create n'a pas retourné d'URL.");
    }

    Ok(url)
}

async fn upload_via_api(filename: &str, content: &str, token: &str) -> anyhow::Result<String> {
    let client = reqwest::Client::builder()
        .user_agent("sparrow-share")
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let body = serde_json::json!({
        "description": format!("🐦 Sparrow Session — shared via sparrow share (v{})", env!("CARGO_PKG_VERSION")),
        "public": true,
        "files": {
            filename: {
                "content": content
            }
        }
    });

    let resp = client
        .post("https://api.github.com/gists")
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    let resp_body = resp.text().await?;

    if !status.is_success() {
        anyhow::bail!(
            "GitHub API a retourné HTTP {} : {}",
            status.as_u16(),
            resp_body,
        );
    }

    let parsed: serde_json::Value = serde_json::from_str(&resp_body)?;
    let html_url = parsed
        .get("html_url")
        .and_then(|u| u.as_str())
        .ok_or_else(|| anyhow::anyhow!("Réponse GitHub inattendue : pas de html_url"))?;

    Ok(html_url.to_string())
}
