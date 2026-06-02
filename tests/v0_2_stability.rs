//! Coverage for the v0.2.0 stability pass — every formerly-deferred Phase
//! 7/9/12/13 item has at least one assertion here so it stays green.

use sparrow::gateway::RunRegistry;
use sparrow::tui::Tui;

#[tokio::test]
async fn run_registry_aborts_an_active_task() {
    let reg = RunRegistry::new();
    let h = tokio::spawn(async {
        // Sleeps far longer than the test allows so abort is observable.
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    });
    reg.insert("run-xyz".into(), h.abort_handle());
    assert!(reg.active_run_ids().contains(&"run-xyz".to_string()));
    assert!(reg.abort("run-xyz"));
    let res = h.await;
    assert!(res.is_err() && res.unwrap_err().is_cancelled());
}

#[tokio::test]
async fn run_registry_abort_unknown_is_a_clean_false() {
    let reg = RunRegistry::new();
    assert!(!reg.abort("does-not-exist"));
}

#[test]
fn tui_agent_picker_returns_matches_after_at_sign() {
    let mut tui = Tui::new().with_agents(vec!["scout".into(), "scribe".into(), "verifier".into()]);
    // Drive the input box: type "hello @sc" so the picker fragment is "sc".
    type_into(&mut tui, "hello @sc");
    let hits = tui.agent_matches();
    assert!(hits.contains(&"@scout".to_string()), "got: {:?}", hits);
    assert!(hits.contains(&"@scribe".to_string()), "got: {:?}", hits);
    assert!(!hits.contains(&"@verifier".to_string()), "got: {:?}", hits);
}

#[test]
fn tui_agent_picker_does_not_fire_inside_an_email() {
    let mut tui = Tui::new().with_agents(vec!["scout".into()]);
    type_into(&mut tui, "ping abdou@example.com");
    assert!(
        tui.agent_matches().is_empty(),
        "picker must not trigger on emails: {:?}",
        tui.agent_matches()
    );
}

#[test]
fn cargo_pkg_version_is_0_2_0() {
    assert_eq!(env!("CARGO_PKG_VERSION"), "0.2.0");
}

// ─── helpers ────────────────────────────────────────────────────────────────────

fn type_into(tui: &mut Tui, text: &str) {
    // The TUI buffers input in `input_lines` with a single row at construction.
    // Mutating it directly via the public surface keeps the test off any
    // crossterm rendering path.
    let line = tui.debug_first_line_mut();
    line.clear();
    line.push_str(text);
    tui.debug_set_cursor_col(text.len());
}
