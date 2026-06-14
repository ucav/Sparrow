//! Inference-time reasoning amplification (test-time compute).
//!
//! This is the machinery that lets a *smaller* model approach a frontier model's
//! single-pass quality on **verifiable** tasks: instead of trusting one greedy
//! sample, we spend extra model calls to search and verify. Three composable
//! primitives, all generic over the [`Brain`] trait:
//!
//! - [`best_of_n`] — sample N diverse drafts, then have a judge call pick the
//!   best (best-of-N / self-consistency by selection).
//! - [`self_refine_from`] — Reflexion loop: critique the answer with an
//!   adversarial reviewer call, then revise, until the reviewer approves or the
//!   round budget is spent.
//! - [`reason_max`] — the full pipeline: best-of-N draft → judge-select →
//!   iterative self-refine, with the compute [`ReasoningBudget`] scaled by task
//!   tier and (optionally) by how weak the underlying model is.
//!
//! **Honest boundary.** Test-time compute buys the most on tasks with a
//! verification signal (code that runs, math that checks, structured constraints
//! a reviewer can spot). It buys less on open-ended tasks with no ground truth.
//! The budget therefore scales with tier, not blindly.

use futures::StreamExt;

use crate::provider::{Brain, BrainEvent, BrainRequest, ContentBlock, Msg, PromptCacheConfig};
use crate::router::TaskTier;

const CRITIC_SYSTEM: &str = "You are a ruthless senior reviewer. Find the single most important flaw in the answer below: a wrong claim, a missing case, an unproven assertion, a violated constraint, or an unclear step. Be concrete and specific. If — and only if — the answer is genuinely correct, complete, and well-justified, reply with exactly the word APPROVED and nothing else.";

const JUDGE_SYSTEM: &str = "You are an impartial judge. You will see a task and several candidate answers. Pick the single best one: most correct, complete, and well-justified. Reply with ONLY its index number, nothing else.";

/// Compute budget for a reasoning-max run. Higher = more model calls = higher
/// reliability at higher cost/latency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReasoningBudget {
    /// Number of independent drafts to sample before selecting (best-of-N).
    pub samples: usize,
    /// Number of critique→revise rounds applied to the selected draft.
    pub refine_rounds: usize,
}

impl ReasoningBudget {
    /// Default budget for a task tier. Cheap tiers get a single greedy pass;
    /// hard tiers get search + iterative refinement.
    pub fn for_tier(tier: &TaskTier) -> Self {
        match tier {
            TaskTier::Trivial | TaskTier::Small => Self {
                samples: 1,
                refine_rounds: 0,
            },
            TaskTier::Medium | TaskTier::Vision => Self {
                samples: 1,
                refine_rounds: 1,
            },
            TaskTier::Hard => Self {
                samples: 3,
                refine_rounds: 2,
            },
        }
    }

    /// Weaker models benefit MORE from test-time compute — they have more
    /// headroom to recover from a bad first sample. Spend more on them. `band`
    /// is the model capability band (1 = small … 5 = frontier); see
    /// `model-capability-profile.json`.
    pub fn scaled_for_capability(self, band: u8) -> Self {
        match band {
            0..=2 => Self {
                samples: (self.samples + 2).min(6),
                refine_rounds: (self.refine_rounds + 1).min(4),
            },
            3 => Self {
                samples: (self.samples + 1).min(6),
                refine_rounds: self.refine_rounds,
            },
            // Strong/frontier models need less external search to be reliable.
            _ => self,
        }
    }

    /// Rough upper bound on model calls this budget will issue, for cost gating.
    pub fn max_calls(&self) -> usize {
        let draft_calls = self.samples + if self.samples > 1 { 1 } else { 0 };
        draft_calls + self.refine_rounds * 2
    }
}

fn user_req(system: &str, user: &str, temperature: f32, max_tokens: u32) -> BrainRequest {
    BrainRequest {
        system: Some(system.to_string()),
        messages: vec![Msg {
            role: "user".into(),
            content: vec![ContentBlock::Text {
                text: user.to_string(),
            }],
        }],
        tools: vec![],
        max_tokens,
        temperature,
        stop: vec![],
        cache: PromptCacheConfig::disabled(),
    }
}

/// Drain a completion stream into a single string. `None` on stream error.
pub async fn collect_text(brain: &dyn Brain, req: BrainRequest) -> Option<String> {
    let mut stream = brain.complete(req).await.ok()?;
    let mut out = String::new();
    while let Some(ev) = stream.next().await {
        match ev {
            BrainEvent::TextDelta(t) => out.push_str(&t),
            BrainEvent::Done(_) => break,
            BrainEvent::Error(_) => return None,
            _ => {}
        }
    }
    Some(out)
}

fn is_approved(critique: &str) -> bool {
    let t = critique.trim().to_ascii_uppercase();
    t == "APPROVED" || t.starts_with("APPROVED")
}

/// Parse the first integer in `s` that is a valid index `< n`; fall back to 0.
fn parse_index(s: &str, n: usize) -> usize {
    let digits: String = s
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse::<usize>().ok().filter(|&i| i < n).unwrap_or(0)
}

/// Have the model pick the best of several candidate answers (the verifier of
/// best-of-N). Returns the chosen index. Single-candidate is index 0.
pub async fn select_best(brain: &dyn Brain, task: &str, candidates: &[String]) -> usize {
    if candidates.len() <= 1 {
        return 0;
    }
    let mut listing = String::new();
    for (i, c) in candidates.iter().enumerate() {
        listing.push_str(&format!("[{i}]\n{}\n---\n", c.trim()));
    }
    let prompt = format!(
        "Task:\n{task}\n\nCandidate answers:\n{listing}\nReply with ONLY the index number of the single best candidate."
    );
    match collect_text(brain, user_req(JUDGE_SYSTEM, &prompt, 0.0, 8)).await {
        Some(out) => parse_index(&out, candidates.len()),
        None => 0,
    }
}

/// Sample `n` drafts (greedy if n=1, else temperature-diversified) and return
/// the judge-selected best. `None` only if every draft failed.
pub async fn best_of_n(brain: &dyn Brain, system: &str, task: &str, n: usize) -> Option<String> {
    let n = n.max(1);
    let mut candidates = Vec::with_capacity(n);
    for i in 0..n {
        let temperature = if n == 1 { 0.0 } else { 0.2 + 0.2 * i as f32 };
        if let Some(c) = collect_text(brain, user_req(system, task, temperature, 2048)).await {
            if !c.trim().is_empty() {
                candidates.push(c);
            }
        }
    }
    match candidates.len() {
        0 => None,
        1 => candidates.pop(),
        _ => {
            let idx = select_best(brain, task, &candidates).await;
            candidates.into_iter().nth(idx)
        }
    }
}

/// Reflexion loop: critique `answer` with an adversarial reviewer, then revise,
/// up to `rounds` times. Stops early when the reviewer replies APPROVED.
pub async fn self_refine_from(
    brain: &dyn Brain,
    system: &str,
    task: &str,
    mut answer: String,
    rounds: usize,
) -> Option<String> {
    for _ in 0..rounds {
        let critique_prompt =
            format!("Task:\n{task}\n\nAnswer to review:\n{answer}\n\nYour critique:");
        let critique =
            collect_text(brain, user_req(CRITIC_SYSTEM, &critique_prompt, 0.0, 512)).await?;
        if is_approved(&critique) {
            break;
        }
        let revise_prompt = format!(
            "Task:\n{task}\n\nYour previous answer:\n{answer}\n\nA reviewer raised this issue:\n{critique}\n\nProduce an improved answer that fully fixes it. Output only the improved answer.",
        );
        answer = collect_text(brain, user_req(system, &revise_prompt, 0.2, 2048)).await?;
    }
    Some(answer)
}

/// Full reasoning-max pipeline for a single answer: best-of-N draft → select →
/// iterative self-refine, per `budget`. `None` only if drafting failed entirely.
pub async fn reason_max(
    brain: &dyn Brain,
    system: &str,
    task: &str,
    budget: ReasoningBudget,
) -> Option<String> {
    let draft = if budget.samples > 1 {
        best_of_n(brain, system, task, budget.samples).await?
    } else {
        collect_text(brain, user_req(system, task, 0.2, 2048)).await?
    };
    if budget.refine_rounds == 0 {
        return Some(draft);
    }
    self_refine_from(brain, system, task, draft, budget.refine_rounds).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::StopReason;
    use crate::provider::{BrainStream, ModelCaps};
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A Brain that returns scripted responses in order, counting calls — lets us
    /// assert the orchestration's control flow deterministically without a model.
    struct ScriptBrain {
        responses: Mutex<VecDeque<String>>,
        calls: AtomicUsize,
    }
    impl ScriptBrain {
        fn new(responses: &[&str]) -> Self {
            Self {
                responses: Mutex::new(responses.iter().map(|s| s.to_string()).collect()),
                calls: AtomicUsize::new(0),
            }
        }
        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }
    #[async_trait::async_trait]
    impl Brain for ScriptBrain {
        fn id(&self) -> &str {
            "mock:script"
        }
        fn caps(&self) -> ModelCaps {
            ModelCaps::default()
        }
        async fn complete(&self, _req: BrainRequest) -> anyhow::Result<BrainStream> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let resp = self
                .responses
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| "DEFAULT".into());
            let events = vec![
                BrainEvent::TextDelta(resp),
                BrainEvent::Done(StopReason::EndTurn),
            ];
            Ok(Box::pin(futures::stream::iter(events)))
        }
    }

    #[test]
    fn budget_scales_with_tier_and_capability() {
        assert_eq!(
            ReasoningBudget::for_tier(&TaskTier::Trivial),
            ReasoningBudget {
                samples: 1,
                refine_rounds: 0
            }
        );
        let hard = ReasoningBudget::for_tier(&TaskTier::Hard);
        assert_eq!(
            hard,
            ReasoningBudget {
                samples: 3,
                refine_rounds: 2
            }
        );
        // A weak model gets MORE compute; a frontier model is left as-is.
        assert!(hard.scaled_for_capability(1).max_calls() > hard.max_calls());
        assert_eq!(hard.scaled_for_capability(5), hard);
    }

    #[test]
    fn parse_index_is_robust() {
        assert_eq!(parse_index("2", 3), 2);
        assert_eq!(parse_index("The best is [1].", 3), 1);
        assert_eq!(parse_index("99", 3), 0); // out of range → 0
        assert_eq!(parse_index("none", 3), 0);
    }

    #[tokio::test]
    async fn self_refine_revises_then_stops_on_approval() {
        // critique(issue) → revise → critique(APPROVED) → stop
        let b = ScriptBrain::new(&["ISSUE: missing empty-input case", "answer v2", "APPROVED"]);
        let out = self_refine_from(&b, "sys", "task", "answer v1".into(), 3)
            .await
            .unwrap();
        assert_eq!(out, "answer v2");
        assert_eq!(b.calls(), 3, "critique + revise + critique");
    }

    #[tokio::test]
    async fn self_refine_stops_immediately_when_approved() {
        let b = ScriptBrain::new(&["APPROVED"]);
        let out = self_refine_from(&b, "sys", "task", "good answer".into(), 3)
            .await
            .unwrap();
        assert_eq!(out, "good answer", "unchanged");
        assert_eq!(b.calls(), 1, "only the critique");
    }

    #[tokio::test]
    async fn best_of_n_samples_then_judges() {
        // 3 drafts, judge picks index 1
        let b = ScriptBrain::new(&["cand A", "cand B", "cand C", "1"]);
        let out = best_of_n(&b, "sys", "task", 3).await.unwrap();
        assert_eq!(out, "cand B");
        assert_eq!(b.calls(), 4, "3 drafts + 1 judge");
    }

    #[tokio::test]
    async fn best_of_one_skips_the_judge() {
        let b = ScriptBrain::new(&["only candidate"]);
        let out = best_of_n(&b, "sys", "task", 1).await.unwrap();
        assert_eq!(out, "only candidate");
        assert_eq!(b.calls(), 1, "no judge call for n=1");
    }

    #[tokio::test]
    async fn reason_max_runs_the_full_pipeline() {
        // budget: 2 samples + 1 refine round
        // d0, d1, judge→"1" (picks d1), critique(issue), revised
        let b = ScriptBrain::new(&["d0", "d1", "1", "ISSUE: x", "refined answer"]);
        let budget = ReasoningBudget {
            samples: 2,
            refine_rounds: 1,
        };
        let out = reason_max(&b, "sys", "task", budget).await.unwrap();
        assert_eq!(out, "refined answer");
        assert_eq!(b.calls(), 5, "2 drafts + judge + critique + revise");
    }
}
