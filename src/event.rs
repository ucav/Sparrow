use serde::{Deserialize, Serialize};

// ─── Core identifiers ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RunId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CheckpointId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub String);

impl RunId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl CheckpointId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl AgentId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

// ─── Content blocks ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Block {
    Text(String),
    Json(serde_json::Value),
    Image { data: Vec<u8>, mime: String },
    Diff { file: String, patch: String },
}

// ─── Tool use types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RiskLevel {
    ReadOnly,
    Mutating,
    Exec,
    Destructive,
    Network,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Decision {
    Allow,
    AskUser,
    Deny,
}

// ─── Agent status ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentStatus {
    Idle,
    Thinking,
    Working,
    WaitingForApproval,
    Done,
    Error,
}

// ─── Model-related types ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    StopSequence(String),
    ToolUse,
    Refusal,
    Error,
}

// ─── Autonomy ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AutonomyLevel {
    Supervised,
    Trusted,
    Autonomous,
}

impl AutonomyLevel {
    pub fn as_float(&self) -> f64 {
        match self {
            AutonomyLevel::Supervised => 0.0,
            AutonomyLevel::Trusted => 0.5,
            AutonomyLevel::Autonomous => 1.0,
        }
    }

    pub fn from_float(f: f64) -> Self {
        if f >= 0.75 {
            AutonomyLevel::Autonomous
        } else if f >= 0.25 {
            AutonomyLevel::Trusted
        } else {
            AutonomyLevel::Supervised
        }
    }
}

// ─── Outcome ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeSummary {
    pub status: String,
    pub diffs: Vec<FileDiff>,
    pub cost_usd: f64,
    pub tokens: TokenUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    pub file: String,
    pub plus: u32,
    pub minus: u32,
}

// ─── THE EVENT MODEL (§3.14) — load-bearing contract ────────────────────────────

/// Every surface renders from this stream; replay records from it.
/// This enum is the contract that connects runtime ↔ surfaces ↔ replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Event {
    RunStarted {
        run: RunId,
        task: String,
        agent: String,
    },
    RouteSelected {
        run: RunId,
        chain: Vec<String>,
    },
    ModelSwitched {
        run: RunId,
        from: String,
        to: String,
        reason: String,
    },
    ThinkingDelta {
        run: RunId,
        text: String,
    },
    Message {
        run: RunId,
        role: String,
        text: String,
    },
    ToolUseProposed {
        run: RunId,
        id: String,
        name: String,
        args: serde_json::Value,
        risk: RiskLevel,
    },
    ApprovalRequested {
        run: RunId,
        id: String,
        summary: String,
    },
    ApprovalResolved {
        run: RunId,
        id: String,
        decision: Decision,
    },
    ToolUseStarted {
        run: RunId,
        id: String,
    },
    ToolOutput {
        run: RunId,
        id: String,
        blocks: Vec<Block>,
    },
    DiffProposed {
        run: RunId,
        file: String,
        patch: String,
        plus: u32,
        minus: u32,
    },
    DiffApplied {
        run: RunId,
        file: String,
    },
    TestResult {
        run: RunId,
        passed: u32,
        failed: u32,
        detail: String,
    },
    AgentSpawned {
        run: RunId,
        role: String,
        model: String,
    },
    AgentStatus {
        run: RunId,
        role: String,
        status: AgentStatus,
        note: String,
    },
    CheckpointCreated {
        run: RunId,
        id: CheckpointId,
        label: String,
    },
    SkillLearned {
        run: RunId,
        name: String,
    },
    CostUpdate {
        run: RunId,
        usd: f64,
    },
    TokenUsage {
        run: RunId,
        input: u64,
        output: u64,
    },
    TokenUsageEstimated {
        run: RunId,
        input: u64,
        output: u64,
        reason: String,
    },
    AutonomyChanged {
        run: RunId,
        level: AutonomyLevel,
    },
    RunFinished {
        run: RunId,
        outcome: OutcomeSummary,
    },
    Error {
        run: RunId,
        message: String,
    },
}
