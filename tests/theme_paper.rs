#[test]
fn console_html_declares_captain_and_paper_theme_tokens() {
    let html =
        std::fs::read_to_string("console.html").expect("console.html must ship with the WebView");
    assert!(
        html.contains(":root{"),
        "captain/default root variables must exist"
    );
    assert!(
        html.contains(":root[data-theme=\"paper\"]"),
        "paper theme block must exist"
    );
    for token in [
        "--bg:#f3eee1",
        "--panel:#e7e0cc",
        "--line:#c6bca3",
        "--fg:#2a2418",
        "--dim:#6a604c",
        "--dimmer:#9a907c",
        "--brand:#a65a1a",
        "--agent:#2f7d67",
        "--planner:#2e5a9c",
        "--coral:#a63a2a",
    ] {
        assert!(
            html.contains(token),
            "paper theme must declare CSS token `{}`",
            token
        );
    }
    assert!(
        html.contains("prefers-color-scheme: light") && html.contains("'sparrow-theme'"),
        "paper theme should auto-detect light mode and persist user override"
    );
}
