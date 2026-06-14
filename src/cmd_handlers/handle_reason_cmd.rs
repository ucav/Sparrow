// src/cmd_handlers/handle_reason_cmd.rs
//
// `sparrow reason "<task>"` — run a reasoning-heavy task through the
// inference-time-scaling pipeline (best-of-N + judge selection + Reflexion
// self-refine) instead of a single greedy completion. This is the
// `src/reasoning/inference_scaling.rs` machinery wired to a live brain: it
// spends extra model calls to raise reliability, so a smaller routed model
// approaches frontier single-pass quality on verifiable reasoning.
//
// It deliberately does NOT use tools (it is a pure reasoning amplifier); for
// tool-using work use `sparrow run` / the swarm.
use super::prelude::*;

pub async fn handle_reason(
    config: &sparrow::config::Config,
    memory: Arc<dyn Memory>,
    task: &str,
) -> anyhow::Result<()> {
    use sparrow::reasoning::{ReasoningBudget, reason_max};
    use sparrow::router::{BasicRouter, BudgetState, Router, RoutingNeed, TaskTier};

    let task = task.trim();
    if task.is_empty() {
        anyhow::bail!("usage: sparrow reason \"<task>\"");
    }

    let providers = build_provider_brains(config, &memory, true);
    let router = BasicRouter::new(config, providers);

    // Reasoning tasks are treated at the Hard tier so they get the full
    // best-of-N + multi-round refine budget. No tools, no vision.
    let need = RoutingNeed {
        tier: TaskTier::Hard,
        required_tools: false,
        required_vision: false,
        prefer_local: false,
    };
    let budget_state = BudgetState {
        daily_limit_usd: config.budget.daily_usd,
        daily_spent_usd: 0.0,
        session_limit_usd: config.budget.session_usd,
        session_spent_usd: 0.0,
    };

    let chain = router.select(&need, &budget_state);
    let Some(brain) = chain.first() else {
        anyhow::bail!(
            "No model available for reasoning. Configure a provider with `sparrow setup`, \
             set an API key, or run a local Ollama (`ollama serve`)."
        );
    };

    let rmax = ReasoningBudget::for_tier(&TaskTier::Hard);
    let system = "You are a rigorous reasoning engine. Solve the task completely and \
correctly. State your conclusion first, then the load-bearing justification. \
Separate confirmed facts from assumptions; never invent a result.";

    eprintln!(
        "🧠 reasoning-max on {} — best-of-{} + {} refine round(s) (~{} model calls)…",
        brain.id(),
        rmax.samples,
        rmax.refine_rounds,
        rmax.max_calls()
    );

    match reason_max(brain.as_ref(), system, task, rmax).await {
        Some(answer) => {
            println!("{}", answer.trim());
            Ok(())
        }
        None => anyhow::bail!("reasoning failed: the model returned no usable output"),
    }
}
