//! v0.9 — right tools sidebar (Preview · Diff · Terminal · Files · Tasks · Plan).
//! Marker tests in the same spirit as `ui_finalisation.rs`: the WebView is a
//! single embedded HTML file, so we assert the structural hooks ship with it.

fn console_html() -> String {
    std::fs::read_to_string("console.html").expect("console.html must ship with the WebView")
}

#[test]
fn console_html_declares_right_sidebar_shell() {
    let html = console_html();
    for marker in [
        "id=\"rightbar\"",
        "id=\"rightbarBtn\"",
        "id=\"rbBody\"",
        "id=\"rbClose\"",
        "id=\"rbPin\"",
        "class=\"rb-inner\"",
        "body.rightbar-open .rightbar",
    ] {
        assert!(
            html.contains(marker),
            "console.html must expose right-sidebar hook `{}`",
            marker
        );
    }
}

#[test]
fn right_sidebar_has_six_tools_and_helpers() {
    let html = console_html();
    for tab in ["preview", "diff", "terminal", "files", "tasks", "plan"] {
        assert!(
            html.contains(&format!("{}:{{label:", tab)),
            "RB_TABS must declare the `{}` tool",
            tab
        );
    }
    for helper in [
        "function openRightSidebar(",
        "function closeRightSidebar(",
        "function toggleRightSidebar(",
        "function autoOpenRightSidebar(",
        "function rbOnEvent(",
    ] {
        assert!(
            html.contains(helper),
            "console.html must define `{}`",
            helper
        );
    }
}

#[test]
fn right_sidebar_auto_open_respects_manual_close_and_priorities() {
    let html = console_html();
    assert!(
        html.contains("wasManuallyClosed"),
        "manual close must be tracked so low/medium events stop reopening the panel"
    );
    assert!(
        html.contains("'task-failed'") && html.contains("'high'"),
        "failures must auto-open with high priority"
    );
    assert!(
        html.contains("sparrow-rightbar-autoopen"),
        "the auto-open preference must persist in localStorage"
    );
}

#[test]
fn right_sidebar_empty_states_are_honest() {
    let html = console_html();
    for empty in ["No changes yet.", "No background tasks running."] {
        assert!(
            html.contains(empty),
            "panel must render the clean empty state `{}` instead of fake data",
            empty
        );
    }
}
