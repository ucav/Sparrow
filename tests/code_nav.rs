// Verifies the code-navigation tools: glob (find files) and symbols
// (find definitions / outline), which bring Sparrow's self-coding loop
// closer to a frontier agent.

use sparrow::event::Block;
use sparrow::tools::code_nav::{Glob, Symbols};
use sparrow::tools::{Tool, ToolCtx};

fn temp_ws(name: &str) -> std::path::PathBuf {
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let d = std::env::temp_dir().join(format!("sparrow-{name}-{id}"));
    std::fs::create_dir_all(d.join("src")).unwrap();
    std::fs::write(
        d.join("src").join("lib.rs"),
        "pub fn build_widget() -> u8 { 1 }\nstruct Gadget;\n",
    )
    .unwrap();
    std::fs::write(d.join("README.md"), "hello").unwrap();
    d
}

fn ctx(root: &std::path::Path) -> ToolCtx {
    ToolCtx {
        workspace_root: root.to_path_buf(),
        run_id: sparrow::event::RunId("test".into()),
    }
}

fn text_of(r: &sparrow::tools::ToolResult) -> String {
    r.content
        .iter()
        .filter_map(|b| match b {
            Block::Text(t) => Some(t.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn glob_finds_rust_files() {
    let ws = temp_ws("glob");
    let r = Glob
        .call(serde_json::json!({ "pattern": "**/*.rs" }), &ctx(&ws))
        .await
        .unwrap();
    let out = text_of(&r);
    assert!(out.contains("src/lib.rs"), "glob should find src/lib.rs, got: {out}");
    assert!(!out.contains("README.md"), "glob *.rs must not match README.md");
    let _ = std::fs::remove_dir_all(&ws);
}

#[tokio::test]
async fn symbols_finds_definition() {
    let ws = temp_ws("symbols");
    let r = Symbols
        .call(serde_json::json!({ "name": "build_widget" }), &ctx(&ws))
        .await
        .unwrap();
    let out = text_of(&r);
    assert!(
        out.contains("src/lib.rs") && out.contains("build_widget"),
        "symbols should locate build_widget in src/lib.rs, got: {out}"
    );
    let _ = std::fs::remove_dir_all(&ws);
}
