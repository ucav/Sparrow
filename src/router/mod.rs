use std::sync::Arc;

use crate::provider::{Brain, BrainError, BrainRequest, ContentBlock, LatencyClass, Msg};

// ─── Routing need ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum TaskTier {
    Trivial,
    Small,
    Medium,
    Hard,
    Vision,
}

impl TaskTier {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "trivial" => TaskTier::Trivial,
            "small" => TaskTier::Small,
            "medium" => TaskTier::Medium,
            "hard" => TaskTier::Hard,
            "vision" => TaskTier::Vision,
            _ => TaskTier::Medium,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            TaskTier::Trivial => "trivial",
            TaskTier::Small => "small",
            TaskTier::Medium => "medium",
            TaskTier::Hard => "hard",
            TaskTier::Vision => "vision",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RoutingNeed {
    pub tier: TaskTier,
    pub required_tools: bool,
    pub required_vision: bool,
    pub prefer_local: bool,
}

#[derive(Debug, Clone)]
pub struct BudgetState {
    pub daily_limit_usd: f64,
    pub daily_spent_usd: f64,
    pub session_limit_usd: f64,
    pub session_spent_usd: f64,
}

impl BudgetState {
    pub fn remaining_daily(&self) -> f64 {
        (self.daily_limit_usd - self.daily_spent_usd).max(0.0)
    }

    pub fn remaining_session(&self) -> f64 {
        (self.session_limit_usd - self.session_spent_usd).max(0.0)
    }

    pub fn is_exhausted(&self) -> bool {
        self.remaining_daily() <= 0.0 || self.remaining_session() <= 0.0
    }
}

// ─── Router trait ───────────────────────────────────────────────────────────────

pub trait Router: Send + Sync {
    /// Returns an ordered fallback chain of Brains.
    /// Primary brain first, fallbacks in order.
    fn select(&self, need: &RoutingNeed, budget: &BudgetState) -> Vec<Arc<dyn Brain>>;

    fn on_error(&self, b: &dyn Brain, e: &BrainError) -> Retry;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Retry {
    NextInChain,
    Abort,
    WaitAndRetry(u64), // seconds
}

// ─── Basic Router implementation ────────────────────────────────────────────────

use std::collections::HashMap;

use crate::config::Config;

pub struct BasicRouter {
    /// provider_name -> list of (model_name, Brain)
    providers: HashMap<String, Vec<Arc<dyn Brain>>>,
    /// task tier -> preferred provider name
    policy: HashMap<String, String>,
    free_first: bool,
}

impl BasicRouter {
    pub fn new(config: &Config, providers: HashMap<String, Vec<Arc<dyn Brain>>>) -> Self {
        let mut policy = HashMap::new();
        for (k, v) in &config.routing.policy {
            policy.insert(k.clone(), v.clone());
        }
        // Defaults
        if !policy.contains_key("trivial") {
            policy.insert("trivial".into(), "local".into());
        }
        if !policy.contains_key("hard") {
            policy.insert("hard".into(), "anthropic".into());
        }

        Self {
            providers,
            policy,
            free_first: config.routing.free_first,
        }
    }

    /// Score a brain for a given need: higher is better.
    fn score(brain: &dyn Brain, need: &RoutingNeed, budget: &BudgetState) -> f64 {
        let caps = brain.caps();
        let mut score: f64 = 0.0;

        // Capability fit
        if need.required_tools && caps.tools {
            score += 50.0;
        }
        if need.required_vision && caps.vision {
            score += 50.0;
        }

        // Cost preference: prefer cheaper/free models
        let est_cost = caps.cost_input_per_mtok + caps.cost_output_per_mtok;
        if est_cost == 0.0 {
            score += 100.0; // free models get a big boost
        } else if budget.remaining_session() < est_cost * 0.1 {
            score -= 200.0; // too expensive for remaining budget
        } else {
            score -= est_cost * 10.0; // penalize expensive models
        }

        // Latency preference
        match caps.latency {
            LatencyClass::Fast => score += 10.0,
            LatencyClass::Medium => score += 5.0,
            LatencyClass::Slow => score += 0.0,
        }

        // Context window fit (larger is better)
        score += (caps.context_window as f64 / 10_000.0).min(10.0);

        score
    }

    fn resolve_provider(&self, need: &RoutingNeed) -> &str {
        self.policy
            .get(need.tier.as_str())
            .map(|s| s.as_str())
            .unwrap_or("anthropic")
    }

    /// Classify a task using a tiny model call (only for ambiguous cases).
    /// §3.6: "Classification: heuristic + a tiny model call only if ambiguous."
    pub async fn classify_with_model(
        &self,
        task: &str,
        brain: &dyn Brain,
    ) -> TaskTier {
        let prompt = format!(
            "Classify this task into exactly one tier: trivial, small, medium, hard, vision.\n\nTask: {}\n\nTier:",
            task
        );

        let req = BrainRequest {
            system: Some("You are a task classifier. Output exactly one word: trivial, small, medium, hard, or vision.".into()),
            messages: vec![Msg {
                role: "user".into(),
                content: vec![ContentBlock::Text { text: prompt }],
            }],
            tools: vec![],
            max_tokens: 10,
            temperature: 0.0,
            stop: vec![],
        };

        match brain.complete(req).await {
            Ok(mut stream) => {
                use futures::StreamExt;
                let mut result = String::new();
                while let Some(ev) = stream.next().await {
                    if let crate::provider::BrainEvent::TextDelta(t) = ev {
                        result.push_str(&t);
                    }
                }
                TaskTier::from_str(result.trim())
            }
            Err(_) => TaskTier::Medium, // fallback
        }
    }
}

impl Router for BasicRouter {
    fn select(&self, need: &RoutingNeed, budget: &BudgetState) -> Vec<Arc<dyn Brain>> {
        if budget.is_exhausted() && !need.prefer_local {
            // Only free/local models remain
            if let Some(local) = self.providers.get("local") {
                return local.clone();
            }
            return vec![];
        }

        let preferred_provider = self.resolve_provider(need);
        let mut scored: Vec<(f64, String, Arc<dyn Brain>)> = Vec::new();

        // Score all available brains
        for (provider_name, brains) in &self.providers {
            if need.prefer_local && provider_name != "local" && provider_name != "ollama" {
                continue;
            }
            for brain in brains {
                let mut s = Self::score(brain.as_ref(), need, budget);
                if provider_name == preferred_provider {
                    s += 25.0;
                }
                if matches!(need.tier, TaskTier::Trivial | TaskTier::Small)
                    && (provider_name == "local" || provider_name == "ollama")
                {
                    s += 30.0;
                }
                scored.push((s, provider_name.clone(), brain.clone()));
            }
        }

        // Sort by score descending
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let mut result: Vec<Arc<dyn Brain>> = Vec::new();
        for (_, _, brain) in &scored {
            result.push(brain.clone());
        }

        if matches!(need.tier, TaskTier::Trivial | TaskTier::Small) {
            if let Some((pos, _)) = scored
                .iter()
                .enumerate()
                .find(|(_, (_, provider_name, _))| {
                    provider_name == "local" || provider_name == "ollama"
                })
            {
                let local_brain = result.remove(pos);
                result.insert(0, local_brain);
            }
        }

        // If free_first and there's a free model, push it first for small tasks.
        if self.free_first {
            if let Some(pos) = result.iter().position(|b| {
                b.caps().cost_input_per_mtok == 0.0
                    && matches!(need.tier, TaskTier::Trivial | TaskTier::Small)
            }) {
                let free_brain = result.remove(pos);
                result.insert(0, free_brain);
            }
        }

        result
    }

    fn on_error(&self, _b: &dyn Brain, e: &BrainError) -> Retry {
        match e {
            BrainError::RateLimit { retry_after } => {
                if let Some(secs) = retry_after {
                    if *secs <= 10 {
                        Retry::WaitAndRetry(*secs)
                    } else {
                        Retry::NextInChain
                    }
                } else {
                    Retry::NextInChain
                }
            }
            BrainError::ServerError { status, .. } if *status >= 500 => Retry::NextInChain,
            BrainError::Timeout => Retry::NextInChain,
            BrainError::Refusal(_) => Retry::Abort,
            _ => Retry::NextInChain,
        }
    }
}
