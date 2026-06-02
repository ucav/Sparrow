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

pub fn friendly_model_switch_reason(reason: &str) -> String {
    let lower = reason.to_lowercase();
    if lower.contains("ollama api error 404") && lower.contains("model") {
        "modèle local indisponible".into()
    } else if lower.contains("ollama") {
        "provider local indisponible".into()
    } else {
        reason.to_string()
    }
}

pub fn is_local_model_unavailable(reason: &str) -> bool {
    matches!(
        friendly_model_switch_reason(reason).as_str(),
        "modèle local indisponible" | "provider local indisponible"
    )
}

/// Streaming filter that strips `<think>…</think>` reasoning blocks emitted by
/// reasoning models (minimax, deepseek-r1, qwq…) so surfaces show the answer,
/// not the chain-of-thought. Handles tags split across streamed deltas.
#[derive(Default)]
pub struct ThinkStripper {
    in_think: bool,
    pending: String,
    /// Content seen inside the current (unclosed) think block, kept so that if
    /// the block NEVER closes (model didn't emit </think>) we can recover it on
    /// flush rather than silently swallowing the whole answer (fail-open).
    think_buf: String,
}

impl ThinkStripper {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a streamed delta; returns the portion that should be displayed.
    pub fn feed(&mut self, delta: &str) -> String {
        const OPEN: &str = "<think>";
        const CLOSE: &str = "</think>";
        self.pending.push_str(delta);
        let mut out = String::new();
        loop {
            if !self.in_think {
                if let Some(i) = self.pending.find(OPEN) {
                    out.push_str(&self.pending[..i]);
                    self.pending.replace_range(..i + OPEN.len(), "");
                    self.in_think = true;
                    self.think_buf.clear();
                    continue;
                }
                let keep = dangling_prefix(&self.pending, OPEN);
                let emit_to = self.pending.len() - keep;
                out.push_str(&self.pending[..emit_to]);
                self.pending.replace_range(..emit_to, "");
                break;
            } else {
                if let Some(i) = self.pending.find(CLOSE) {
                    // Real closed think block → discard its content.
                    self.pending.replace_range(..i + CLOSE.len(), "");
                    self.in_think = false;
                    self.think_buf.clear();
                    continue;
                }
                let keep = dangling_prefix(&self.pending, CLOSE);
                let drop_to = self.pending.len() - keep;
                // Stash dropped think content for fail-open recovery.
                self.think_buf.push_str(&self.pending[..drop_to]);
                self.pending.replace_range(..drop_to, "");
                break;
            }
        }
        out
    }

    /// Flush remaining buffered text. If a think block was opened but never
    /// closed, recover its content (the model likely put the answer there).
    pub fn flush(&mut self) -> String {
        let mut rest = std::mem::take(&mut self.pending);
        if self.in_think {
            // Unclosed think → show what we stashed (fail-open).
            let recovered = std::mem::take(&mut self.think_buf);
            self.in_think = false;
            format!("{}{}", recovered, rest)
        } else {
            self.think_buf.clear();
            std::mem::take(&mut rest)
        }
    }
}

/// Length (bytes) of the trailing portion of `s` that is a prefix of `tag`,
/// so a tag split across deltas isn't emitted prematurely. ASCII tags only.
fn dangling_prefix(s: &str, tag: &str) -> usize {
    let max = tag.len().saturating_sub(1).min(s.len());
    for n in (1..=max).rev() {
        if s.is_char_boundary(s.len() - n) && s[s.len() - n..] == tag[..n] {
            return n;
        }
    }
    0
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
        context_window: u64,
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
    /// A compaction pass has just completed. Surfaces the before/after sizes
    /// (in chars) and the path of the handoff doc, if any.
    Compacted {
        run: RunId,
        before_chars: usize,
        after_chars: usize,
        handoff_path: Option<String>,
    },
}
