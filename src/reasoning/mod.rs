use crate::provider::{ContentBlock, Msg};

// ─── Reasoning depth ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ReasoningDepth {
    Fast,
    Adaptive,
    Deep,
}

impl Default for ReasoningDepth {
    fn default() -> Self {
        ReasoningDepth::Adaptive
    }
}

// ─── Anti-simulation guard ─────────────────────────────────────────────────────

/// The anti-simulation guard checks every assistant turn for fabricated results.
/// Rule: an agent MUST NOT claim a result without real tool execution.
/// If the assistant asserts a result (pass/fail/output = X/returned Y) without
/// a matching ToolOutput in the same turn, the engine rejects and asks for real execution.
pub struct AntiSimulationGuard;

impl AntiSimulationGuard {
    /// Keywords that suggest a fabricated result claim
    const CLAIM_KEYWORDS: &'static [&'static str] = &[
        "all tests pass", "tests pass", "build succeeds", "build successful",
        "output:", "returns:", "returned:", "the result is", "got:",
        "passed", "succeeded", "compiled", "ran successfully",
        "no errors", "0 errors", "zero failures", "green",
    ];

    /// Check if an assistant message contains a result claim without having tool output
    pub fn check(turn_messages: &[Msg], tool_outputs_in_turn: bool) -> Option<String> {
        if tool_outputs_in_turn {
            return None; // Results are backed by real tool execution
        }

        for msg in turn_messages.iter().filter(|m| m.role == "assistant") {
            for block in &msg.content {
                if let ContentBlock::Text { text } = block {
                    let lower = text.to_lowercase();
                    for kw in Self::CLAIM_KEYWORDS {
                        if lower.contains(kw) {
                            return Some(format!(
                                "anti-simulation: assistant claimed result (matched '{}') without executing a tool. Execute the tool first, then report the RAW output.",
                                kw
                            ));
                        }
                    }
                }
            }
        }
        None
    }

    /// Check if an assertion about code was made without reading the file first
    pub fn check_code_claim(text: &str, had_fs_read: bool, had_search: bool) -> Option<String> {
        if had_fs_read || had_search {
            return None;
        }
        let lower = text.to_lowercase();
        let code_claims = ["function", "struct", "impl", "trait", "fn ", "pub fn", "mod ", "class "];
        let assertion_patterns = ["exists", "is defined", "takes", "returns", "has a", "contains"];

        let has_code_ref = code_claims.iter().any(|c| lower.contains(c));
        let has_assertion = assertion_patterns.iter().any(|a| lower.contains(a));

        if has_code_ref && has_assertion {
            return Some(
                "hallucination-guard: you made an assertion about the code without reading it first. Use fs_read or search to verify before claiming.".into()
            );
        }
        None
    }
}

// ─── Hallucination guard ───────────────────────────────────────────────────────

pub struct HallucinationGuard;

impl HallucinationGuard {
    /// Before claiming a fact about code, verify via real read/search.
    /// Returns a corrective message if the assistant made unverified claims.
    pub fn verify(text: &str, tools_called: &[String]) -> Option<String> {
        let has_read = tools_called.iter().any(|t| t == "fs_read" || t == "search");
        AntiSimulationGuard::check_code_claim(text, has_read, has_read)
    }
}

// ─── Self-critique ─────────────────────────────────────────────────────────────

pub struct SelfCritique;

impl SelfCritique {
    /// Before a mutating batch, self-review the changes against constraints
    pub fn pre_mutation_review(
        diffs: &[crate::event::FileDiff],
        spec: Option<&str>,
    ) -> String {
        let mut review = String::from("## Self-critique (pre-mutation review)\n\n");

        if diffs.is_empty() {
            review.push_str("No diffs to review.\n");
            return review;
        }

        review.push_str(&format!("Files to change: {}\n", diffs.len()));
        for d in diffs {
            review.push_str(&format!(
                "- {} (+{} -{})\n",
                d.file, d.plus, d.minus
            ));
        }

        review.push_str("\n### Checklist\n");
        review.push_str("- [ ] Each change addresses the spec/requirement\n");
        review.push_str("- [ ] No unnecessary formatting changes\n");
        review.push_str("- [ ] Tests still pass after changes\n");
        review.push_str("- [ ] No secrets or credentials exposed\n");
        review.push_str("- [ ] Edge cases considered\n");

        if let Some(s) = spec {
            review.push_str(&format!("\nRefer to spec: {}\n", s));
        }

        review
    }
}

// ─── Uncertainty ───────────────────────────────────────────────────────────────

pub struct UncertaintyEstimator;

impl UncertaintyEstimator {
    /// Estimate confidence based on whether the agent has read relevant files
    pub fn confidence(has_read_files: bool, has_run_tests: bool, task_complexity: &str) -> f64 {
        let mut confidence: f64 = 0.3;

        if has_read_files {
            confidence += 0.3;
        }
        if has_run_tests {
            confidence += 0.3;
        }

        match task_complexity {
            "trivial" => confidence += 0.1,
            "small" => confidence += 0.05,
            "medium" => confidence += 0.0,
            "hard" => confidence -= 0.1,
            "vision" => confidence -= 0.05,
            _ => {}
        }

        confidence.max(0.0).min(1.0)
    }

    /// Generate uncertainty message
    pub fn uncertain_message() -> &'static str {
        "I'm not sure about this — let me verify before proceeding."
    }
}

// ─── Stop and ask ──────────────────────────────────────────────────────────────

pub struct StopAndAsk;

impl StopAndAsk {
    /// Determine if the current situation warrants asking the user
    pub fn should_ask(
        ambiguity_level: f64,
        is_irreversible: bool,
        autonomy_allows: bool,
        constraints_missing: bool,
    ) -> Option<String> {
        if ambiguity_level > 0.7 && !autonomy_allows {
            return Some(
                "High ambiguity detected. Could you clarify:\n- What exact behavior do you expect?\n- Are there specific constraints I should know?".into()
            );
        }

        if is_irreversible && !autonomy_allows {
            return Some(
                "This action is irreversible. Before proceeding, please confirm:\n- Is this the expected outcome?\n- Are there any backups or safeguards in place?".into()
            );
        }

        if constraints_missing {
            return Some(
                "Missing constraints. Could you specify:\n- Target environment/OS?\n- Performance requirements?\n- Compatibility constraints?".into()
            );
        }

        None
    }

    /// Ask exactly ONE targeted question (never more than one)
    pub fn ask_single(question: &str) -> String {
        format!("❓ {}", question)
    }
}

// ─── Reasoning engine (wired into main engine loop) ────────────────────────────

pub struct ReasoningEngine {
    pub depth: ReasoningDepth,
    pub anti_simulation: bool,
    pub hallucination_guard: bool,
    pub self_critique: bool,
}

impl Default for ReasoningEngine {
    fn default() -> Self {
        Self {
            depth: ReasoningDepth::Adaptive,
            anti_simulation: true,
            hallucination_guard: true,
            self_critique: true,
        }
    }
}

impl ReasoningEngine {
    /// Decide planning depth based on task complexity
    pub fn plan_depth(&self, task: &str) -> usize {
        let tier = crate::router::TaskTier::from_str(
            if task.len() > 100 { "hard" } else if task.len() > 30 { "medium" } else { "small" }
        );
        match self.depth {
            ReasoningDepth::Fast => 1,
            ReasoningDepth::Deep => 5,
            ReasoningDepth::Adaptive => match tier {
                crate::router::TaskTier::Trivial => 1,
                crate::router::TaskTier::Small => 2,
                crate::router::TaskTier::Medium => 3,
                crate::router::TaskTier::Hard => 4,
                crate::router::TaskTier::Vision => 3,
            },
        }
    }

    /// Run the anti-simulation guard on an assistant turn
    pub fn guard_turn(
        &self,
        messages: &[Msg],
        tool_outputs_in_turn: bool,
    ) -> Option<String> {
        if self.anti_simulation {
            AntiSimulationGuard::check(messages, tool_outputs_in_turn)
        } else {
            None
        }
    }
}

// ─── Event extensions for reasoning ────────────────────────────────────────────

/// Reasoning events emitted during the think phase
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ReasoningEvent {
    /// Agent is planning sub-steps before acting
    Planning { steps: Vec<String> },
    /// Agent is self-critiquing before a mutation
    SelfCritique { review: String },
    /// Agent is uncertain and verifying
    UncertaintyCheck { message: String },
    /// Agent is asking the user a question
    AskUser { question: String },
    /// Anti-simulation guard rejected a turn
    FabricationRejected { reason: String },
    /// Hallucination guard caught an unverified claim
    ClaimRejected { reason: String },
}
