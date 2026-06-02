use crate::commands::SlashCommand;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadOnlyPlan {
    pub task: String,
    pub summary: String,
    pub steps: Vec<String>,
    pub risks: Vec<String>,
    pub acceptance: Vec<String>,
    pub estimated_tier: String,
    pub read_only: bool,
}

impl ReadOnlyPlan {
    pub fn render_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# Sparrow Plan\n\n");
        out.push_str("Mode: read-only. No tools, edits, exec, or checkpoints were run.\n\n");
        out.push_str(&format!("Task: {}\n\n", self.task));
        out.push_str(&format!("Tier: {}\n\n", self.estimated_tier));
        out.push_str("## Summary\n");
        out.push_str(&self.summary);
        out.push_str("\n\n## Steps\n");
        for (idx, step) in self.steps.iter().enumerate() {
            out.push_str(&format!("{}. {}\n", idx + 1, step));
        }
        out.push_str("\n## Risks\n");
        for risk in &self.risks {
            out.push_str(&format!("- {}\n", risk));
        }
        out.push_str("\n## Acceptance\n");
        for item in &self.acceptance {
            out.push_str(&format!("- {}\n", item));
        }
        out
    }
}

pub fn build_read_only_plan(task: &str, commands: &[SlashCommand]) -> ReadOnlyPlan {
    let trimmed = task.trim();
    let lower = trimmed.to_lowercase();
    let mut steps = vec![
        "Inspect the relevant files and current repository state before editing.".into(),
        "Identify the smallest safe implementation path and any affected tests.".into(),
        "Apply scoped changes only after the plan is accepted.".into(),
        "Run targeted checks, then broader build/test gates if code changed.".into(),
        "Summarize files changed, verification evidence, and remaining risk.".into(),
    ];

    if lower.contains("github") || lower.contains("ci") || lower.contains("pull request") {
        steps.insert(
            1,
            "Inspect GitHub/CI state and collect failing job logs before changing code.".into(),
        );
    }
    if lower.contains("webview") || lower.contains("ui") || lower.contains("console") {
        steps
            .push("Render or screenshot the UI to confirm layout and interaction behavior.".into());
    }
    if lower.contains("memory") || lower.contains("session") {
        steps.push("Verify persistence with a restart-safe session or memory check.".into());
    }

    let risks = vec![
        "This plan is read-only; execution still needs approval or a follow-up run.".into(),
        "Broad requests may need decomposition into smaller phases to keep tests meaningful."
            .into(),
        "Existing local unpushed commits must be preserved and not rewritten.".into(),
    ];

    let mut acceptance = vec![
        "No filesystem mutation occurs during plan generation.".into(),
        "The accepted execution path has concrete tests or smoke checks.".into(),
        "The final answer cites current evidence, not intent.".into(),
    ];
    if !commands.is_empty() {
        acceptance.push(format!(
            "{} slash command(s) are available for follow-up control.",
            commands.len()
        ));
    }

    ReadOnlyPlan {
        task: trimmed.into(),
        summary: summarize_task(trimmed),
        steps,
        risks,
        acceptance,
        estimated_tier: estimate_tier(trimmed).into(),
        read_only: true,
    }
}

fn summarize_task(task: &str) -> String {
    if task.is_empty() {
        return "No task was provided.".into();
    }
    format!(
        "Sparrow should treat this as a controlled execution request: understand the goal, preserve existing work, then act only after the user accepts the plan. The requested task is: {}",
        task
    )
}

fn estimate_tier(task: &str) -> &'static str {
    let lower = task.to_lowercase();
    if lower.contains("image") || lower.contains("vision") || lower.contains("screenshot") {
        "vision"
    } else if lower.contains("audit")
        || lower.contains("architecture")
        || lower.contains("refactor")
        || lower.contains("autonomie")
        || lower.contains("complete")
    {
        "hard"
    } else if lower.contains("bug")
        || lower.contains("fix")
        || lower.contains("corrige")
        || lower.contains("test")
    {
        "small"
    } else if task.len() > 260 {
        "medium"
    } else {
        "trivial"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_is_read_only_and_mentions_no_mutation() {
        let plan = build_read_only_plan("corrige le bug WebView", &[]);
        assert!(plan.read_only);
        assert!(plan.render_markdown().contains("No tools, edits, exec"));
        assert_eq!(plan.estimated_tier, "small");
    }
}
