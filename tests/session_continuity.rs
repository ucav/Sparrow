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
                    text: "remember my name is Alice".into(),
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
            ContentBlock::Text { text } => assert!(text.contains("Alice")),
            _ => panic!("expected text block"),
        }

        // Clearing the session removes continuity.
        store.delete(key).unwrap();
        assert!(store.load(key).is_none());
    }

    let _ = std::fs::remove_file(&path);
}

#[test]
fn session_round_trips_reasoning_blocks_for_provider_continuity() {
    let path = temp_db("session-reasoning");
    let key = "user:reasoning";

    let store = SessionStore::open(&path).unwrap();
    let msgs = vec![Msg {
        role: "assistant".into(),
        content: vec![
            ContentBlock::Reasoning {
                text: "opaque provider state".into(),
            },
            ContentBlock::Text {
                text: "visible answer".into(),
            },
        ],
    }];
    store.save(key, &msgs, None).unwrap();

    let sess = store.load(key).expect("session should persist");
    let loaded: Vec<Msg> = serde_json::from_str(&sess.messages_json).unwrap();
    assert!(matches!(
        &loaded[0].content[0],
        ContentBlock::Reasoning { text } if text == "opaque provider state"
    ));
    assert!(matches!(
        &loaded[0].content[1],
        ContentBlock::Text { text } if text == "visible answer"
    ));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn session_search_and_scroll_find_old_turns() {
    let path = temp_db("session-search");
    let key = "user:search";
    let store = SessionStore::open(&path).unwrap();
    let msgs = vec![
        Msg {
            role: "user".into(),
            content: vec![ContentBlock::Text {
                text: "first turn about routing".into(),
            }],
        },
        Msg {
            role: "assistant".into(),
            content: vec![ContentBlock::Text {
                text: "routing is noted".into(),
            }],
        },
        Msg {
            role: "user".into(),
            content: vec![ContentBlock::Text {
                text: "later mention phoenix-context-window".into(),
            }],
        },
        Msg {
            role: "assistant".into(),
            content: vec![ContentBlock::Text {
                text: "found the old context marker".into(),
            }],
        },
    ];
    store.save(key, &msgs, Some("search test")).unwrap();

    let hits = store.search("phoenix context", 5);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].session_id, key);
    assert_eq!(hits[0].turn_index, 2);

    let slice = store.scroll(key, hits[0].turn_index, 1, 1).unwrap();
    assert_eq!(slice.start, 1);
    assert_eq!(slice.messages.len(), 3);

    let _ = std::fs::remove_file(&path);
}
