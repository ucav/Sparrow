// Verifies cross-surface session persistence: messages saved under a session
// key are reloaded as conversation context (§8).

use sparrow::provider::{ContentBlock, Msg};
use sparrow::runtime::session::SessionStore;

fn temp_db(name: &str) -> std::path::PathBuf {
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("sparrow-{name}-{id}.db"))
}

#[test]
fn session_round_trips_messages_across_reopen() {
    let path = temp_db("session");
    let key = "user:42";

    // First "surface" writes a turn.
    {
        let store = SessionStore::open(&path).unwrap();
        let msgs = vec![
            Msg {
                role: "user".into(),
                content: vec![ContentBlock::Text {
                    text: "remember my name is Abdou".into(),
                }],
            },
            Msg {
                role: "assistant".into(),
                content: vec![ContentBlock::Text {
                    text: "Noted.".into(),
                }],
            },
        ];
        store.save(key, &msgs, None).unwrap();
    }

    // A different "surface" (new store handle, same DB) resumes the session.
    {
        let store = SessionStore::open(&path).unwrap();
        let sess = store.load(key).expect("session should persist");
        let msgs: Vec<Msg> = serde_json::from_str(&sess.messages_json).unwrap();
        assert_eq!(msgs.len(), 2);
        match &msgs[0].content[0] {
            ContentBlock::Text { text } => assert!(text.contains("Abdou")),
            _ => panic!("expected text block"),
        }

        // Clearing the session removes continuity.
        store.delete(key).unwrap();
        assert!(store.load(key).is_none());
    }

    let _ = std::fs::remove_file(&path);
}
