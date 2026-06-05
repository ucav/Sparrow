use serde::{Deserialize, Serialize};

use crate::event::{CheckpointId, Decision, RiskLevel};

pub use crate::event::AutonomyLevel;

// ─── Autonomy contract ──────────────────────────────────────────────────────────

/// Continuous trust contract attached to every run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutonomyContract {
    pub level: AutonomyLevel,
    pub approve: ApprovalPolicy,
    pub budget: Budget,
    pub stops: Vec<HardStop>,
}

impl AutonomyContract {
    pub fn supervised() -> Self {
        Self {
            level: AutonomyLevel::Supervised,
            approve: ApprovalPolicy::default_supervised(),
            budget: Budget::default(),
            stops: vec![
                HardStop::RiskLevel(RiskLevel::Destructive),
                HardStop::BudgetExceeded,
            ],
        }
    }

    pub fn trusted() -> Self {
        Self {
            level: AutonomyLevel::Trusted,
            approve: ApprovalPolicy::default_trusted(),
            budget: Budget::default(),
            stops: vec![HardStop::RiskLevel(RiskLevel::Destructive)],
        }
    }

    pub fn autonomous() -> Self {
        Self {
            level: AutonomyLevel::Autonomous,
            approve: ApprovalPolicy::default_autonomous(),
            budget: Budget::default(),
            stops: vec![],
        }
    }

    pub fn decide(&self, action: &ProposedAction) -> Decision {
        self.evaluate(action).decision
    }

    pub fn evaluate(&self, action: &ProposedAction) -> AutonomyVerdict {
        // Check hard stops first
        for stop in &self.stops {
            match stop {
                HardStop::RiskLevel(rl) if action.risk == *rl => {
                    return AutonomyVerdict::new(
                        Decision::Deny,
                        false,
                        false,
                        format!("hard stop blocks {:?} actions", action.risk),
                    );
                }
                _ => {}
            }
        }
        let decision = self.approve.decide(action);
        let needs_checkpoint = matches!(
            action.risk,
            RiskLevel::Mutating | RiskLevel::Exec | RiskLevel::Destructive
        ) && matches!(decision, Decision::Allow | Decision::AskUser);
        let notify = matches!(self.level, AutonomyLevel::Trusted)
            && matches!(decision, Decision::Allow)
            && matches!(action.risk, RiskLevel::Mutating | RiskLevel::Exec);
        AutonomyVerdict::new(
            decision,
            needs_checkpoint,
            notify,
            format!("{:?} policy for tool '{}'", self.level, action.tool_name),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutonomyVerdict {
    pub decision: Decision,
    pub needs_checkpoint: bool,
    pub notify: bool,
    pub reason: String,
}

impl AutonomyVerdict {
    fn new(
        decision: Decision,
        needs_checkpoint: bool,
        notify: bool,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            decision,
            needs_checkpoint,
            notify,
            reason: reason.into(),
        }
    }
}

// ─── Approval policy ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalPolicy {
    pub read_only: Decision,
    pub mutating: Decision,
    pub exec: Decision,
    pub destructive: Decision,
    pub network: Decision,
}

impl ApprovalPolicy {
    pub fn default_supervised() -> Self {
        Self {
            read_only: Decision::Allow,
            mutating: Decision::AskUser,
            exec: Decision::AskUser,
            destructive: Decision::Deny,
            network: Decision::AskUser,
        }
    }

    pub fn default_trusted() -> Self {
        Self {
            read_only: Decision::Allow,
            mutating: Decision::Allow,
            exec: Decision::Allow,
            destructive: Decision::AskUser,
            network: Decision::Allow,
        }
    }

    pub fn default_autonomous() -> Self {
        Self {
            read_only: Decision::Allow,
            mutating: Decision::Allow,
            exec: Decision::Allow,
            destructive: Decision::AskUser,
            network: Decision::Allow,
        }
    }

    pub fn decide(&self, action: &ProposedAction) -> Decision {
        match action.risk {
            RiskLevel::ReadOnly => self.read_only.clone(),
            RiskLevel::Mutating => self.mutating.clone(),
            RiskLevel::Exec => self.exec.clone(),
            RiskLevel::Destructive => self.destructive.clone(),
            RiskLevel::Network => self.network.clone(),
        }
    }
}

// ─── Proposed action ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ProposedAction {
    pub tool_name: String,
    pub risk: RiskLevel,
    pub args: serde_json::Value,
}

// ─── Budget ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Budget {
    pub max_usd: f64,
    pub max_tokens: u64,
    pub max_wallclock_secs: u64,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            max_usd: 5.0,
            max_tokens: 100_000,
            max_wallclock_secs: 3600,
        }
    }
}

// ─── Hard stops ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HardStop {
    RiskLevel(RiskLevel),
    BudgetExceeded,
    SandboxEscape,
    RepeatedToolFailure,
}

// ─── THE GATE TRAIT ─────────────────────────────────────────────────────────────

pub trait Gate: Send + Sync {
    fn decide(&self, action: &ProposedAction) -> Decision;
}

impl Gate for AutonomyContract {
    fn decide(&self, action: &ProposedAction) -> Decision {
        self.decide(action)
    }
}

// ─── Checkpoints ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Checkpoint {
    pub id: CheckpointId,
    pub label: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Snapshot and rewind workspace state.
pub trait Checkpoints: Send + Sync {
    fn snapshot(&self, label: &str) -> anyhow::Result<CheckpointId>;
    fn list(&self) -> Vec<Checkpoint>;
    fn rewind(&self, to: CheckpointId) -> anyhow::Result<()>;
    /// Show unified diff between HEAD and a checkpoint ref.
    fn diff(&self, id: &CheckpointId) -> anyhow::Result<String>;
    /// Delete checkpoint refs older than `older_than_days` days.
    /// Returns the number of refs deleted.
    fn prune(&self, older_than_days: u64) -> anyhow::Result<usize>;
}

/// Git-backed checkpoint implementation (basic, M0).
pub struct GitCheckpoints {
    repo_path: std::path::PathBuf,
}

impl GitCheckpoints {
    pub fn new(repo_path: std::path::PathBuf) -> Self {
        Self { repo_path }
    }
}

impl Checkpoints for GitCheckpoints {
    fn snapshot(&self, label: &str) -> anyhow::Result<CheckpointId> {
        let id = CheckpointId::new();
        use std::process::Command;

        let in_repo = Command::new("git")
            .args(["rev-parse", "--is-inside-work-tree"])
            .current_dir(&self.repo_path)
            .output()?;

        if !in_repo.status.success() {
            anyhow::bail!("Not a git repository: {}", self.repo_path.display());
        }

        let stash = Command::new("git")
            .args(["stash", "create", &format!("SPARROW-CHECKPOINT: {}", label)])
            .current_dir(&self.repo_path)
            .output()?;

        let mut sha = String::from_utf8_lossy(&stash.stdout).trim().to_string();
        if sha.is_empty() {
            let head = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(&self.repo_path)
                .output()?;
            if !head.status.success() {
                anyhow::bail!("Cannot create checkpoint without HEAD");
            }
            sha = String::from_utf8_lossy(&head.stdout).trim().to_string();
        }

        let ref_name = format!("refs/sparrow/checkpoints/{}", id.0);
        let status = Command::new("git")
            .args(["update-ref", &ref_name, &sha])
            .current_dir(&self.repo_path)
            .status()?;

        if !status.success() {
            anyhow::bail!("Failed to save checkpoint ref {}", ref_name);
        }

        Ok(id)
    }

    fn list(&self) -> Vec<Checkpoint> {
        use std::process::Command;
        let output = Command::new("git")
            .args([
                "for-each-ref",
                "refs/sparrow/checkpoints",
                "--format=%(refname:short) %(objectname:short) %(creatordate:iso)",
            ])
            .current_dir(&self.repo_path)
            .output()
            .ok();

        let mut checkpoints = Vec::new();
        if let Some(output) = output {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(name) = parts.first() {
                    let id = name.rsplit('/').next().unwrap_or(name).to_string();
                    checkpoints.push(Checkpoint {
                        id: CheckpointId(id.clone()),
                        label: format!("checkpoint {}", id),
                        timestamp: chrono::Utc::now(),
                    });
                }
            }
        }
        checkpoints
    }

    fn rewind(&self, to: CheckpointId) -> anyhow::Result<()> {
        use std::process::Command;
        let ref_name = format!("refs/sparrow/checkpoints/{}", to.0);
        let status = Command::new("git")
            .args(["reset", "--hard", &ref_name])
            .current_dir(&self.repo_path)
            .status()?;

        if !status.success() {
            anyhow::bail!("Failed to rewind to checkpoint {}", to.0);
        }
        Ok(())
    }

    fn diff(&self, id: &CheckpointId) -> anyhow::Result<String> {
        use std::process::Command;
        let ref_name = format!("refs/sparrow/checkpoints/{}", id.0);
        let output = Command::new("git")
            .args(["diff", &ref_name, "HEAD"])
            .current_dir(&self.repo_path)
            .output()?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            anyhow::bail!("git diff failed for checkpoint {}: {}", id.0, err);
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn prune(&self, older_than_days: u64) -> anyhow::Result<usize> {
        use std::process::Command;
        // List all sparrow checkpoint refs with their creator dates
        let output = Command::new("git")
            .args([
                "for-each-ref",
                "refs/sparrow/checkpoints",
                "--format=%(refname) %(creatordate:unix)",
            ])
            .current_dir(&self.repo_path)
            .output()?;
        if !output.status.success() {
            return Ok(0);
        }
        let cutoff = chrono::Utc::now()
            .timestamp()
            .saturating_sub((older_than_days * 86_400) as i64);
        let text = String::from_utf8_lossy(&output.stdout);
        let to_delete: Vec<String> = text
            .lines()
            .filter_map(|line| {
                let mut parts = line.splitn(2, ' ');
                let refname = parts.next()?.trim().to_string();
                let ts: i64 = parts.next()?.trim().parse().ok()?;
                if ts < cutoff { Some(refname) } else { None }
            })
            .collect();
        let count = to_delete.len();
        for refname in &to_delete {
            let _ = Command::new("git")
                .args(["update-ref", "-d", refname])
                .current_dir(&self.repo_path)
                .status();
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(risk: RiskLevel) -> ProposedAction {
        ProposedAction {
            tool_name: "edit".into(),
            risk,
            args: serde_json::json!({}),
        }
    }

    #[test]
    fn trusted_mutating_verdict_requires_checkpoint_and_notify() {
        let verdict = AutonomyContract::trusted().evaluate(&action(RiskLevel::Mutating));
        assert_eq!(verdict.decision, Decision::Allow);
        assert!(verdict.needs_checkpoint);
        assert!(verdict.notify);
    }

    #[test]
    fn supervised_readonly_verdict_needs_no_checkpoint() {
        let verdict = AutonomyContract::supervised().evaluate(&action(RiskLevel::ReadOnly));
        assert_eq!(verdict.decision, Decision::Allow);
        assert!(!verdict.needs_checkpoint);
        assert!(!verdict.notify);
    }

    #[test]
    fn hard_stop_verdict_denies_without_checkpoint() {
        let mut contract = AutonomyContract::autonomous();
        contract
            .stops
            .push(HardStop::RiskLevel(RiskLevel::Destructive));

        let verdict = contract.evaluate(&action(RiskLevel::Destructive));
        assert_eq!(verdict.decision, Decision::Deny);
        assert!(!verdict.needs_checkpoint);
        assert!(!verdict.notify);
        assert!(verdict.reason.contains("hard stop"));
    }
}
