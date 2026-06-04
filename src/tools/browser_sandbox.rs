use async_trait::async_trait;
use base64::Engine;
use serde_json::json;
use std::io::Write;
use std::process::Stdio;

use crate::event::{Block, RiskLevel};
use crate::tools::{Tool, ToolCtx, ToolResult};

const PLAYWRIGHT_DRIVER: &str = include_str!("../../scripts/playwright-driver.mjs");

/// Browser automation through a real Playwright Chromium runtime.
///
/// The driver is embedded into the binary and materialized into a temporary
/// `.mjs` file per call, so installed Sparrow binaries do not depend on a repo
/// checkout. The host must provide Node.js plus the `playwright` package and a
/// Chromium browser (`npm install && npx playwright install chromium`).
pub struct BrowserTool;

#[async_trait]
impl Tool for BrowserTool {
    fn name(&self) -> &str {
        "browser"
    }

    fn description(&self) -> &str {
        "Control a Playwright headless browser: navigate, screenshot, click, type, extract text, or evaluate JavaScript"
    }

    fn schema(&self) -> serde_json::Value {
        browser_schema(&[
            "navigate",
            "screenshot",
            "get_text",
            "extract",
            "click",
            "type",
            "evaluate",
        ])
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::Network
    }

    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        run_playwright(args, ctx, false).await
    }
}

/// Computer-use primitive focused on screenshot/click/type.
///
/// This is intentionally separate from `browser`: it is classified as Exec so
/// supervised/autonomous policy gates can treat UI-driving actions as stronger
/// than passive web reads. On Linux hardened mode, the launched Node process is
/// wrapped with `bwrap` when available.
pub struct ComputerTool;

#[async_trait]
impl Tool for ComputerTool {
    fn name(&self) -> &str {
        "computer"
    }

    fn description(&self) -> &str {
        "Computer-use actions through Playwright Chromium: screenshot, click, and type, gated as sandboxed exec"
    }

    fn schema(&self) -> serde_json::Value {
        browser_schema(&["screenshot", "click", "type"])
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::Exec
    }

    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        run_playwright(args, ctx, true).await
    }
}

fn browser_schema(actions: &[&str]) -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "action": { "type": "string", "enum": actions },
            "url": { "type": "string", "description": "URL to open before the action; defaults to about:blank" },
            "selector": { "type": "string", "description": "CSS selector for click/type/extract or element screenshot" },
            "text": { "type": "string", "description": "Text for type/fill actions" },
            "js": { "type": "string", "description": "JavaScript expression/function body for evaluate" },
            "timeout_ms": { "type": "integer", "description": "Timeout in milliseconds, default 30000" },
            "full_page": { "type": "boolean", "description": "For screenshots, capture the full page unless false" },
            "viewport": {
                "type": "object",
                "properties": {
                    "width": { "type": "integer" },
                    "height": { "type": "integer" },
                    "deviceScaleFactor": { "type": "number" }
                }
            }
        },
        "required": ["action"]
    })
}

async fn run_playwright(
    mut args: serde_json::Value,
    ctx: &ToolCtx,
    require_computer_action: bool,
) -> anyhow::Result<ToolResult> {
    let action = args["action"].as_str().unwrap_or("navigate").to_string();
    if require_computer_action && !matches!(action.as_str(), "screenshot" | "click" | "type") {
        return Ok(ToolResult::error(format!(
            "computer only supports screenshot, click, and type (got {})",
            action
        )));
    }

    if args.get("url").and_then(|v| v.as_str()).is_none() {
        args["url"] = json!("about:blank");
    }

    let driver_path = materialize_driver()?;
    let workspace_root = ctx.workspace_root.clone();
    let output = tokio::task::spawn_blocking(move || {
        invoke_node_driver(&driver_path, &args, &workspace_root)
    })
    .await
    .map_err(|e| anyhow::anyhow!("browser task join error: {}", e))??;

    parse_driver_output(&action, &output)
}

fn materialize_driver() -> anyhow::Result<std::path::PathBuf> {
    let dir = std::env::temp_dir().join("sparrow-playwright");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("driver-{}.mjs", env!("CARGO_PKG_VERSION")));
    if !path.exists() || std::fs::read_to_string(&path).ok().as_deref() != Some(PLAYWRIGHT_DRIVER) {
        std::fs::write(&path, PLAYWRIGHT_DRIVER)?;
    }
    Ok(path)
}

fn invoke_node_driver(
    driver_path: &std::path::Path,
    args: &serde_json::Value,
    workspace_root: &std::path::Path,
) -> anyhow::Result<String> {
    let mut child = build_command(driver_path, workspace_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            anyhow::anyhow!(
                "failed to launch Playwright driver via Node.js: {}. Install Node.js, then run `npm install` and `npx playwright install chromium`.",
                e
            )
        })?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(serde_json::to_string(args)?.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    if !output.status.success() {
        anyhow::bail!(
            "Playwright driver exited {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let stdout = String::from_utf8(output.stdout)?;
    if stdout.trim().is_empty() {
        anyhow::bail!(
            "Playwright driver returned no output: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(stdout)
}

fn build_command(
    driver_path: &std::path::Path,
    workspace_root: &std::path::Path,
) -> std::process::Command {
    #[cfg(target_os = "linux")]
    {
        if std::env::var("SPARROW_BROWSER_BWRAP").ok().as_deref() != Some("0") && which("bwrap") {
            let mut cmd = std::process::Command::new("bwrap");
            cmd.arg("--die-with-parent")
                .arg("--unshare-pid")
                .arg("--proc")
                .arg("/proc")
                .arg("--dev")
                .arg("/dev")
                .arg("--tmpfs")
                .arg("/tmp");
            for path in ["/usr", "/bin", "/lib", "/lib64", "/etc"] {
                add_ro_bind_if_exists(&mut cmd, path);
            }
            if let Some(home) = std::env::var_os("HOME") {
                let cache = std::path::PathBuf::from(home)
                    .join(".cache")
                    .join("ms-playwright");
                add_bind_if_exists(&mut cmd, &cache);
            }
            add_bind_if_exists(&mut cmd, &std::env::temp_dir());
            cmd.arg("--bind")
                .arg(workspace_root)
                .arg(workspace_root)
                .arg("--chdir")
                .arg(workspace_root)
                .arg("node")
                .arg(driver_path);
            return cmd;
        }
    }

    let mut cmd = std::process::Command::new("node");
    cmd.arg(driver_path).current_dir(workspace_root);
    cmd
}

fn parse_driver_output(action: &str, stdout: &str) -> anyhow::Result<ToolResult> {
    let value: serde_json::Value = serde_json::from_str(stdout.trim())?;
    if value.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        let error = value
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("Playwright driver failed");
        let detail = value.get("detail").and_then(|v| v.as_str()).unwrap_or("");
        return Ok(ToolResult::error(
            format!("{}{}", error, if detail.is_empty() { "" } else { ": " }) + detail,
        ));
    }

    if let Some(image) = value.get("image_base64").and_then(|v| v.as_str()) {
        let data = base64::engine::general_purpose::STANDARD.decode(image)?;
        return Ok(ToolResult::ok(vec![Block::Image {
            data,
            mime: value
                .get("mime")
                .and_then(|v| v.as_str())
                .unwrap_or("image/png")
                .to_string(),
        }]));
    }

    if let Some(text) = value.get("text").and_then(|v| v.as_str()) {
        return Ok(ToolResult::text(text.to_string()));
    }

    if let Some(result) = value.get("result") {
        return Ok(ToolResult::text(result.to_string()));
    }

    let title = value.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let url = value.get("url").and_then(|v| v.as_str()).unwrap_or("");
    Ok(ToolResult::text(format!(
        "{} ok{}{}",
        action,
        if url.is_empty() { "" } else { " · " },
        if title.is_empty() { url } else { title }
    )))
}

#[cfg(target_os = "linux")]
fn which(cmd: &str) -> bool {
    std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {}", cmd))
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn add_ro_bind_if_exists(cmd: &mut std::process::Command, path: &str) {
    let path = std::path::Path::new(path);
    if path.exists() {
        cmd.arg("--ro-bind").arg(path).arg(path);
    }
}

#[cfg(target_os = "linux")]
fn add_bind_if_exists(cmd: &mut std::process::Command, path: &std::path::Path) {
    if path.exists() {
        cmd.arg("--bind").arg(path).arg(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_screenshot_payload_as_image_block() {
        let out = json!({
            "ok": true,
            "mime": "image/png",
            "image_base64": base64::engine::general_purpose::STANDARD.encode([1_u8, 2, 3]),
        });
        let result = parse_driver_output("screenshot", &out.to_string()).unwrap();
        assert!(!result.is_error);
        assert!(matches!(
            result.content.as_slice(),
            [Block::Image { mime, data }] if mime == "image/png" && data == &vec![1, 2, 3]
        ));
    }

    #[test]
    fn computer_rejects_non_computer_actions_before_driver_launch() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            ComputerTool
                .call(
                    json!({"action": "navigate", "url": "https://example.com"}),
                    &ToolCtx {
                        workspace_root: std::env::current_dir().unwrap(),
                        run_id: crate::event::RunId("test".into()),
                    },
                )
                .await
                .unwrap()
        });
        assert!(result.is_error);
    }
}
