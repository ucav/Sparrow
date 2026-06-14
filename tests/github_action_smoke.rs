//! End-to-end smoke for the GitHub Action's CLI surface — the part that can be
//! verified WITHOUT a live GitHub token or network.
//!
//! The Action's `review` mode runs `sparrow github review <pr> [--dry-run]`.
//! With `--dry-run` it must build and print the review plan and exit 0 without
//! shelling out to `gh` or calling a model. This locks that contract so the
//! Action's dry-run path (the only one CI can exercise without secrets) can't
//! silently regress. Live token round-trips still require a human / secrets CI.

use std::process::Command;

fn sparrow() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sparrow"))
}

#[test]
fn github_review_dry_run_prints_plan_offline() {
    let out = sparrow()
        .args(["github", "review", "42", "--dry-run"])
        // Prove it does not depend on a token being present.
        .env_remove("GITHUB_TOKEN")
        .output()
        .expect("failed to run sparrow");

    assert!(
        out.status.success(),
        "dry-run review must exit 0 offline; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let json: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("dry-run must emit valid JSON plan");
    assert_eq!(json["pr"], 42);
    assert_eq!(json["dry_run"], true);
}

#[test]
fn github_review_dry_run_parses_allowed_tools() {
    let out = sparrow()
        .args([
            "github",
            "review",
            "7",
            "--dry-run",
            "--allowed-tools",
            "read_file, exec ,grep",
        ])
        .output()
        .expect("failed to run sparrow");
    assert!(out.status.success());

    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON plan");
    let tools = json["allowed_tools"]
        .as_array()
        .expect("allowed_tools array");
    // Whitespace trimmed, empties dropped.
    let names: Vec<&str> = tools.iter().filter_map(|t| t.as_str()).collect();
    assert_eq!(names, vec!["read_file", "exec", "grep"]);
}
