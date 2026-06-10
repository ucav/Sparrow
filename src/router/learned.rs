//! Per-repo routing memory — local-first learning of which task tiers
//! actually succeed in THIS repository.
//!
//! Every verified run records its outcome per tier in
//! `.sparrow/routing_memory.json` under the workspace root. When a tier keeps
//! failing or escalating here, the engine starts the next run one tier higher
//! — the router learns the repo without any telemetry leaving the machine.
//!
//! Only verification-backed outcomes are recorded: a run that "completed"
//! without a verify command proves nothing and would pollute the data.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::router::TaskTier;

/// Outcome of a run, as far as routing quality is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunRoutingOutcome {
    /// The run finished and the verify command passed without escalation.
    VerifiedSuccess,
    /// The run finished but only after escalating to a stronger model.
    Escalated,
    /// The run failed (verification never passed, or the chain errored out).
    Failed,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TierStats {
    #[serde(default)]
    pub verified_success: u32,
    #[serde(default)]
    pub escalated: u32,
    #[serde(default)]
    pub failed: u32,
}

impl TierStats {
    fn samples(&self) -> u32 {
        self.verified_success + self.escalated + self.failed
    }

    /// Halve all counters so recent runs dominate — the repo (and the
    /// available models) change over time.
    fn decay(&mut self) {
        self.verified_success /= 2;
        self.escalated /= 2;
        self.failed /= 2;
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepoRoutingMemory {
    #[serde(default)]
    pub tiers: HashMap<String, TierStats>,
    #[serde(skip)]
    path: Option<PathBuf>,
}

/// Minimum verified samples for a tier before its stats may influence routing.
const MIN_SAMPLES: u32 = 4;
/// Share of (escalated + failed) runs at which the starting tier gets bumped.
const BUMP_THRESHOLD: f64 = 0.5;
/// Counters decay once a tier accumulates this many samples.
const DECAY_AT: u32 = 50;

impl RepoRoutingMemory {
    fn file_path(workspace_root: &Path) -> PathBuf {
        workspace_root.join(".sparrow").join("routing_memory.json")
    }

    /// Load the repo's routing memory; a missing or corrupt file is an empty
    /// memory, never an error.
    pub fn load(workspace_root: &Path) -> Self {
        let path = Self::file_path(workspace_root);
        let mut mem: RepoRoutingMemory = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        mem.path = Some(path);
        mem
    }

    /// Record a run outcome for `tier` and persist (best-effort).
    pub fn record(&mut self, tier: &TaskTier, outcome: RunRoutingOutcome) {
        let stats = self.tiers.entry(tier.as_str().to_string()).or_default();
        match outcome {
            RunRoutingOutcome::VerifiedSuccess => stats.verified_success += 1,
            RunRoutingOutcome::Escalated => stats.escalated += 1,
            RunRoutingOutcome::Failed => stats.failed += 1,
        }
        if stats.samples() >= DECAY_AT {
            stats.decay();
        }
        self.save();
    }

    fn save(&self) {
        let Some(path) = &self.path else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }

    /// If this repo's history says `tier` mostly fails or escalates, return
    /// the tier the run should START at instead.
    pub fn suggest_bump(&self, tier: &TaskTier) -> Option<TaskTier> {
        let stats = self.tiers.get(tier.as_str())?;
        if stats.samples() < MIN_SAMPLES {
            return None;
        }
        let bad = (stats.escalated + stats.failed) as f64;
        if bad / stats.samples() as f64 >= BUMP_THRESHOLD {
            next_tier_up(tier)
        } else {
            None
        }
    }
}

fn next_tier_up(tier: &TaskTier) -> Option<TaskTier> {
    match tier {
        TaskTier::Trivial => Some(TaskTier::Small),
        TaskTier::Small => Some(TaskTier::Medium),
        TaskTier::Medium => Some(TaskTier::Hard),
        // Nothing above Hard; Vision is orthogonal, not a strength tier.
        TaskTier::Hard | TaskTier::Vision => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> RepoRoutingMemory {
        RepoRoutingMemory::default()
    }

    #[test]
    fn no_bump_without_enough_samples() {
        let mut m = mem();
        for _ in 0..3 {
            m.tiers.entry("small".into()).or_default().failed += 1;
        }
        assert_eq!(m.suggest_bump(&TaskTier::Small), None);
    }

    #[test]
    fn bump_when_majority_fails() {
        let mut m = mem();
        let s = m.tiers.entry("small".into()).or_default();
        s.failed = 2;
        s.escalated = 1;
        s.verified_success = 1;
        assert_eq!(m.suggest_bump(&TaskTier::Small), Some(TaskTier::Medium));
    }

    #[test]
    fn no_bump_when_mostly_verified() {
        let mut m = mem();
        let s = m.tiers.entry("medium".into()).or_default();
        s.verified_success = 5;
        s.failed = 1;
        assert_eq!(m.suggest_bump(&TaskTier::Medium), None);
    }

    #[test]
    fn hard_has_no_higher_tier() {
        let mut m = mem();
        let s = m.tiers.entry("hard".into()).or_default();
        s.failed = 10;
        assert_eq!(m.suggest_bump(&TaskTier::Hard), None);
    }

    #[test]
    fn decay_halves_counters() {
        let mut s = TierStats {
            verified_success: 30,
            escalated: 10,
            failed: 10,
        };
        s.decay();
        assert_eq!(s.verified_success, 15);
        assert_eq!(s.samples(), 25);
    }

    #[test]
    fn load_missing_file_is_empty() {
        let dir = std::env::temp_dir().join("sparrow-test-no-such-dir-xyz");
        let m = RepoRoutingMemory::load(&dir);
        assert!(m.tiers.is_empty());
    }

    #[test]
    fn record_and_reload_roundtrip() {
        let dir = std::env::temp_dir().join(format!("sparrow-rm-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mut m = RepoRoutingMemory::load(&dir);
        m.record(&TaskTier::Medium, RunRoutingOutcome::Escalated);
        m.record(&TaskTier::Medium, RunRoutingOutcome::VerifiedSuccess);
        let reloaded = RepoRoutingMemory::load(&dir);
        let stats = reloaded.tiers.get("medium").expect("medium stats");
        assert_eq!(stats.escalated, 1);
        assert_eq!(stats.verified_success, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
