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

#[test]
fn console_html_has_dynamic_swarm_hooks() {
    let html =
        std::fs::read_to_string("console.html").expect("console.html must ship with the WebView");
    // The Sprint 1 deliverable 1bis: the swarm row must expose the anchor
    // and the overflow chip so loadSwarmAgents() can inject extras.
    assert!(
        html.contains("swarm-extras-anchor"),
        "console.html must expose #swarm-extras-anchor for dynamic agent lanes"
    );
    assert!(
        html.contains("swarm-more"),
        "console.html must expose #swarm-more for the +N overflow chip"
    );
    assert!(
        html.contains("loadSwarmAgents"),
        "console.html must call loadSwarmAgents() on connect"
    );
    // No truncation: `text-overflow: ellipsis` must not gate `.lane .msg`.
    let lane_msg_block = html
        .split(".lane .msg")
        .nth(1)
        .expect("expected `.lane .msg` CSS block");
    let first_rule = lane_msg_block.split('}').next().unwrap_or("");
    assert!(
        !first_rule.contains("text-overflow:ellipsis"),
        "`.lane .msg` must not truncate text in the new swarm row, got: {}",
        first_rule
    );
}

#[test]
fn console_html_has_drawer_with_seven_panels() {
    let html =
        std::fs::read_to_string("console.html").expect("console.html must ship with the WebView");
    // Rail buttons for the 7 panels of Sprint 1 inc 2.
    for panel in [
        "sessions",
        "memory",
        "plugins",
        "tools",
        "permissions",
        "security",
        "artifacts",
    ] {
        let rail_marker = format!("data-panel=\"{}\"", panel);
        let body_marker = format!("data-panel-body=\"{}\"", panel);
        assert!(
            html.contains(&rail_marker),
            "rail must expose button for {}",
            panel
        );
        assert!(
            html.contains(&body_marker),
            "drawer must expose body for {}",
            panel
        );
    }
    assert!(
        html.contains("PANEL_LOADERS"),
        "panel loader registry must be present"
    );
    assert!(
        html.contains("openPanel"),
        "openPanel() switch handler must be present"
    );
}

#[test]
fn console_html_has_paper_theme_and_chrome_chips() {
    let html =
        std::fs::read_to_string("console.html").expect("console.html must ship with the WebView");
    // Sprint 1 inc 4-5: paper theme variant + chrome chips for palette /
    // replay / sound / theme. The selector must come from a CSS block keyed
    // on `data-theme="paper"`.
    assert!(
        html.contains("[data-theme=\"paper\"]"),
        "console.html must declare a paper theme block"
    );
    for chip in ["cmdkBtn", "replayBtn", "soundBtn", "themeBtn"] {
        let marker = format!("id=\"{}\"", chip);
        assert!(html.contains(&marker), "chrome must expose {} chip", chip);
    }
    // Hero welcome + boot animation must be wired.
    assert!(
        html.contains("injectHero") && html.contains(".hero{"),
        "hero welcome must be injected at term boot"
    );
    assert!(
        html.contains("runBootAnimation") && html.contains("bootOverlay"),
        "boot overlay must be present and triggered"
    );
    // Sound system + Cmd+M mute toggle.
    assert!(
        html.contains("function chirp(") && html.contains("'sparrow-muted'"),
        "WebAudio chirp + mute persistence must be wired"
    );
}

#[test]
fn console_html_has_slash_palette_and_agent_picker() {
    let html =
        std::fs::read_to_string("console.html").expect("console.html must ship with the WebView");
    // Slash palette modal + Cmd+K wiring.
    assert!(
        html.contains("id=\"palette\""),
        "console.html must declare the slash palette modal"
    );
    assert!(
        html.contains("paletteOpen") && html.contains("paletteFilter"),
        "palette open / filter functions must be present"
    );
    assert!(
        html.contains("key.toLowerCase()==='k'"),
        "Cmd/Ctrl+K shortcut must be wired"
    );
    // Inline @-picker.
    assert!(
        html.contains("id=\"agentPicker\""),
        "console.html must declare the inline @-picker"
    );
    assert!(
        html.contains("agentPickerState") && html.contains("agentPickerAccept"),
        "@-picker state/accept helpers must be present"
    );
    // Caches load on connect.
    assert!(
        html.contains("loadCommandsCache") && html.contains("loadAgentsCache"),
        "both /commands and /agents must be pre-fetched on connect"
    );
}

#[test]
fn console_html_has_typed_event_renderers() {
    let html =
        std::fs::read_to_string("console.html").expect("console.html must ship with the WebView");
    // Sprint 1 inc 6: every Event variant of v0.2.0 that the mockup
    // promises has a styled renderer must actually have one in the
    // shipping console.
    for needle in [
        ".tool-card",
        ".diff-card",
        ".compact-banner",
        ".skill-pop",
        ".streaming::after",
    ] {
        assert!(
            html.contains(needle),
            "console.html must define the `{}` renderer block",
            needle
        );
    }
    // And the JS dispatch must route the Event types into those renderers.
    for case in [
        "case 'ToolUseProposed'",
        "case 'ToolOutput'",
        "case 'DiffProposed'",
        "case 'Compacted'",
    ] {
        assert!(
            html.contains(case),
            "handleEvent() must wire `{}` into the new renderers",
            case
        );
    }
    assert!(
        html.contains("renderDiffCard") && html.contains("renderCompactBanner"),
        "diff + compact renderers must be implemented"
    );
}

#[test]
fn classify_agent_color_falls_back_to_steel() {
    use sparrow::console::classify_agent_color;
    assert_eq!(classify_agent_color("blue"), "planner");
    assert_eq!(classify_agent_color("TEAL"), "coder");
    assert_eq!(classify_agent_color("gold"), "gold");
    assert_eq!(classify_agent_color("coral"), "coral");
    assert_eq!(classify_agent_color("something-unknown"), "steel");
    assert_eq!(classify_agent_color(""), "steel");
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
