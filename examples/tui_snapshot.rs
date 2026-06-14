use std::env;
use std::fs;
use std::path::PathBuf;

use sparrow::event::{
    AgentStatus, AutonomyLevel, Block, CheckpointId, Event, FileDiff, OutcomeSummary, RunId,
    TokenUsage,
};
use sparrow::tui::Tui;

fn run() -> RunId {
    RunId("snapshot-run".into())
}

fn busy_cockpit() -> Tui {
    let mut tui = Tui::new();
    tui.force_booted();
    let r = run();
    for ev in [
        Event::RunStarted {
            run: r.clone(),
            task: "audit and redesign the Sparrow TUI".into(),
            agent: "pipeline".into(),
        },
        Event::RouteSelected {
            run: r.clone(),
            chain: vec![
                "deepseek-v4-pro".into(),
                "claude-sonnet-4".into(),
                "local-verifier".into(),
            ],
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
            input: 4_756,
            output: 410,
        },
    ] {
        tui.push_event(ev);
    }

    for (role, note) in [
        ("planner", "route selected · design constraints mapped"),
        ("coder", "editing the ratatui layout and visual density"),
        ("verifier", "checking overflow at 80, 100 and 140 columns"),
    ] {
        tui.push_event(Event::AgentSpawned {
            run: r.clone(),
            role: role.into(),
            model: "claude-sonnet-4".into(),
        });
        tui.push_event(Event::AgentStatus {
            run: r.clone(),
            role: role.into(),
            status: AgentStatus::Working,
            note: note.into(),
        });
    }

    tui.push_event(Event::ToolUseProposed {
        run: r.clone(),
        id: "tool-1".into(),
        name: "read_file".into(),
        args: serde_json::json!({"path": "src/tui/mod.rs"}),
        risk: sparrow::event::RiskLevel::ReadOnly,
    });
    tui.push_event(Event::ToolOutput {
        run: r.clone(),
        id: "tool-1".into(),
        blocks: vec![Block::Text(
            "render_cockpit · render_swarm_lanes · render_scroll · render_input".into(),
        )],
        is_error: false,
    });
    tui.push_event(Event::DiffProposed {
        run: r.clone(),
        file: "src/tui/mod.rs".into(),
        patch: "@@ -1,3 +1,5 @@\n-old framed panels\n+minimal rails\n+compact agent lanes\n".into(),
        plus: 2,
        minus: 1,
    });
    tui.push_event(Event::CheckpointCreated {
        run: r.clone(),
        id: CheckpointId("ckpt-ui-1".into()),
        label: "before tui redesign".into(),
    });
    tui.push_event(Event::RunFinished {
        run: r,
        outcome: OutcomeSummary {
            status: "ok".into(),
            diffs: vec![FileDiff {
                file: "src/tui/mod.rs".into(),
                plus: 120,
                minus: 80,
            }],
            cost_usd: 0.0421,
            tokens: TokenUsage {
                input: 4_756,
                output: 410,
            },
            cost_comparison: String::new(),
            duration_ms: Some(12_340),
        },
    });
    tui
}

fn main() -> anyhow::Result<()> {
    let out_dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("artifacts/tui-snapshots"));
    fs::create_dir_all(&out_dir)?;

    for (name, width, height) in [("standard", 120, 40), ("compact", 90, 30)] {
        let mut tui = busy_cockpit();
        let lines = tui.render_to_lines(width, height);
        fs::write(
            out_dir.join(format!("{name}-{width}x{height}.txt")),
            lines.join("\n"),
        )?;
    }

    Ok(())
}
