use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct CommitPlan {
    pub message: String,
    pub staged_stat: String,
    pub secret_findings: Vec<String>,
}

pub fn build_commit_plan(
    message: Option<String>,
    staged_stat: String,
    staged_diff: &str,
) -> CommitPlan {
    let secret_findings = scan_diff_for_secret_patterns(staged_diff);
    let message = message.unwrap_or_else(|| generated_message(&staged_stat));
    CommitPlan {
        message,
        staged_stat,
        secret_findings,
    }
}

pub fn scan_diff_for_secret_patterns(diff: &str) -> Vec<String> {
    let mut findings = Vec::new();
    for (idx, line) in diff.lines().enumerate() {
        if !line.starts_with('+') || line.starts_with("+++") {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        let suspicious = lower.contains("api_key")
            || lower.contains("secret")
            || lower.contains("token")
            || line.contains("sk-")
            || line.contains("ghp_")
            || line.contains("xoxb-")
            || line.contains("AKIA");
        if suspicious {
            findings.push(format!("line {}: {}", idx + 1, line.trim()));
        }
    }
    findings
}

fn generated_message(staged_stat: &str) -> String {
    if staged_stat.contains("docs/") || staged_stat.contains("README") {
        "docs: update Sparrow release materials".into()
    } else if staged_stat.contains("Cargo.toml") || staged_stat.contains("Cargo.lock") {
        "chore: update Sparrow workspace configuration".into()
    } else if staged_stat.contains("console.html") {
        "feat: improve Sparrow console experience".into()
    } else {
        "chore: update Sparrow".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_plan_detects_secret_like_added_lines() {
        let plan = build_commit_plan(
            None,
            "Cargo.toml | 2 +".into(),
            "+OPENAI_API_KEY=sk-test\n context\n",
        );
        assert_eq!(
            plan.message,
            "chore: update Sparrow workspace configuration"
        );
        assert_eq!(plan.secret_findings.len(), 1);
    }
}
