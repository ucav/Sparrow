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
        // Check hard stops first
        for stop in &self.stops {
            match stop {
                HardStop::RiskLevel(rl) if action.risk == *rl => {
                    return Decision::Deny;
                }
                _ => {}
            }
        }
        // Delegate to approval policy
        self.approve.decide(action)
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
}
