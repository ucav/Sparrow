// src/cmd_handlers/handle_github_cmd.rs
pub fn handle_github(action: sparrow::cli::GithubAction) -> anyhow::Result<()> {
    use sparrow::cli::GithubAction;
    use sparrow::github;

    match action {
        GithubAction::Review {
            pr,
            dry_run,
            model,
            allowed_tools,
        } => {
            let mut plan = github::plan_review(pr, model, allowed_tools, dry_run);
            if dry_run {
                println!("{}", serde_json::to_string_pretty(&plan)?);
                return Ok(());
            }
            github::require_action_env()?;
            plan.diff_preview = github::fetch_pr_diff(pr)?;
            // We intentionally do NOT call the model here. The actual review
            // is performed by `sparrow run` invoked separately by the Action
            // composite step, so this command stays a pure data-fetcher.
            println!("{}", serde_json::to_string_pretty(&plan)?);
        }
        GithubAction::Status => {
            github::require_action_env()?;
            println!("{}", github::ci_status()?);
        }
        GithubAction::Logs { run_id } => {
            github::require_action_env()?;
            println!("{}", github::ci_logs(&run_id)?);
        }
    }
    Ok(())
}

// ─── MCP commands ───────────────────────────────────────────────────────────────
