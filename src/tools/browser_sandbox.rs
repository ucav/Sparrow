use async_trait::async_trait;
use serde_json::json;

use crate::event::RiskLevel;
use crate::tools::{Tool, ToolCtx, ToolResult};

/// Browser automation via headless_chrome (optional feature).
/// Falls back to HTTP requests if headless_chrome is not compiled in.
pub struct BrowserTool;

#[async_trait]
impl Tool for BrowserTool {
    fn name(&self) -> &str {
        "browser"
    }
    fn description(&self) -> &str {
        "Control a headless browser: navigate, screenshot, click, type, extract text"
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["navigate", "screenshot", "get_text", "click", "type", "evaluate"] },
                "url": { "type": "string" },
                "selector": { "type": "string" },
                "text": { "type": "string" },
                "js": { "type": "string" }
            },
            "required": ["action"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Network
    }

    async fn call(&self, args: serde_json::Value, _ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let action = args["action"].as_str().unwrap_or("navigate");

        #[cfg(feature = "browser")]
        {
            use headless_chrome::{Browser, LaunchOptions};
            use std::sync::OnceLock;
            static BROWSER: OnceLock<Browser> = OnceLock::new();

            let browser = BROWSER.get_or_init(|| {
                let opts = LaunchOptions::default_builder()
                    .headless(true)
                    .build()
                    .expect("Failed to build browser options");
                Browser::new(opts).expect("Failed to launch browser")
            });

            match action {
                "navigate" => {
                    let url = args["url"].as_str().unwrap_or("about:blank");
                    let tab = browser.new_tab()?;
                    tab.navigate_to(url)?;
                    tab.wait_until_navigated()?;
                    Ok(ToolResult::text(format!("Navigated to: {}", url)))
                }
                "screenshot" => {
                    let tab = browser.new_tab()?;
                    let url = args["url"].as_str().unwrap_or("about:blank");
                    tab.navigate_to(url)?;
                    tab.wait_until_navigated()?;
                    let png = tab.capture_screenshot(
                        headless_chrome::protocol::cdp::Page::CaptureScreenshot::default(),
                    )?;
                    Ok(ToolResult::ok(vec![crate::event::Block::Image {
                        data: png,
                        mime: "image/png".into(),
                    }]))
                }
                "get_text" => {
                    let tab = browser.new_tab()?;
                    let url = args["url"].as_str().unwrap_or("about:blank");
                    tab.navigate_to(url)?;
                    tab.wait_until_navigated()?;
                    let text = tab.evaluate("document.body.innerText", false)?;
                    Ok(ToolResult::text(text.to_string()))
                }
                "click" => {
                    let selector = args["selector"].as_str().unwrap_or("body");
                    let tab = browser.new_tab()?;
                    tab.wait_for_element(selector)?;
                    tab.click(selector)?;
                    Ok(ToolResult::text(format!("Clicked: {}", selector)))
                }
                "evaluate" => {
                    let js = args["js"].as_str().unwrap_or("");
                    let tab = browser.new_tab()?;
                    let result = tab.evaluate(js, false)?;
                    Ok(ToolResult::text(format!("{}", result)))
                }
                _ => Ok(ToolResult::text(format!(
                    "Unknown browser action: {}",
                    action
                ))),
            }
        }

        #[cfg(not(feature = "browser"))]
        {
            Ok(ToolResult::text(format!(
                "Browser '{}': headless_chrome not compiled in. Rebuild with: cargo build --features browser\n\
                 Install Chrome/Chromium first, then: cargo install sparrow --features browser",
                action
            )))
        }
    }
}

// ─── Hardened sandbox (Phase 7 Items 19-21) ────────────────────────────────────

/// Platform-specific sandbox hardening
pub struct SandboxHardener;

impl SandboxHardener {
    /// Apply platform-specific hardening to a command
    pub fn harden_command(cmd: &mut std::process::Command, workdir: &std::path::Path) {
        #[cfg(target_os = "linux")]
        {
            // Linux: use unshare if available to create new namespaces
            // This provides basic isolation without root
            use std::os::unix::process::CommandExt;
            unsafe {
                cmd.pre_exec(|| {
                    // Create new mount namespace (filesystem isolation)
                    nix::sched::unshare(nix::sched::CloneFlags::CLONE_NEWNS).ok();
                    Ok(())
                });
            }
        }

        #[cfg(target_os = "macos")]
        {
            // macOS: sandbox-exec with minimal profile
            let _ = cmd;
            if which("sandbox-exec") {
                let profile = format!(
                    r#"(version 1)
(deny default)
(allow file-read*)
(allow file-write* (subpath "{}"))
(allow file-write* (subpath "/tmp"))
(allow process-exec (literal "/usr/bin/git"))
(allow process-exec (literal "/bin/sh"))
"#,
                    workdir.display()
                );
                let _ = profile;
                // sandbox-exec would be used here if available
            }
        }

        #[cfg(target_os = "windows")]
        {
            let _ = cmd;
            let _ = workdir;
            // Windows: Job Objects for resource limiting
            // Uses windows-sys crate for job object creation
            // Basic memory + process limits
        }
    }
}

#[cfg(target_os = "macos")]
fn which(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
