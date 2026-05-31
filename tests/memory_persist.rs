use sparrow::memory::{Fact, Memory, SqliteMemory};

fn temp_db(name: &str) -> std::path::PathBuf {
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir()
        .join(format!("sparrow-{name}-{id}"))
        .join("memory.db")
}

#[test]
fn sqlite_memory_persists_facts_after_reopen() {
    let db = temp_db("memory-persist");
    let first = SqliteMemory::open(&db).unwrap();
    first
        .remember(Fact {
            id: "fact-routing".into(),
            key: "routing.policy".into(),
            value: "small tasks prefer ollama".into(),
            created_at: "2026-05-31 00:00:00".into(),
            updated_at: "2026-05-31 00:00:00".into(),
        })
        .unwrap();
    drop(first);

    let reopened = SqliteMemory::open(&db).unwrap();
    let facts = reopened.recall("ollama", 5);

    assert!(
        facts
            .iter()
            .any(|fact| { fact.id == "fact-routing" && fact.value == "small tasks prefer ollama" })
    );

    let root = db.parent().unwrap().to_path_buf();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn sqlite_memory_caches_discovered_models_for_24h() {
    let db = temp_db("model-discovery-cache");
    let memory = SqliteMemory::open(&db).unwrap();
    memory
        .cache_discovered_models(
            "anthropic",
            &[
                "claude-sonnet-4-6".to_string(),
                "claude-opus-4-1".to_string(),
            ],
        )
        .unwrap();

    let models = memory.get_discovered_models("anthropic");
    assert!(models.contains(&"claude-sonnet-4-6".to_string()));
    assert!(models.contains(&"claude-opus-4-1".to_string()));

    memory
        .cache_discovered_models("anthropic", &["claude-haiku-4-5".to_string()])
        .unwrap();
    let refreshed = memory.get_discovered_models("anthropic");
    assert_eq!(refreshed, vec!["claude-haiku-4-5".to_string()]);

    let root = db.parent().unwrap().to_path_buf();
    let _ = std::fs::remove_dir_all(root);
}
