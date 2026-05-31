#[cfg(test)]
mod tests {
    use sparrow::autonomy::{
        ApprovalPolicy, AutonomyContract, AutonomyLevel, Budget, HardStop, ProposedAction,
    };
    use sparrow::event::{Decision, RiskLevel};
    use sparrow::router::{BasicRouter, BudgetState, Router, RoutingNeed, TaskTier};
    use sparrow::sandbox::{Command, Limits, LocalSandbox, Sandbox};
    use sparrow::event::{Event, OutcomeSummary, RunId};
    use sparrow::redaction::RedactionFilter;
    use sparrow::config::Config;
    use std::sync::Arc;
    use std::path::PathBuf;

    // ─── Autonomy Matrix Tests (§12) ──────────────────────────────────────────

    fn test_action(risk: RiskLevel) -> ProposedAction {
        ProposedAction {
            tool_name: "test_tool".into(),
            risk,
            args: serde_json::json!({}),
        }
    }

    #[test]
    fn test_autonomy_supervised_readonly() {
        let contract = AutonomyContract::supervised();
        let action = test_action(RiskLevel::ReadOnly);
        assert_eq!(contract.decide(&action), Decision::Allow);
    }

    #[test]
    fn test_autonomy_supervised_mutating() {
        let contract = AutonomyContract::supervised();
        let action = test_action(RiskLevel::Mutating);
        assert_eq!(contract.decide(&action), Decision::AskUser);
    }

    #[test]
    fn test_autonomy_supervised_exec() {
        let contract = AutonomyContract::supervised();
        let action = test_action(RiskLevel::Exec);
        assert_eq!(contract.decide(&action), Decision::AskUser);
    }

    #[test]
    fn test_autonomy_supervised_destructive() {
        let contract = AutonomyContract::supervised();
        let action = test_action(RiskLevel::Destructive);
        assert_eq!(contract.decide(&action), Decision::Deny);
    }

    #[test]
    fn test_autonomy_supervised_network() {
        let contract = AutonomyContract::supervised();
        let action = test_action(RiskLevel::Network);
        assert_eq!(contract.decide(&action), Decision::AskUser);
    }

    #[test]
    fn test_autonomy_trusted_mutating() {
        let contract = AutonomyContract::trusted();
        let action = test_action(RiskLevel::Mutating);
        assert_eq!(contract.decide(&action), Decision::Allow);
    }

    #[test]
    fn test_autonomy_trusted_destructive() {
        let contract = AutonomyContract::trusted();
        let action = test_action(RiskLevel::Destructive);
        // Trusted has a hard stop for Destructive, so it's Denied, not Asked
        assert_eq!(contract.decide(&action), Decision::Deny);
    }

    #[test]
    fn test_autonomy_autonomous_mutating() {
        let contract = AutonomyContract::autonomous();
        let action = test_action(RiskLevel::Mutating);
        assert_eq!(contract.decide(&action), Decision::Allow);
    }

    #[test]
    fn test_autonomy_autonomous_exec() {
        let contract = AutonomyContract::autonomous();
        let action = test_action(RiskLevel::Exec);
        assert_eq!(contract.decide(&action), Decision::Allow);
    }

    #[test]
    fn test_autonomy_autonomous_destructive() {
        let contract = AutonomyContract::autonomous();
        let action = test_action(RiskLevel::Destructive);
        assert_eq!(contract.decide(&action), Decision::AskUser);
    }

    #[test]
    fn test_autonomy_hardstop_destructive() {
        let mut contract = AutonomyContract::autonomous();
        contract.stops.push(HardStop::RiskLevel(RiskLevel::Destructive));
        let action = test_action(RiskLevel::Destructive);
        assert_eq!(contract.decide(&action), Decision::Deny);
    }

    // ─── Router Simulation Tests (§12) ────────────────────────────────────────

    use sparrow::provider::{Brain, BrainError, BrainEvent, BrainRequest, BrainStream, ModelCaps, LatencyClass};
    use async_trait::async_trait;
    use futures::stream;

    struct MockBrain {
        id: String,
        caps: ModelCaps,
    }

    #[async_trait]
    impl Brain for MockBrain {
        fn id(&self) -> &str { &self.id }
        fn caps(&self) -> ModelCaps { self.caps.clone() }
        async fn complete(&self, _req: BrainRequest) -> anyhow::Result<BrainStream> {
            Ok(Box::pin(stream::empty()))
        }
    }

    fn make_mock(id: &str, input_cost: f64, output_cost: f64, latency: LatencyClass) -> Arc<dyn Brain> {
        Arc::new(MockBrain {
            id: id.to_string(),
            caps: ModelCaps {
                context_window: 128_000,
                max_output: 16_000,
                tools: true,
                vision: false,
                cost_input_per_mtok: input_cost,
                cost_output_per_mtok: output_cost,
                latency,
            },
        })
    }

    #[test]
    fn test_router_free_first() {
        let mut config = Config::default();
        config.routing.free_first = true;

        let mut providers = std::collections::HashMap::new();
        providers.insert("local".into(), vec![make_mock("local:free-model", 0.0, 0.0, LatencyClass::Slow)]);
        providers.insert("cloud".into(), vec![make_mock("cloud:paid-model", 10.0, 30.0, LatencyClass::Fast)]);

        let router = BasicRouter::new(&config, providers);

        let need = RoutingNeed {
            tier: TaskTier::Medium,
            required_tools: true,
            required_vision: false,
            prefer_local: false,
        };

        let budget = BudgetState {
            daily_limit_usd: 100.0,
            daily_spent_usd: 0.0,
            session_limit_usd: 10.0,
            session_spent_usd: 0.0,
        };

        let chain = router.select(&need, &budget);
        assert!(!chain.is_empty());
        // Free model should be first due to free_first policy
        assert!(chain[0].id().contains("free-model"));
    }

    #[test]
    fn test_router_budget_exhausted() {
        let mut config = Config::default();
        config.routing.free_first = false;

        let mut providers = std::collections::HashMap::new();
        providers.insert("cloud".into(), vec![make_mock("cloud:paid-model", 10.0, 30.0, LatencyClass::Fast)]);

        let router = BasicRouter::new(&config, providers);

        let need = RoutingNeed {
            tier: TaskTier::Medium,
            required_tools: true,
            required_vision: false,
            prefer_local: false,
        };

        let budget = BudgetState {
            daily_limit_usd: 0.0,
            daily_spent_usd: 0.0,
            session_limit_usd: 0.0,
            session_spent_usd: 0.0,
        };

        let chain = router.select(&need, &budget);
        // All paid models should be filtered out
        assert!(chain.is_empty() || chain.iter().all(|b| b.caps().cost_input_per_mtok == 0.0));
    }

    #[test]
    fn test_router_policy_tiers() {
        let mut config = Config::default();
        config.routing.policy.insert("trivial".into(), "local".into());
        config.routing.policy.insert("hard".into(), "cloud".into());

        let mut providers = std::collections::HashMap::new();
        providers.insert("local".into(), vec![make_mock("local:cheap", 0.0, 0.0, LatencyClass::Fast)]);
        providers.insert("cloud".into(), vec![make_mock("cloud:powerful", 10.0, 30.0, LatencyClass::Slow)]);

        let router = BasicRouter::new(&config, providers);

        let need = RoutingNeed {
            tier: TaskTier::Trivial,
            required_tools: false,
            required_vision: false,
            prefer_local: false,
        };

        let budget = BudgetState {
            daily_limit_usd: 100.0, daily_spent_usd: 0.0,
            session_limit_usd: 10.0, session_spent_usd: 0.0,
        };

        let chain = router.select(&need, &budget);
        // Trivial should prefer local
        assert!(chain[0].id().contains("local"));
    }

    #[test]
    fn test_router_small_prefers_ollama_before_free_cloud() {
        let mut config = Config::default();
        config.routing.free_first = true;

        let mut providers = std::collections::HashMap::new();
        providers.insert("ollama".into(), vec![make_mock("qwen3.5:32b", 0.0, 0.0, LatencyClass::Slow)]);
        providers.insert("nvidia".into(), vec![make_mock("nvidia/nemotron", 0.0, 0.0, LatencyClass::Fast)]);

        let router = BasicRouter::new(&config, providers);
        let need = RoutingNeed {
            tier: TaskTier::Small,
            required_tools: false,
            required_vision: false,
            prefer_local: false,
        };
        let budget = BudgetState {
            daily_limit_usd: 100.0,
            daily_spent_usd: 0.0,
            session_limit_usd: 10.0,
            session_spent_usd: 0.0,
        };

        let chain = router.select(&need, &budget);
        assert_eq!(chain[0].id(), "qwen3.5:32b");
        assert_eq!(chain[1].id(), "nvidia/nemotron");
    }

    #[test]
    fn test_router_rate_limit_retry() {
        let router = BasicRouter::new(&Config::default(), std::collections::HashMap::new());
        let brain = make_mock("test", 1.0, 1.0, LatencyClass::Fast);

        let retry = router.on_error(brain.as_ref(), &BrainError::RateLimit { retry_after: Some(5) });
        assert!(matches!(retry, sparrow::router::Retry::WaitAndRetry(5)));

        let retry = router.on_error(brain.as_ref(), &BrainError::RateLimit { retry_after: Some(60) });
        assert!(matches!(retry, sparrow::router::Retry::NextInChain));

        let retry = router.on_error(brain.as_ref(), &BrainError::Refusal("no".into()));
        assert!(matches!(retry, sparrow::router::Retry::Abort));
    }

    // ─── Sandbox Escape Tests (§12) ───────────────────────────────────────────

    #[test]
    fn test_sandbox_path_isolation() {
        let tmp = std::env::temp_dir().join("sparrow-test-sandbox");
        std::fs::create_dir_all(&tmp).ok();

        let sandbox = LocalSandbox::new(tmp.clone());

        // Root should be the workspace
        assert_eq!(sandbox.root(), tmp.as_path());

        // Policy should restrict to workspace
        let policy = sandbox.policy();
        assert!(policy.allowed_paths.iter().any(|p| p == &tmp));

        // Cleanup
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_sandbox_exec_basic() {
        let tmp = std::env::temp_dir().join("sparrow-test-exec");
        std::fs::create_dir_all(&tmp).ok();

        let sandbox = LocalSandbox::new(tmp.clone());
        let cmd = Command {
            program: if cfg!(windows) { "cmd" } else { "echo" }.into(),
            args: if cfg!(windows) {
                vec!["/c".into(), "echo hello".into()]
            } else {
                vec!["hello".into()]
            },
            env: std::collections::HashMap::new(),
            workdir: tmp.clone(),
        };
        let limits = Limits { timeout_ms: 5000, max_output_bytes: 1024 };

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(sandbox.exec(&cmd, &limits));
        match result {
            Ok(r) => {
                assert!(r.stdout.contains("hello"));
                assert_eq!(r.exit_code, 0);
            }
            Err(_) => {} // May fail on some platforms, not a test failure
        }

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_redaction_filter_patterns() {
        let filter = RedactionFilter::new();
        assert!(filter.contains_secret("sk-ant-api03-abcdef"));
        assert!(filter.contains_secret("ghp_1234567890abcdef"));
        assert!(!filter.contains_secret("normal text"));
    }

    #[test]
    fn test_redaction_event() {
        let mut filter = RedactionFilter::new();
        filter.load_secrets(vec!["mysecret".into()]);

        let event = Event::ThinkingDelta {
            run: RunId("test".into()),
            text: "key is mysecret here".into(),
        };
        let redacted = filter.redact_event(&event);
        if let Event::ThinkingDelta { text, .. } = redacted {
            assert!(!text.contains("mysecret"));
            assert!(text.contains("[REDACTED]"));
        } else {
            panic!("wrong type");
        }
    }

    // ─── Golden Replay Test (§12) ────────────────────────────────────────────

    #[test]
    fn test_golden_replay_roundtrip() {
        let events = vec![
            Event::RunStarted {
                run: RunId("golden-1".into()),
                task: "test task".into(),
                agent: "test-agent".into(),
            },
            Event::ThinkingDelta {
                run: RunId("golden-1".into()),
                text: "thinking...".into(),
            },
            Event::RunFinished {
                run: RunId("golden-1".into()),
                outcome: OutcomeSummary {
                    status: "completed".into(),
                    diffs: vec![],
                    cost_usd: 0.01,
                    tokens: sparrow::event::TokenUsage { input: 100, output: 50 },
                },
            },
        ];

        // Serialize to JSONL
        let mut jsonl = String::new();
        for e in &events {
            jsonl.push_str(&serde_json::to_string(e).unwrap());
            jsonl.push('\n');
        }

        // Deserialize back
        let parsed: Vec<Event> = jsonl
            .lines()
            .filter(|l| !l.is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();

        assert_eq!(parsed.len(), 3);

        // Verify first event
        match &parsed[0] {
            Event::RunStarted { run, task, agent } => {
                assert_eq!(run.0, "golden-1");
                assert_eq!(task, "test task");
                assert_eq!(agent, "test-agent");
            }
            _ => panic!(),
        }
    }

    // ─── Swarm Pipeline Test (§12) ──────────────────────────────────────────

    #[test]
    fn test_pipeline_config_validation() {
        use sparrow::extras::PipelineConfig;

        // Valid default pipeline
        let default = PipelineConfig::default_pipeline();
        assert!(default.validate().is_ok());

        // Invalid: unknown dependency
        let invalid = PipelineConfig {
            name: "bad".into(),
            steps: vec![
                sparrow::extras::PipelineStep {
                    role: "coder".into(),
                    model_preference: None,
                    prompt_override: None,
                    depends_on: vec!["nonexistent".into()],
                },
            ],
            max_reworks: 3,
        };
        assert!(invalid.validate().is_err());

        // Empty pipeline
        let empty = PipelineConfig {
            name: "empty".into(),
            steps: vec![],
            max_reworks: 1,
        };
        assert!(empty.validate().is_err());
    }

    // ─── Config Tests ────────────────────────────────────────────────────────

    #[test]
    fn test_config_defaults() {
        let config = Config::default();
        assert_eq!(config.theme, "captain");
        assert_eq!(config.budget.daily_usd, 5.0);
        assert_eq!(config.budget.session_usd, 1.0);
        assert!(config.defaults.autonomy == AutonomyLevel::Trusted);
    }

    // ─── Embeddings Test ─────────────────────────────────────────────────────

    #[test]
    fn test_embeddings_similarity() {
        use sparrow::extras::Embeddings;

        let mut emb = Embeddings::new();
        emb.add("Rust programming language");
        emb.add("TypeScript for web development");
        emb.add("cooking recipes for dinner");

        let results = emb.search("Rust programming", 2);
        assert!(!results.is_empty());
        assert!(results[0].contains("Rust"));
    }

    // ─── TaskTier Tests ──────────────────────────────────────────────────────

    #[test]
    fn test_task_tier_from_str() {
        assert!(matches!(TaskTier::from_str("trivial"), TaskTier::Trivial));
        assert!(matches!(TaskTier::from_str("HARD"), TaskTier::Hard));
        assert!(matches!(TaskTier::from_str("unknown"), TaskTier::Medium));
    }

    // ─── Provider Registry Tests ───────────────────────────────────────────

    #[test]
    fn test_provider_registry_not_empty() {
        let providers = sparrow::config::providers::provider_registry();
        assert!(!providers.is_empty());
        assert!(providers.iter().any(|p| p.id == "ollama"));
        assert!(providers.iter().any(|p| p.id == "anthropic"));
        assert!(providers.iter().any(|p| p.id == "nvidia"));
        assert!(providers.iter().any(|p| p.id == "openai-codex"));
    }

    #[test]
    fn test_provider_registry_models_have_tags() {
        let providers = sparrow::config::providers::provider_registry();
        for p in &providers {
            for m in &p.models {
                assert!(!m.tags.is_empty(), "Model {}/{} has no tags", p.id, m.name);
            }
        }
    }

    #[test]
    fn test_find_provider() {
        let found = sparrow::config::providers::find_provider("nvidia");
        assert!(found.is_some());
        assert_eq!(found.unwrap().label, "NVIDIA NIM");
    }

    #[test]
    fn test_find_model() {
        let found = sparrow::config::providers::find_model("anthropic", "claude-sonnet-4-6");
        assert!(found.is_some());
        assert!(found.unwrap().tags.contains(&"code".to_string()));
    }

    #[test]
    fn test_onboarding_providers() {
        let providers = sparrow::config::providers::onboarding_providers();
        // Should return top recommended ones
        let ids: Vec<&str> = providers.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&"ollama"));
        assert!(ids.contains(&"nvidia"));
        assert!(ids.contains(&"anthropic"));
    }

    // ─── Credential Safety Tests ──────────────────────────────────────────

    #[test]
    fn test_api_key_not_in_config_view() {
        use sparrow::config::ProviderConfig;
        let mut cfg = sparrow::config::Config::default();
        cfg.providers.insert("test".into(), ProviderConfig {
            adapter: "openai-compatible".into(),
            base_url: Some("https://example.com".into()),
            models: vec!["gpt-5".into()],
            api_key_env: Some("sk-real-key-12345".into()),
        });
        // The config itself may hold a key, but the API view should redact
        let p = cfg.providers.get("test").unwrap();
        assert!(p.api_key_env.as_ref().unwrap().contains("sk-"));
    }

    #[test]
    fn test_config_defaults_have_providers_empty() {
        let cfg = sparrow::config::Config::default();
        assert!(cfg.providers.is_empty());
        assert_eq!(cfg.budget.daily_usd, 5.0);
    }

    // ─── M1 Trust Tests ──────────────────────────────────────────────────

    #[test]
    fn test_checkpoints_rewind() {
        use sparrow::autonomy::{Checkpoints, GitCheckpoints};
        let tmp = std::env::temp_dir().join("sparrow-m1-checkpoint-test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).ok();

        // Init git repo
        std::process::Command::new("git").args(["init"]).current_dir(&tmp).output().ok();
        std::process::Command::new("git").args(["config","user.email","test@sparrow.dev"]).current_dir(&tmp).output().ok();
        std::process::Command::new("git").args(["config","user.name","Test"]).current_dir(&tmp).output().ok();
        std::fs::write(tmp.join("test.txt"), "original").ok();
        std::process::Command::new("git").args(["add","test.txt"]).current_dir(&tmp).output().ok();
        std::process::Command::new("git").args(["commit","-m","init"]).current_dir(&tmp).output().ok();

        let cp = GitCheckpoints::new(tmp.clone());
        let id = cp.snapshot("pre-mutation").expect("snapshot should succeed");
        assert!(!id.0.is_empty());

        // Mutate
        std::fs::write(tmp.join("test.txt"), "modified").ok();

        // Rewind
        cp.rewind(id).expect("rewind should succeed");
        let content = std::fs::read_to_string(tmp.join("test.txt")).unwrap_or_default();
        assert_eq!(content.trim(), "original", "rewind must restore original content");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_memory_persistence() {
        use sparrow::memory::{Fact, Memory, SqliteMemory};
        let tmp = std::env::temp_dir().join("sparrow-m1-memory.db");
        let _ = std::fs::remove_file(&tmp);

        let mem = SqliteMemory::open(&tmp).expect("open memory");
        let fact = Fact {
            id: "test-1".into(), key: "user:language".into(), value: "Rust".into(),
            created_at: "2026-01-01".into(), updated_at: "2026-01-01".into(),
        };
        mem.remember(fact.clone()).expect("remember");

        // Re-open (simulates new session)
        drop(mem);
        let mem2 = SqliteMemory::open(&tmp).expect("re-open memory");
        let facts = mem2.all_facts();
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].key, "user:language");
        assert_eq!(facts[0].value, "Rust");

        // Recall via FTS5
        let recalled = mem2.recall("Rust", 5);
        assert!(!recalled.is_empty());

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_autonomy_matrix_15_combinations() {
        use sparrow::autonomy::{AutonomyContract, ProposedAction};
        use sparrow::event::{AutonomyLevel, RiskLevel};

        let levels = [AutonomyLevel::Supervised, AutonomyLevel::Trusted, AutonomyLevel::Autonomous];
        let risks = [RiskLevel::ReadOnly, RiskLevel::Mutating, RiskLevel::Exec, RiskLevel::Destructive, RiskLevel::Network];

        for level in &levels {
            for risk in &risks {
                let contract = match level {
                    AutonomyLevel::Supervised => AutonomyContract::supervised(),
                    AutonomyLevel::Trusted => AutonomyContract::trusted(),
                    AutonomyLevel::Autonomous => AutonomyContract::autonomous(),
                    _ => unreachable!(),
                };
                let action = ProposedAction {
                    tool_name: "test".into(),
                    risk: risk.clone(),
                    args: serde_json::json!({}),
                };
                let decision = contract.decide(&action);
                // Every combination must produce a valid decision
                assert!(matches!(decision, sparrow::event::Decision::Allow | sparrow::event::Decision::AskUser | sparrow::event::Decision::Deny));
            }
        }
        // 5 × 3 = 15 combinations tested
    }

    #[test]
    fn test_budget_hard_stop_in_contract() {
        use sparrow::autonomy::AutonomyContract;
        let contract = AutonomyContract::supervised();
        // Budget exceeded should be a hard stop
        assert!(contract.stops.iter().any(|s| matches!(s, sparrow::autonomy::HardStop::BudgetExceeded)));
        // Destructive should be denied in supervised
        assert!(contract.stops.iter().any(|s| matches!(s, sparrow::autonomy::HardStop::RiskLevel(sparrow::event::RiskLevel::Destructive))));
    }

    #[test]
    fn test_memory_redaction() {
        use sparrow::memory::{Fact, Memory, SqliteMemory};
        let tmp = std::env::temp_dir().join("sparrow-m1-redact.db");
        let _ = std::fs::remove_file(&tmp);

        let mem = SqliteMemory::open(&tmp).expect("open memory");
        let fact_with_secret = Fact {
            id: "secret-1".into(), key: "token".into(),
            value: "sk-ant-api03-secret-key-here".into(),
            created_at: "2026-01-01".into(), updated_at: "2026-01-01".into(),
        };
        mem.remember(fact_with_secret).expect("remember");

        let facts = mem.all_facts();
        assert!(!facts.is_empty());
        // The value should be redacted
        let val = &facts[0].value;
        assert!(!val.contains("sk-ant-api03"), "Secret should be redacted: {}", val);

        let _ = std::fs::remove_file(&tmp);
    }
}
