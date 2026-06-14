//! Wiring smoke for `sparrow reason` — validates the command parses and
//! dispatches WITHOUT making a model call (no provider/network needed).
//! The inference-time-scaling pipeline itself is unit-tested in
//! `src/reasoning/inference_scaling.rs` with a mock brain.

use std::process::Command;

fn sparrow() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sparrow"))
}

#[test]
fn reason_help_describes_inference_scaling() {
    let out = sparrow()
        .args(["reason", "--help"])
        .output()
        .expect("run sparrow");
    assert!(out.status.success(), "reason --help must exit 0");
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(
        help.contains("best-of-N") || help.contains("inference-time"),
        "help should describe the reasoning-max pipeline, got: {help}"
    );
}

#[test]
fn reason_empty_task_fails_before_any_model_call() {
    // Empty task hits the guard and bails — no provider construction, no network.
    let out = sparrow()
        .args(["reason", ""])
        .output()
        .expect("run sparrow");
    assert!(
        !out.status.success(),
        "empty task must be rejected, not silently run"
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.to_lowercase().contains("usage"), "stderr: {err}");
}
