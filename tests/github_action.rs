use sparrow::github::{plan_review, require_action_env};

#[test]
fn action_yml_is_present_and_parses() {
    let content =
        std::fs::read_to_string("action.yml").expect("action.yml must exist at repo root");
    // Cheap structural check — full YAML parsing would add a dep; we only need
    // to know the required inputs are declared.
    assert!(content.contains("name: \"Sparrow\""), "action name");
    assert!(
        content.contains("github-token"),
        "github-token input declared"
    );
    assert!(content.contains("mode"), "mode input declared");
    assert!(content.contains("dry-run"), "dry-run input declared");
    assert!(content.contains("composite"), "composite action");
}

#[test]
fn sample_workflow_is_present() {
    let content = std::fs::read_to_string(".github/workflows/sparrow-pr-review.yml")
        .expect("sample workflow must exist");
    assert!(content.contains("pull-requests: write"));
    assert!(content.contains("contents: read"));
    assert!(content.contains("ucav/Sparrow@master"));
    assert!(content.contains("mode: review"));
}

#[test]
fn missing_github_token_fails_with_clear_error() {
    unsafe {
        std::env::remove_var("GITHUB_TOKEN");
    }
    let err = require_action_env().unwrap_err().to_string();
    assert!(
        err.contains("GITHUB_TOKEN"),
        "error must name the missing variable: {}",
        err
    );
}

#[test]
fn dry_run_review_does_not_require_gh() {
    // Should never call into gh — pure data shaping.
    let plan = plan_review(
        123,
        Some("claude-opus-4-7".into()),
        Some("fs_read,search".into()),
        true,
    );
    assert_eq!(plan.pr, 123);
    assert!(plan.dry_run);
    assert_eq!(plan.allowed_tools, vec!["fs_read", "search"]);
    let json = serde_json::to_string(&plan).expect("plan must serialize");
    assert!(json.contains("\"pr\":123"));
    assert!(json.contains("\"dry_run\":true"));
}
