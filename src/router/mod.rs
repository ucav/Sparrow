pub mod learned;

use std::sync::Arc;

use crate::provider::{
    Brain, BrainError, BrainRequest, ContentBlock, LatencyClass, Msg, PromptCacheConfig,
};

// ─── Routing need ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
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

    /// Look up a brain by its model ID across all registered providers.
    /// Used by the WebView model-override path when the user picks a model
    /// that isn't in the natural routing chain.
    fn find_brain_by_id(&self, model_id: &str) -> Option<Arc<dyn Brain>>;
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
    /// Global preferred provider override — when Some, every tier resolves to
    /// this provider first (capability constraints still apply).
    preferred_provider: Option<String>,
    preferred_model: Option<String>,
    /// \"auto\" or \"manual\" — see RoutingConfig.routing_mode.
    routing_mode: String,
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
            preferred_provider: config.routing.preferred_provider.clone(),
            preferred_model: config.routing.preferred_model.clone(),
            routing_mode: config.routing.routing_mode.clone(),
        }
    }

    /// Score a brain for a given need: higher is better.
    fn score(brain: &dyn Brain, need: &RoutingNeed, budget: &BudgetState) -> f64 {
        let caps = brain.caps();
        let mut score: f64 = 0.0;

        // Capability fit
        if need.required_tools {
            if caps.tools {
                score += 50.0;
            } else {
                score -= 250.0;
            }
        }
        if need.required_vision {
            if caps.vision {
                score += 50.0;
            } else {
                score -= 300.0;
            }
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

        // Latency / capability preference — tier-aware.
        // Cheap tiers want speed; hard tiers want capable (bigger, slower) models.
        match need.tier {
            TaskTier::Trivial | TaskTier::Small => match caps.latency {
                LatencyClass::Fast => score += 15.0,
                LatencyClass::Medium => score += 6.0,
                LatencyClass::Slow => score += 0.0,
            },
            TaskTier::Medium | TaskTier::Hard | TaskTier::Vision => match caps.latency {
                LatencyClass::Slow => score += 18.0,
                LatencyClass::Medium => score += 9.0,
                LatencyClass::Fast => score += 0.0,
            },
        }

        // Context window fit. Weighted more heavily for capable tiers so larger
        // models (which we infer to have bigger windows) rise for hard tasks.
        let ctx_weight = match need.tier {
            TaskTier::Hard | TaskTier::Medium => 20_000.0,
            _ => 10_000.0,
        };
        let ctx_cap = match need.tier {
            TaskTier::Hard | TaskTier::Medium => 20.0,
            _ => 10.0,
        };
        score += (caps.context_window as f64 / ctx_weight).min(ctx_cap);

        score
    }

    fn resolve_provider(&self, need: &RoutingNeed) -> &str {
        // Manual mode: always use the user's chosen provider, no fallback ever.
        if self.routing_mode == "manual" {
            if let Some(ref pref) = self.preferred_provider {
                return pref.as_str();
            }
        }
        // If a global preferred provider is configured, use it for all tiers.
        // The scoring loop will still add a +25 bonus for this provider so it
        // bubbles to the top; capability fallback still kicks in if it lacks
        // tools/vision support.
        if let Some(ref pref) = self.preferred_provider {
            return pref.as_str();
        }
        self.policy
            .get(need.tier.as_str())
            .map(|s| s.as_str())
            .unwrap_or("anthropic")
    }

    /// Classify a task using a tiny model call (only for ambiguous cases).
    /// §3.6: "Classification: heuristic + a tiny model call only if ambiguous."
    pub async fn classify_with_model(&self, task: &str, brain: &dyn Brain) -> TaskTier {
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
            cache: PromptCacheConfig::disabled(),
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
        // Manual mode with a specific model: use EXACTLY that model, nothing else.
        if self.routing_mode == "manual" {
            if let Some(ref model) = self.preferred_model {
                for brains in self.providers.values() {
                    for brain in brains {
                        if brain.id() == *model {
                            return vec![brain.clone()];
                        }
                    }
                }
                // Model not found — return empty, let engine report the error
                return vec![];
            }
        }

        if budget.is_exhausted() && !need.prefer_local {
            // Only free/local models remain
            if let Some(local) = self.providers.get("local") {
                return local.clone();
            }
            return vec![];
        }

        let preferred_provider = self.resolve_provider(need);
        let preferred_is_local = preferred_provider == "local" || preferred_provider == "ollama";
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

        // Stable sort by score descending (preserves insertion order for ties,
        // so equal-scored discovered models keep a deterministic order).
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        // Build a PROVIDER-DIVERSE fallback chain. A single preferred provider
        // (e.g. opencode-zen) can have dozens of discovered models that would
        // otherwise fill every slot — leaving no fallback if that whole provider
        // is down (no credits / empty responses). Cap models per provider so
        // other providers (NVIDIA, ollama…) always get fallback slots.
        const MAX_CHAIN: usize = 6;
        const PER_PROVIDER_CAP: usize = 3;
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut per_provider: HashMap<String, usize> = HashMap::new();
        // (provider, brain) so we can reorder by provider below.
        let mut result: Vec<(String, Arc<dyn Brain>)> = Vec::new();

        // Pass 1: respect the per-provider cap → forces cross-provider diversity.
        for (_, prov, brain) in &scored {
            if result.len() >= MAX_CHAIN {
                break;
            }
            let id = brain.id().to_string();
            if seen.contains(&id) {
                continue;
            }
            let count = per_provider.entry(prov.clone()).or_insert(0);
            if *count >= PER_PROVIDER_CAP {
                continue;
            }
            *count += 1;
            seen.insert(id);
            result.push((prov.clone(), brain.clone()));
        }
        // Pass 2: if too few providers to fill the chain, top up over the cap.
        if result.len() < MAX_CHAIN {
            for (_, prov, brain) in &scored {
                if result.len() >= MAX_CHAIN {
                    break;
                }
                let id = brain.id().to_string();
                if seen.insert(id) {
                    result.push((prov.clone(), brain.clone()));
                }
            }
        }

        // For trivial/small tasks with a local-preferred policy, put a local/free
        // model first (works offline, $0).
        if matches!(need.tier, TaskTier::Trivial | TaskTier::Small)
            && (preferred_is_local || self.free_first)
            && self.routing_mode != "manual"
        {
            if let Some(pos) = result.iter().position(|(prov, b)| {
                (prov == "local" || prov == "ollama") || b.caps().cost_input_per_mtok == 0.0
            }) {
                let chosen = result.remove(pos);
                result.insert(0, chosen);
            }
        }

        result.into_iter().map(|(_, brain)| brain).collect()
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

    fn find_brain_by_id(&self, model_id: &str) -> Option<Arc<dyn Brain>> {
        for brains in self.providers.values() {
            for b in brains {
                if b.id() == model_id {
                    return Some(b.clone());
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{BrainStream, ModelCaps};

    // ─── Mock brain ──────────────────────────────────────────────────────────
    struct MockBrain {
        id: String,
        caps: ModelCaps,
    }

    #[async_trait::async_trait]
    impl Brain for MockBrain {
        fn id(&self) -> &str {
            &self.id
        }
        fn caps(&self) -> ModelCaps {
            self.caps.clone()
        }
        async fn complete(&self, _req: BrainRequest) -> anyhow::Result<BrainStream> {
            // Routing never calls complete(); only id()/caps() are exercised.
            anyhow::bail!("mock brain does not complete")
        }
    }

    fn free_caps() -> ModelCaps {
        ModelCaps {
            cost_input_per_mtok: 0.0,
            cost_output_per_mtok: 0.0,
            tools: true,
            ..Default::default()
        }
    }

    fn brain(id: &str) -> Arc<dyn Brain> {
        Arc::new(MockBrain {
            id: id.into(),
            caps: free_caps(),
        })
    }

    fn medium_need() -> RoutingNeed {
        RoutingNeed {
            tier: TaskTier::Medium,
            required_tools: false,
            required_vision: false,
            prefer_local: false,
        }
    }

    fn ample_budget() -> BudgetState {
        BudgetState {
            daily_limit_usd: 100.0,
            daily_spent_usd: 0.0,
            session_limit_usd: 100.0,
            session_spent_usd: 0.0,
        }
    }

    fn router(providers: HashMap<String, Vec<Arc<dyn Brain>>>) -> BasicRouter {
        BasicRouter::new(&Config::default(), providers)
    }

    // ─── on_error: error → Retry classification (fallback policy) ─────────────
    #[test]
    fn on_error_classifies_every_brain_error() {
        let r = router(HashMap::new());
        let b = brain("x:y");

        // Short rate-limit → wait and retry the SAME provider.
        assert_eq!(
            r.on_error(
                b.as_ref(),
                &BrainError::RateLimit {
                    retry_after: Some(5)
                }
            ),
            Retry::WaitAndRetry(5)
        );
        // Long rate-limit → don't block, fall through the chain.
        assert_eq!(
            r.on_error(
                b.as_ref(),
                &BrainError::RateLimit {
                    retry_after: Some(60)
                }
            ),
            Retry::NextInChain
        );
        assert_eq!(
            r.on_error(b.as_ref(), &BrainError::RateLimit { retry_after: None }),
            Retry::NextInChain
        );
        // 5xx server errors → next provider.
        assert_eq!(
            r.on_error(
                b.as_ref(),
                &BrainError::ServerError {
                    status: 503,
                    body: String::new()
                }
            ),
            Retry::NextInChain
        );
        // Timeout → next provider.
        assert_eq!(
            r.on_error(b.as_ref(), &BrainError::Timeout),
            Retry::NextInChain
        );
        // Explicit model refusal → abort (retrying won't help).
        assert_eq!(
            r.on_error(b.as_ref(), &BrainError::Refusal("nope".into())),
            Retry::Abort
        );
        // Unknown → conservatively try the next provider.
        assert_eq!(
            r.on_error(b.as_ref(), &BrainError::Unknown("boom".into())),
            Retry::NextInChain
        );
    }

    // ─── select: budget exhaustion forces the free/local lane ─────────────────
    #[test]
    fn select_budget_exhausted_returns_only_local() {
        let mut providers: HashMap<String, Vec<Arc<dyn Brain>>> = HashMap::new();
        providers.insert("local".into(), vec![brain("local:tiny")]);
        providers.insert("nvidia".into(), vec![brain("nvidia:big")]);
        let r = router(providers);

        let exhausted = BudgetState {
            daily_limit_usd: 1.0,
            daily_spent_usd: 1.0,
            session_limit_usd: 1.0,
            session_spent_usd: 1.0,
        };
        let chain = r.select(&medium_need(), &exhausted);
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].id(), "local:tiny");
    }

    // ─── select: manual mode pins exactly one model, never falls back ─────────
    #[test]
    fn select_manual_mode_pins_exact_model() {
        let mut providers: HashMap<String, Vec<Arc<dyn Brain>>> = HashMap::new();
        providers.insert(
            "nvidia".into(),
            vec![brain("nvidia:m1"), brain("nvidia:m2")],
        );

        let mut config = Config::default();
        config.routing.routing_mode = "manual".into();
        config.routing.preferred_model = Some("nvidia:m2".into());
        let r = BasicRouter::new(&config, providers);

        let chain = r.select(&medium_need(), &ample_budget());
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].id(), "nvidia:m2");
    }

    #[test]
    fn select_manual_mode_missing_model_is_empty() {
        let mut providers: HashMap<String, Vec<Arc<dyn Brain>>> = HashMap::new();
        providers.insert("nvidia".into(), vec![brain("nvidia:m1")]);

        let mut config = Config::default();
        config.routing.routing_mode = "manual".into();
        config.routing.preferred_model = Some("nvidia:does-not-exist".into());
        let r = BasicRouter::new(&config, providers);

        assert!(r.select(&medium_need(), &ample_budget()).is_empty());
    }

    // ─── select: the chain stays provider-diverse so one dead provider can't
    //     fill every fallback slot (PER_PROVIDER_CAP / MAX_CHAIN contract) ─────
    #[test]
    fn select_chain_is_provider_diverse() {
        let mut providers: HashMap<String, Vec<Arc<dyn Brain>>> = HashMap::new();
        providers.insert(
            "opencode".into(),
            (0..5).map(|i| brain(&format!("opencode:m{i}"))).collect(),
        );
        providers.insert(
            "nvidia".into(),
            (0..2).map(|i| brain(&format!("nvidia:m{i}"))).collect(),
        );
        let r = router(providers);

        let chain = r.select(&medium_need(), &ample_budget());
        // 5 + 2 = 7 candidates, capped to MAX_CHAIN.
        assert_eq!(chain.len(), 6);
        // Both providers must be represented (no monopoly by opencode).
        let nvidia = chain
            .iter()
            .filter(|b| b.id().starts_with("nvidia:"))
            .count();
        let opencode = chain
            .iter()
            .filter(|b| b.id().starts_with("opencode:"))
            .count();
        assert_eq!(
            nvidia, 2,
            "all nvidia fallbacks must survive the per-provider cap"
        );
        assert_eq!(opencode, 4);
        // No duplicate model ids in the chain.
        let mut ids: Vec<&str> = chain.iter().map(|b| b.id()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), chain.len());
    }

    // ─── select: prefer_local drops cloud providers entirely ──────────────────
    #[test]
    fn select_prefer_local_excludes_cloud() {
        let mut providers: HashMap<String, Vec<Arc<dyn Brain>>> = HashMap::new();
        providers.insert("ollama".into(), vec![brain("ollama:llama")]);
        providers.insert("nvidia".into(), vec![brain("nvidia:big")]);
        let r = router(providers);

        let need = RoutingNeed {
            prefer_local: true,
            ..medium_need()
        };
        let chain = r.select(&need, &ample_budget());
        assert!(!chain.is_empty());
        assert!(
            chain.iter().all(|b| b.id().starts_with("ollama:")),
            "prefer_local must exclude cloud providers, got: {:?}",
            chain.iter().map(|b| b.id()).collect::<Vec<_>>()
        );
    }
}
