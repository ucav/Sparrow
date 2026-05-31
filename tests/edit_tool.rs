use serde_json::json;
use sparrow::event::{Block, RunId};
use sparrow::tools::edit::Edit;
use sparrow::tools::{Tool, ToolCtx};

fn temp_workspace(name: &str) -> std::path::PathBuf {
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("sparrow-{name}-{id}"))
}

#[tokio::test]
async fn edit_tool_replaces_exact_match_and_returns_diff_block() {
    let root = temp_workspace("edit-tool");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("sample.txt"), "alpha\nbeta\n").unwrap();

    let result = Edit
        .call(
            json!({
                "path": "sample.txt",
                "old": "beta",
                "new": "gamma"
            }),
            &ToolCtx {
                workspace_root: root.clone(),
                run_id: RunId::new(),
            },
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert_eq!(
        std::fs::read_to_string(root.join("sample.txt")).unwrap(),
        "alpha\ngamma\n"
    );
    assert!(result.content.iter().any(|block| matches!(
        block,
        Block::Diff { file, patch } if file == "sample.txt" && patch.contains("-beta") && patch.contains("+gamma")
    )));

    let _ = std::fs::remove_dir_all(root);
}
