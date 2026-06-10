// src/cmd_handlers/handle_review_cmd.rs — `sparrow review` pipeline
use super::prelude::*;
use std::process::Command;

/// Adversarial read-only review of the current local diff.
///
/// The flow is intentionally narrow: collect the diff against a base ref,
/// frame it with an explicit reviewer protocol (security → correctness →
/// regressions → performance → readability), and run it through `run_task`
/// with the standard pre-run quote / continuation flags switched off
/// because the user just asked for a one-shot review, not a conversation.
pub async fn handle_review(
    base: Option<String>,
    paths: Vec<String>,
    dry_run: bool,
    config: &sparrow::config::Config,
    memory: Arc<dyn Memory>,
    skills: Arc<dyn SkillLibrary>,
    recorder: Arc<FsRecorder>,
) -> anyhow::Result<()> {
    let workspace = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    if !workspace.join(".git").exists() {
        anyhow::bail!(
            "`sparrow review` needs a git repository (no .git/ in {}). Use \
             `sparrow run \"…\"` for free-form review of arbitrary text.",
            workspace.display()
        );
    }

    let base_ref = base.unwrap_or_else(|| resolve_default_base(&workspace));

    let mut args: Vec<String> = vec![
        "diff".into(),
        "--stat".into(),
        format!("{}...HEAD", base_ref),
    ];
    if !paths.is_empty() {
        args.push("--".into());
        args.extend(paths.iter().cloned());
    }
    let stat = run_git(&workspace, &args).unwrap_or_default();

    let mut diff_args: Vec<String> = vec![
        "diff".into(),
        "--no-color".into(),
        "-U6".into(),
        format!("{}...HEAD", base_ref),
    ];
    if !paths.is_empty() {
        diff_args.push("--".into());
        diff_args.extend(paths.iter().cloned());
    }
    let diff_text = run_git(&workspace, &diff_args).unwrap_or_default();
    let uncommitted =
        run_git(&workspace, &["diff", "--no-color", "-U6", "HEAD"]).unwrap_or_default();

    if diff_text.trim().is_empty() && uncommitted.trim().is_empty() {
        println!(
            "No changes to review (base `{}` matches HEAD and working tree is clean).",
            base_ref
        );
        return Ok(());
    }

    // Truncate hard if the diff is huge: 60k chars is already several model
    // turns. A bigger diff means the user should narrow with `--paths`.
    const MAX_DIFF_CHARS: usize = 60_000;
    let mut combined = String::new();
    combined.push_str(&diff_text);
    if !uncommitted.trim().is_empty() {
        combined.push_str("\n\n# Uncommitted changes (not yet committed)\n\n");
        combined.push_str(&uncommitted);
    }
    let truncated = combined.len() > MAX_DIFF_CHARS;
    if truncated {
        combined.truncate(MAX_DIFF_CHARS);
        combined.push_str("\n\n[…diff truncated — narrow with --paths]");
    }

    let prompt = format!(
        "# Adversarial code review\n\
         \n\
         You are reviewing a local diff. **Read-only**: do not edit files, do \
         not propose to commit anything, do not run network tools. Your output \
         is a structured review, not a fix.\n\
         \n\
         ## Base\n\
         `{base}`{trunc}\n\
         \n\
         ## Diffstat\n```\n{stat}\n```\n\
         \n\
         ## Review checklist (in priority order)\n\
         1. **Security** — secrets exposed, injection, path traversal, unsafe \
            deserialisation, missing authn/authz.\n\
         2. **Correctness** — does the change actually solve the stated problem? \
            What edge cases are unhandled?\n\
         3. **Regressions** — what existing behaviour could this break? Which \
            tests would catch the regression, and is one included?\n\
         4. **Performance** — needless allocations, N+1 queries, hidden O(n²), \
            blocking calls on hot paths.\n\
         5. **Readability** — confusing names, functions doing two things, \
            comments that just paraphrase the next line.\n\
         \n\
         ## Output format\n\
         Group findings by severity: `Critical`, `Important`, `Nit`. For each \
         finding, quote 1–3 lines of the diff (with file:line) before \
         explaining. End with one sentence: SHIP / FIX FIRST / NEEDS \
         DISCUSSION.\n\
         \n\
         ## Diff\n```diff\n{diff}\n```",
        base = base_ref,
        trunc = if truncated { " (diff truncated)" } else { "" },
        stat = stat,
        diff = combined,
    );

    if dry_run {
        println!("{}", prompt);
        return Ok(());
    }

    // Reviews are always one-shot: never continue a session, always quote,
    // never carry assume_yes (we want the quote for an explicit confirm).
    let flags = RunFlags {
        session_mode: SessionMode::Fresh,
        assume_yes: false,
    };
    run_task(&prompt, config, memory, skills, recorder, None, flags).await
}

fn run_git(workspace: &std::path::Path, args: &[impl AsRef<std::ffi::OsStr>]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(args)
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

/// Pick the most sensible base ref: try `origin/main`, then `main`, then
/// `HEAD~1` so even a brand-new repo with one commit can be reviewed.
fn resolve_default_base(workspace: &std::path::Path) -> String {
    for candidate in ["origin/main", "main", "origin/master", "master", "HEAD~1"] {
        if run_git(workspace, &["rev-parse", "--verify", "--quiet", candidate]).is_some() {
            return candidate.to_string();
        }
    }
    "HEAD~1".to_string()
}
