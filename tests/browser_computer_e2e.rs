use sparrow::event::{Block, RunId};
use sparrow::tools::browser_sandbox::{BrowserTool, ComputerTool};
use sparrow::tools::{Tool, ToolCtx, ToolResult};

fn ctx() -> ToolCtx {
    ToolCtx {
        workspace_root: std::env::current_dir().expect("workspace root"),
        run_id: RunId("browser-computer-e2e".into()),
    }
}

fn runtime_is_required() -> bool {
    std::env::var("SPARROW_REQUIRE_PLAYWRIGHT_E2E")
        .ok()
        .as_deref()
        == Some("1")
}

fn data_url(html: &str) -> String {
    let mut encoded = String::with_capacity(html.len());
    for byte in html.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push_str("%20"),
            b'\n' => encoded.push_str("%0A"),
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    format!("data:text/html,{encoded}")
}

fn skip_or_panic(result: &ToolResult, action: &str) -> bool {
    if !result.is_error {
        return false;
    }
    let message = result
        .content
        .iter()
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    let missing_runtime = message.contains("Playwright")
        || message.contains("Chromium")
        || message.contains("Node.js")
        || message.contains("playwright");
    if missing_runtime && !runtime_is_required() {
        eprintln!("skipping Playwright E2E `{action}` because runtime is unavailable: {message}");
        return true;
    }
    panic!("Playwright E2E `{action}` failed: {message}");
}

#[tokio::test]
async fn browser_screenshot_returns_real_png_block() {
    let result = BrowserTool
        .call(
            serde_json::json!({
                "action": "screenshot",
                "url": "about:blank",
                "viewport": { "width": 320, "height": 200 },
                "full_page": false
            }),
            &ctx(),
        )
        .await
        .expect("browser tool call");
    if skip_or_panic(&result, "browser.screenshot") {
        return;
    }

    assert!(!result.is_error);
    match result.content.as_slice() {
        [Block::Image { mime, data }] => {
            assert_eq!(mime, "image/png");
            assert!(
                data.starts_with(b"\x89PNG\r\n\x1a\n"),
                "screenshot must be PNG bytes"
            );
            assert!(data.len() > 500, "screenshot should not be an empty image");
        }
        other => panic!("expected one image block, got {other:?}"),
    }
}

#[tokio::test]
async fn computer_can_type_and_click_in_headless_page() {
    let url = data_url(
        r#"<!doctype html>
<html>
  <body>
    <input id="q" />
    <button id="go" onclick="document.body.dataset.clicked='yes'">Go</button>
  </body>
</html>"#,
    );
    let context = ctx();

    let typed = ComputerTool
        .call(
            serde_json::json!({
                "action": "type",
                "url": url,
                "selector": "#q",
                "text": "sparrow"
            }),
            &context,
        )
        .await
        .expect("computer type call");
    if skip_or_panic(&typed, "computer.type") {
        return;
    }
    assert!(!typed.is_error);
    assert!(
        matches!(typed.content.as_slice(), [Block::Text(text)] if text.contains("typed 7 chars into #q"))
    );

    let clicked = ComputerTool
        .call(
            serde_json::json!({
                "action": "click",
                "url": url,
                "selector": "#go"
            }),
            &context,
        )
        .await
        .expect("computer click call");
    if skip_or_panic(&clicked, "computer.click") {
        return;
    }
    assert!(!clicked.is_error);
    assert!(
        matches!(clicked.content.as_slice(), [Block::Text(text)] if text.contains("clicked #go"))
    );
}
