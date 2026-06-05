use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use sparrow::event::{Block, RunId};
use sparrow::memory::{Memory, SqliteMemory};
use sparrow::tools::knowledge_graph::KnowledgeGraphTool;
use sparrow::tools::{Tool, ToolCtx};
use tokio::net::TcpListener;

fn temp_db(name: &str) -> std::path::PathBuf {
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir()
        .join(format!("sparrow-{name}-{id}"))
        .join("memory.db")
}

fn ctx() -> ToolCtx {
    ToolCtx {
        workspace_root: std::env::current_dir().expect("workspace root"),
        run_id: RunId("knowledge-graph-tool-test".into()),
    }
}

fn json_block(result: &sparrow::tools::ToolResult) -> serde_json::Value {
    assert!(!result.is_error, "tool returned error: {result:?}");
    match result.content.as_slice() {
        [Block::Json(value)] => value.clone(),
        other => panic!("expected one JSON block, got {other:?}"),
    }
}

#[tokio::test]
async fn knowledge_graph_tool_persists_across_sqlite_reopen() {
    let db = temp_db("kg-tool-persist");
    let first: Arc<dyn Memory> = Arc::new(SqliteMemory::open(&db).expect("first memory"));
    let tool = KnowledgeGraphTool::new(first);
    let context = ctx();

    tool.call(
        serde_json::json!({
            "action": "upsert_node",
            "id": "project:sparrow",
            "label": "Sparrow",
            "kind": "project",
            "properties": { "tier": "core" }
        }),
        &context,
    )
    .await
    .expect("upsert project");
    tool.call(
        serde_json::json!({
            "action": "upsert_node",
            "id": "feature:knowledge-graph",
            "label": "Knowledge Graph",
            "kind": "feature",
            "properties": { "persistent": true }
        }),
        &context,
    )
    .await
    .expect("upsert feature");
    tool.call(
        serde_json::json!({
            "action": "upsert_edge",
            "from_id": "project:sparrow",
            "to_id": "feature:knowledge-graph",
            "relation": "ships",
            "weight": 2.0
        }),
        &context,
    )
    .await
    .expect("upsert edge");
    drop(tool);

    let reopened: Arc<dyn Memory> = Arc::new(SqliteMemory::open(&db).expect("reopened memory"));
    let reopened_tool = KnowledgeGraphTool::new(reopened);

    let search = reopened_tool
        .call(
            serde_json::json!({
                "action": "search",
                "query": "knowledge",
                "limit": 10
            }),
            &context,
        )
        .await
        .expect("search");
    let search_json = json_block(&search);
    assert_eq!(
        search_json.as_array().expect("search array")[0]["id"],
        "feature:knowledge-graph"
    );

    let neighbors = reopened_tool
        .call(
            serde_json::json!({
                "action": "neighbors",
                "id": "project:sparrow",
                "direction": "outgoing",
                "limit": 10
            }),
            &context,
        )
        .await
        .expect("neighbors");
    let neighbors_json = json_block(&neighbors);
    assert_eq!(neighbors_json.as_array().expect("neighbors array").len(), 1);
    assert_eq!(neighbors_json[0][0]["relation"], "ships");
    assert_eq!(neighbors_json[0][1]["id"], "feature:knowledge-graph");

    let root = db.parent().expect("db parent").to_path_buf();
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn knowledge_graph_tool_syncs_to_local_neo4j_http_endpoint() {
    let db = temp_db("kg-tool-neo4j");
    let memory: Arc<dyn Memory> = Arc::new(SqliteMemory::open(&db).expect("memory"));
    let tool = KnowledgeGraphTool::new(memory);
    let context = ctx();
    tool.call(
        serde_json::json!({
            "action": "upsert_node",
            "id": "project:sparrow",
            "label": "Sparrow",
            "kind": "project"
        }),
        &context,
    )
    .await
    .expect("upsert project");
    tool.call(
        serde_json::json!({
            "action": "upsert_node",
            "id": "feature:neo4j",
            "label": "Neo4j Sync",
            "kind": "feature"
        }),
        &context,
    )
    .await
    .expect("upsert feature");
    tool.call(
        serde_json::json!({
            "action": "upsert_edge",
            "id": "project:sparrow:syncs:feature:neo4j",
            "from_id": "project:sparrow",
            "to_id": "feature:neo4j",
            "relation": "syncs"
        }),
        &context,
    )
    .await
    .expect("upsert edge");

    let captured = Arc::new(Mutex::new(None::<serde_json::Value>));
    let app = Router::new()
        .route(
            "/db/neo4j/tx/commit",
            post(
                |State(captured): State<Arc<Mutex<Option<serde_json::Value>>>>,
                 Json(body): Json<serde_json::Value>| async move {
                    *captured.lock().expect("capture lock") = Some(body);
                    Json(serde_json::json!({ "results": [], "errors": [] }))
                },
            ),
        )
        .with_state(captured.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });

    let previous_url = std::env::var("NEO4J_URL").ok();
    let previous_user = std::env::var("NEO4J_USER").ok();
    let previous_password = std::env::var("NEO4J_PASSWORD").ok();
    unsafe {
        std::env::set_var("NEO4J_URL", format!("http://{addr}/db/neo4j/tx/commit"));
        std::env::set_var("NEO4J_USER", "neo4j");
        std::env::set_var("NEO4J_PASSWORD", "test-password");
    }
    let result = tool
        .call(serde_json::json!({ "action": "sync_neo4j" }), &context)
        .await
        .expect("sync neo4j");
    restore_env("NEO4J_URL", previous_url);
    restore_env("NEO4J_USER", previous_user);
    restore_env("NEO4J_PASSWORD", previous_password);
    server.abort();

    assert!(!result.is_error, "sync should succeed: {result:?}");
    assert!(
        matches!(result.content.as_slice(), [Block::Text(text)] if text.contains("3 statements")),
        "sync result should report statements: {result:?}"
    );

    let body = captured
        .lock()
        .expect("capture lock")
        .clone()
        .expect("Neo4j request body");
    let statements = body["statements"].as_array().expect("statements array");
    assert_eq!(statements.len(), 3);
    assert!(
        statements
            .iter()
            .all(|statement| statement["parameters"].is_object()),
        "Neo4j sync must use parameterized statements"
    );
    assert_eq!(statements[0]["parameters"]["id"], "feature:neo4j");

    let root = db.parent().expect("db parent").to_path_buf();
    let _ = std::fs::remove_dir_all(root);
}

fn restore_env(key: &str, value: Option<String>) {
    match value {
        Some(value) => unsafe { std::env::set_var(key, value) },
        None => unsafe { std::env::remove_var(key) },
    }
}
