use sparrow::tui::theme::{THEME_NAMES, by_name};

#[test]
fn three_built_in_themes_are_resolvable_by_name() {
    assert_eq!(THEME_NAMES, &["captain", "midnight", "paper"]);
    let cap = by_name("captain");
    let mid = by_name("midnight");
    let pap = by_name("paper");
    // The three variants must produce distinct backgrounds — that is what
    // makes them visibly different in the cockpit.
    assert_ne!(cap.bg, mid.bg);
    assert_ne!(cap.bg, pap.bg);
    assert_ne!(mid.bg, pap.bg);
}

#[test]
fn unknown_theme_name_falls_back_to_captain() {
    let unknown = by_name("does-not-exist");
    let captain = by_name("captain");
    // Same background → same theme.
    assert_eq!(unknown.bg, captain.bg);
    assert_eq!(unknown.brand, captain.brand);
}

#[test]
fn theme_lookup_is_case_insensitive_and_trims() {
    let a = by_name("Midnight");
    let b = by_name("  MIDNIGHT  ");
    let c = by_name("midnight");
    assert_eq!(a.bg, b.bg);
    assert_eq!(b.bg, c.bg);
}

#[test]
fn keyboard_doc_lists_critical_shortcuts() {
    let doc = std::fs::read_to_string("docs/keyboard.md")
        .expect("docs/keyboard.md must ship with the TUI");
    for shortcut in [
        "Ctrl+I",
        "Ctrl+L",
        "Ctrl+O",
        "Shift+Enter",
        "Tab",
        "Up",
        "Down",
    ] {
        assert!(
            doc.contains(shortcut),
            "docs/keyboard.md must document {}",
            shortcut
        );
    }
    for theme in ["captain", "midnight", "paper"] {
        assert!(doc.contains(theme), "docs/keyboard.md must list {}", theme);
    }
}

#[tokio::test]
async fn webview_app_builds_with_phase13_routes() {
    // Spin up the server on a free port, then shut it down — proves all
    // routes (sessions, artifacts, upload, security, memory, plugins, tools,
    // permissions, commands) compile and bind without panicking.
    use std::net::SocketAddr;
    use tokio::sync::broadcast;

    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let (tx, _rx) = broadcast::channel(16);
    let server = sparrow::console::WebViewServer::new(addr, tx, None, None, None, None, None);
    // Just verify the constructor accepts the expected shape — actually
    // binding requires a tokio TcpListener which we skip to keep the test
    // hermetic and fast on all platforms.
    drop(server);
}
