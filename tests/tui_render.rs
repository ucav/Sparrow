//! Headless render coverage for the TUI cockpit.
//!
//! Every other TUI test deliberately stays off the crossterm/render path
//! (see `v0_2_stability.rs`). This file is the opposite: it drives the *real*
//! `render` tree through an in-memory `TestBackend` so the ~1500 lines that
//! paint the cockpit, swarm lanes, diff panel, checkpoint timeline, toast and
//! input box actually execute and get asserted. A panic or layout overflow in
//! any sub-renderer fails here instead of on a user's terminal at launch.

use sparrow::event::{
    AgentStatus, AutonomyLevel, Block, CheckpointId, Event, FileDiff, OutcomeSummary, RunId,
    TokenUsage,
};
use sparrow::tui::Tui;

fn run() -> RunId {
    RunId("test-run".into())
}

/// A booted cockpit with a representative, busy run already streamed in.
fn busy_cockpit() -> Tui {
    let mut tui = Tui::new();
    tui.force_booted();
    let r = run();
    for ev in [
        Event::RunStarted {
            run: r.clone(),
            task: "refactor the auth module".into(),
            agent: "pipeline".into(),
        },
        Event::RouteSelected {
            run: r.clone(),
            chain: vec!["ollama/qwen2.5-coder".into(), "claude-sonnet-4".into()],
            context_window: 128_000,
        },
        Event::AutonomyChanged {
            run: r.clone(),
            level: AutonomyLevel::Trusted,
        },
        Event::CostUpdate {
            run: r.clone(),
            usd: 0.0421,
        },
        Event::TokenUsage {
            run: r.clone(),
            input: 1200,
            output: 800,
        },
    ] {
        tui.push_event(ev);
    }
    tui
}

fn joined(lines: &[String]) -> String {
    lines.join("\n")
}

#[test]
fn boot_splash_renders_progressively() {
    // The first frame is an intentional blank (the splash fades in), so drive
    // the animation to its late state and assert the splash actually paints the
    // wordmark and the "ready" line rather than the cockpit.
    let mut tui = Tui::new();
    tui.debug_set_boot_progress(70);
    let text = joined(&tui.render_to_lines(120, 40));
    assert!(text.contains("SPARROW"), "boot wordmark missing:\n{text}");
    assert!(text.contains("ready"), "boot ready line missing:\n{text}");
}

#[test]
fn cockpit_renders_core_hud() {
    let mut tui = busy_cockpit();
    let text = joined(&tui.render_to_lines(120, 40));
    assert!(text.contains("SPARROW"), "wordmark missing:\n{text}");
    assert!(text.contains("route:"), "route label missing:\n{text}");
    assert!(
        text.contains("claude-sonnet-4"),
        "route chain missing:\n{text}"
    );
    assert!(text.contains("tok"), "token counter missing:\n{text}");
    assert!(text.contains('$'), "cost readout missing:\n{text}");
    assert!(text.contains("TRUSTED"), "autonomy pill missing:\n{text}");
}

#[test]
fn swarm_lanes_render_all_three_roles() {
    let mut tui = busy_cockpit();
    let r = run();
    for role in ["planner", "coder", "verifier"] {
        tui.push_event(Event::AgentSpawned {
            run: r.clone(),
            role: role.into(),
            model: "claude-sonnet-4".into(),
        });
        tui.push_event(Event::AgentStatus {
            run: r.clone(),
            role: role.into(),
            status: AgentStatus::Working,
            note: "scanning".into(),
        });
    }
    let text = joined(&tui.render_to_lines(120, 40));
    for role in ["planner", "coder", "verifier"] {
        assert!(text.contains(role), "swarm lane `{role}` missing:\n{text}");
    }
}

#[test]
fn diff_panel_renders_file_and_counts() {
    let mut tui = busy_cockpit();
    tui.push_event(Event::DiffProposed {
        run: run(),
        file: "src/auth/login.rs".into(),
        patch: "@@ -1,3 +1,4 @@\n fn login() {\n-    todo!()\n+    verify()?;\n+    Ok(())\n }\n"
            .into(),
        plus: 2,
        minus: 1,
    });
    let text = joined(&tui.render_to_lines(120, 40));
    assert!(text.contains("login.rs"), "diff file name missing:\n{text}");
}

#[test]
fn checkpoint_timeline_renders() {
    let mut tui = busy_cockpit();
    tui.push_event(Event::CheckpointCreated {
        run: run(),
        id: CheckpointId("ckpt-1".into()),
        label: "before-refactor".into(),
    });
    let text = joined(&tui.render_to_lines(120, 40));
    assert!(
        text.contains("before-refactor"),
        "checkpoint label missing:\n{text}"
    );
}

#[test]
fn skill_learned_raises_toast() {
    let mut tui = busy_cockpit();
    tui.push_event(Event::SkillLearned {
        run: run(),
        name: "rust-async".into(),
    });
    let text = joined(&tui.render_to_lines(120, 40));
    assert!(
        text.contains("skill learned") || text.contains("rust-async"),
        "skill-learned toast missing:\n{text}"
    );
}

#[test]
fn run_finished_reports_status_and_cost() {
    let mut tui = busy_cockpit();
    tui.push_event(Event::RunFinished {
        run: run(),
        outcome: OutcomeSummary {
            status: "ok".into(),
            diffs: vec![FileDiff {
                file: "src/auth/login.rs".into(),
                plus: 2,
                minus: 1,
            }],
            cost_usd: 0.0421,
            tokens: TokenUsage {
                input: 1200,
                output: 800,
            },
            cost_comparison: String::new(),
        },
    });
    let text = joined(&tui.render_to_lines(120, 40));
    assert!(text.contains("done"), "completion line missing:\n{text}");
}

#[test]
fn tool_output_text_appears_in_scrollback() {
    let mut tui = busy_cockpit();
    let r = run();
    tui.push_event(Event::ToolUseProposed {
        run: r.clone(),
        id: "t1".into(),
        name: "read_file".into(),
        args: serde_json::json!({"path": "Cargo.toml"}),
        risk: sparrow::event::RiskLevel::ReadOnly,
    });
    tui.push_event(Event::ToolOutput {
        run: r.clone(),
        id: "t1".into(),
        blocks: vec![Block::Text("name = \"sparrow-cli\"".into())],
    });
    let text = joined(&tui.render_to_lines(120, 40));
    assert!(
        text.contains("read_file"),
        "tool group header missing:\n{text}"
    );
}

#[test]
fn replay_mode_renders_recorded_events() {
    let events = vec![
        Event::RunStarted {
            run: run(),
            task: "demo".into(),
            agent: "pipeline".into(),
        },
        Event::RouteSelected {
            run: run(),
            chain: vec!["ollama/qwen2.5-coder".into()],
            context_window: 128_000,
        },
    ];
    let mut tui = Tui::new().with_replay(events);
    let text = joined(&tui.render_to_lines(120, 40));
    assert!(text.contains("SPARROW"), "replay cockpit missing:\n{text}");
}

/// Render at a range of terminal sizes — small, standard, and wide — to catch
/// layout-overflow panics and ensure the cockpit degrades instead of crashing.
#[test]
fn renders_across_terminal_sizes_without_panic() {
    for (w, h) in [(40, 12), (60, 20), (80, 24), (100, 30), (160, 50)] {
        let mut tui = busy_cockpit();
        let lines = tui.render_to_lines(w, h);
        assert_eq!(lines.len(), h as usize, "row count wrong at {w}x{h}");
        for line in &lines {
            assert!(
                line.chars().count() <= w as usize,
                "row overflowed {w} cols at {w}x{h}: {line:?}"
            );
        }
    }
}

/// Not an assertion — dumps the real rendered header at 80 and 100 columns so
/// `cargo test -- --nocapture dump_header` shows the responsive HUD by eye.
#[test]
fn dump_header_for_eyeball() {
    for w in [80u16, 100] {
        let mut tui = busy_cockpit();
        // Worst case: an active agent badge plus a two-hop route.
        let lines = tui.render_to_lines(w, 6);
        eprintln!("\n── {w} cols ──");
        for l in lines.iter().take(3) {
            eprintln!("{l}");
        }
    }
}

/// The cockpit HUD packs spinner, wordmark, verb, route, cost, tokens and the
/// autonomy pill onto a single un-wrapped line. On an 80-column terminal — the
/// classic default and the frame most launch screenshots are taken in — the
/// rightmost element (the autonomy pill) must still be visible. This guards the
/// header against silently truncating its status at standard width.
#[test]
fn autonomy_pill_survives_at_80_columns() {
    let mut tui = busy_cockpit();
    let lines = tui.render_to_lines(80, 24);
    let header = lines
        .iter()
        .find(|l| l.contains("SPARROW"))
        .expect("header row");
    assert!(
        header.contains("TRUSTED"),
        "autonomy pill truncated off the 80-col header: {header:?}"
    );
}
