// Verifies the self-improvement memory loop (§3.8): after a run touches files,
// the Distiller extracts durable user facts (language/framework) into memory.

use sparrow::event::{Event, RiskLevel, RunId};
use sparrow::extras::Distiller;
use sparrow::memory::{Memory, SqliteMemory};
use std::sync::Arc;

fn temp_db(name: &str) -> std::path::PathBuf {
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("sparrow-{name}-{id}.db"))
}

#[tokio::test]
async fn distiller_extracts_language_facts_from_tool_events() {
    let path = temp_db("distiller");
    let memory: Arc<dyn Memory> = Arc::new(SqliteMemory::open(&path).unwrap());

    // Simulate a run that edited Rust files via tools.
    let run = RunId("test-run".into());
    let events = vec![
        Event::ToolUseProposed {
            run: run.clone(),
            id: String::new(),
            name: "fs_write".into(),
            args: serde_json::json!({ "path": "src/main.rs", "content": "fn main() {}" }),
            risk: RiskLevel::Mutating,
        },
        Event::ToolUseProposed {
            run: run.clone(),
            id: String::new(),
            name: "edit".into(),
            args: serde_json::json!({ "path": "src/lib.rs" }),
            risk: RiskLevel::Mutating,
        },
        Event::ThinkingDelta {
            run: run.clone(),
            text: "I'll add a test for this with TDD.".into(),
        },
    ];

    Distiller::distill(&memory, &events, "add a rust function").await;

    let facts = memory.all_facts();
    assert!(
        facts
            .iter()
            .any(|f| f.key == "user:language" && f.value == "Rust"),
        "expected a 'user:language = Rust' fact, got: {:?}",
        facts.iter().map(|f| (&f.key, &f.value)).collect::<Vec<_>>()
    );
    assert!(
        facts.iter().any(|f| f.key == "user:style"),
        "expected a style fact from the TDD hint"
    );

    let _ = std::fs::remove_file(&path);
}
