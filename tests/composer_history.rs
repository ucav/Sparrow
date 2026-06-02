use sparrow::provider::{ContentBlock, Msg};
use sparrow::runtime::session::SessionStore;

fn user(text: &str) -> Msg {
    Msg {
        role: "user".into(),
        content: vec![ContentBlock::Text { text: text.into() }],
    }
}

fn assistant(text: &str) -> Msg {
    Msg {
        role: "assistant".into(),
        content: vec![ContentBlock::Text { text: text.into() }],
    }
}

#[test]
fn recent_inputs_returns_latest_user_messages_only() {
    let tmp = tempfile::tempdir().expect("tmp");
    let store = SessionStore::open(&tmp.path().join("sessions.db")).expect("session store");

    store
        .save(
            "first",
            &[
                user("fix lint errors"),
                assistant("done"),
                user("write tests"),
            ],
            Some("first"),
        )
        .expect("save first");
    store
        .save(
            "second",
            &[
                user("analyse repo"),
                assistant("summary"),
                user("fix lint errors"),
            ],
            Some("second"),
        )
        .expect("save second");

    let inputs = store.recent_inputs(10);
    assert!(inputs.contains(&"analyse repo".to_string()));
    assert!(inputs.contains(&"write tests".to_string()));
    assert!(inputs.contains(&"fix lint errors".to_string()));
    assert!(!inputs.contains(&"done".to_string()));
    assert!(!inputs.contains(&"summary".to_string()));
    assert_eq!(
        inputs
            .iter()
            .filter(|item| item.as_str() == "fix lint errors")
            .count(),
        1,
        "history should de-duplicate repeated user prompts"
    );
}
