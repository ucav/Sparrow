//! GitHub Action / remote PR workflow helpers.
//!
//! These functions thinly wrap the `gh` CLI so that workflow steps can call
//! `sparrow github review|status|logs` from inside an Action. They never
//! invent results: a missing `gh` binary or missing `GITHUB_TOKEN` returns a
//! clear error.

use std::process::Command;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ReviewPlan {
    pub pr: u64,
    pub model: Option<String>,
    pub allowed_tools: Vec<String>,
    pub dry_run: bool,
    pub diff_preview: String,
}

/// Returns Err with a clear message when the environment is not set up for
/// GitHub Action use. Used by every subcommand so the action fails loudly when
/// secrets are missing instead of silently no-oping.
pub fn require_action_env() -> anyhow::Result<()> {
    if std::env::var("GITHUB_TOKEN")
        .ok()
        .filter(|s| !s.is_empty())
        .is_none()
    {
        anyhow::bail!(
            "GITHUB_TOKEN is not set. The Sparrow GitHub Action requires a token \
             with `pull-requests: write` and `contents: read` permissions."
        );
    }
    if !gh_available() {
        anyhow::bail!(
            "`gh` CLI is not on PATH. The Sparrow GitHub Action depends on the \
             official GitHub CLI being installed on the runner."
        );
    }
    Ok(())
}

pub fn gh_available() -> bool {
    Command::new("gh")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Builds a `ReviewPlan` from inputs. With `dry_run = true` this never shells
/// out and is safe in tests.
pub fn plan_review(
    pr: u64,
    model: Option<String>,
    allowed_tools: Option<String>,
    dry_run: bool,
) -> ReviewPlan {
    let tools = allowed_tools
        .map(|s| {
            s.split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default();
    ReviewPlan {
        pr,
        model,
        allowed_tools: tools,
        dry_run,
        diff_preview: String::new(),
    }
}

/// Fetch the diff for `pr` via `gh pr diff <pr>`. Returns the stdout text on
/// success, otherwise an error that includes stderr — never a silent empty
/// string.
pub fn fetch_pr_diff(pr: u64) -> anyhow::Result<String> {
    let out = Command::new("gh")
        .args(["pr", "diff", &pr.to_string()])
        .output()?;
    if !out.status.success() {
        anyhow::bail!(
            "gh pr diff {} failed (exit {:?}): {}",
            pr,
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Returns `gh run list --limit 5` as raw text. Used by `sparrow github status`.
pub fn ci_status() -> anyhow::Result<String> {
    let out = Command::new("gh")
        .args(["run", "list", "--limit", "5"])
        .output()?;
    if !out.status.success() {
        anyhow::bail!(
            "gh run list failed (exit {:?}): {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Returns `gh run view <id> --log` as raw text. Used by `sparrow github logs`.
pub fn ci_logs(run_id: &str) -> anyhow::Result<String> {
    let out = Command::new("gh")
        .args(["run", "view", run_id, "--log"])
        .output()?;
    if !out.status.success() {
        anyhow::bail!(
            "gh run view {} --log failed (exit {:?}): {}",
            run_id,
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_token_is_clear_error() {
        // Force-unset; safe in test process.
        unsafe {
            std::env::remove_var("GITHUB_TOKEN");
        }
        let res = require_action_env();
        assert!(res.is_err());
        let msg = res.unwrap_err().to_string();
        assert!(msg.contains("GITHUB_TOKEN"), "got: {}", msg);
    }

    #[test]
    fn plan_review_parses_tools_csv() {
        let plan = plan_review(
            42,
            Some("claude-sonnet-4-6".into()),
            Some("fs_read, edit, search".into()),
            true,
        );
        assert_eq!(plan.pr, 42);
        assert_eq!(plan.model.as_deref(), Some("claude-sonnet-4-6"));
        assert_eq!(plan.allowed_tools, vec!["fs_read", "edit", "search"]);
        assert!(plan.dry_run);
    }

    #[test]
    fn plan_review_handles_no_tools() {
        let plan = plan_review(1, None, None, true);
        assert!(plan.allowed_tools.is_empty());
        assert!(plan.model.is_none());
    }

    #[test]
    fn plan_review_trims_empty_csv_entries() {
        let plan = plan_review(7, None, Some(" , fs_read , ".into()), false);
        assert_eq!(plan.allowed_tools, vec!["fs_read"]);
    }
}
