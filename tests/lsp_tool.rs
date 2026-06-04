use sparrow::event::{Block, RunId};
use sparrow::tools::builder_tools::LspClient;
use sparrow::tools::{Tool, ToolCtx, known_tool_metadata};

fn ctx(root: &std::path::Path) -> ToolCtx {
    ToolCtx {
        workspace_root: root.to_path_buf(),
        run_id: RunId("lsp-test".into()),
    }
}

fn text(result: sparrow::tools::ToolResult) -> String {
    result
        .content
        .into_iter()
        .map(|block| match block {
            Block::Text(text) => text,
            other => format!("{:?}", other),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn lsp_goto_definition_and_references_are_real() {
    let tmp = tempfile::tempdir().expect("tmp");
    let src = tmp.path().join("src");
    std::fs::create_dir_all(&src).expect("src");
    std::fs::write(
        src.join("lib.rs"),
        "pub fn alpha() -> i32 { 7 }\n\npub fn beta() -> i32 { alpha() }\n",
    )
    .expect("write");

    let tool = LspClient;
    let def = tool
        .call(
            serde_json::json!({
                "action": "goto_definition",
                "file": "src/lib.rs",
                "symbol": "alpha"
            }),
            &ctx(tmp.path()),
        )
        .await
        .expect("definition");
    let def = text(def);
    assert!(def.contains("src/lib.rs:1: pub fn alpha()"));

    let refs = tool
        .call(
            serde_json::json!({
                "action": "find_references",
                "file": "src/lib.rs",
                "symbol": "alpha"
            }),
            &ctx(tmp.path()),
        )
        .await
        .expect("references");
    let refs = text(refs);
    assert!(refs.contains("src/lib.rs:1: pub fn alpha()"));
    assert!(refs.contains("src/lib.rs:3: pub fn beta()"));
}

#[tokio::test]
async fn lsp_hover_returns_context_and_symbol_definition() {
    let tmp = tempfile::tempdir().expect("tmp");
    std::fs::write(
        tmp.path().join("main.py"),
        "def greet(name):\n    return f'hi {name}'\n\nprint(greet('sparrow'))\n",
    )
    .expect("write");

    let out = LspClient
        .call(
            serde_json::json!({
                "action": "hover",
                "file": "main.py",
                "line": 4,
                "column": 8
            }),
            &ctx(tmp.path()),
        )
        .await
        .expect("hover");
    let out = text(out);
    assert!(out.contains("hover main.py:4:8"));
    assert!(out.contains("symbol: greet"));
    assert!(out.contains("main.py:1: def greet(name):"));
}

#[test]
fn lsp_is_advertised_as_debug_readonly_tool() {
    let tools = known_tool_metadata(None);
    let lsp = tools
        .into_iter()
        .find(|tool| tool.name == "lsp")
        .expect("lsp metadata");
    assert_eq!(lsp.toolset, "debug");
    assert!(!lsp.exec);
    assert!(!lsp.mutates_files);
}
