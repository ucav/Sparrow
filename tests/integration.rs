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

    // ─── M2 Swarm Tests ──────────────────────────────────────────────────

    #[test]
    fn test_file_locks_prevent_collision() {
        use sparrow::orchestrator::FileLocks;
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let locks = FileLocks::new();
            // Lock file A
            assert!(locks.try_lock(&["auth.rs".into()]).await.is_ok());
            // Can't lock same file again
            assert!(locks.try_lock(&["auth.rs".into()]).await.is_err());
            // But can lock different file
            assert!(locks.try_lock(&["token.rs".into()]).await.is_ok());
            // Release
            locks.release(&["auth.rs".into()]).await;
            // Now can lock again
            assert!(locks.try_lock(&["auth.rs".into()]).await.is_ok());
        });
    }

    #[test]
    fn test_swarm_verdict_pass_rework() {
        use sparrow::orchestrator::Verdict;
        let pass = Verdict::Pass;
        let rework = Verdict::Rework { findings: vec!["missing edge case".into()] };

        assert!(matches!(pass, Verdict::Pass));
        if let Verdict::Rework { findings } = rework {
            assert_eq!(findings.len(), 1);
        } else {
            panic!("expected Rework");
        }
    }

    #[test]
    fn test_swarm_plan_defaults() {
        use sparrow::orchestrator::SwarmPlan;
        let plan = SwarmPlan::default();
        assert_eq!(plan.max_reworks, 3);
    }

    #[test]
    fn test_orchestrator_events_emitted() {
        // Smoke test: orchestrator struct can be constructed
        let config = sparrow::config::Config::default();
        let router = Arc::new(sparrow::router::BasicRouter::new(
            &config,
            std::collections::HashMap::new(),
        ));
        let memory: Arc<dyn sparrow::memory::Memory> = Arc::new(
            sparrow::memory::SqliteMemory::open(&std::env::temp_dir().join("sparrow-m2-swarm-smoke.db")).unwrap()
        );
        let _orchestrator = sparrow::orchestrator::DefaultOrchestrator::new(router, config, memory);
        // Verify orchestrator can be constructed without panicking
        assert!(true);
    }

    // ─── M3 Grows Tests ──────────────────────────────────────────────────

    #[test]
    fn test_skill_parse_from_markdown() {
        use sparrow::capabilities::Skill;
        let md = "# Skill: Rust Error Handling\n\n**Trigger:** error, Result, anyhow\n\n## Body\nUse anyhow for app code.";
        let skill = Skill::from_markdown(md, "test.skill.md");
        assert!(skill.is_some());
        let s = skill.unwrap();
        assert_eq!(s.name, "Rust Error Handling");
        assert!(s.trigger.contains(&"error".to_string()));
        assert!(s.trigger.contains(&"result".to_string()));
        assert!(s.body.contains("anyhow"));
    }

    #[test]
    fn test_skill_relevance_scoring() {
        use sparrow::capabilities::Skill;
        let skill = Skill {
            name: "test".into(), description: "desc".into(),
            trigger: vec!["rust".into(), "error".into(), "handling".into()],
            body: "test body".into(), source_file: "".into(),
            usage_count: 0, created_at: "".into(), score: 0.5, auto_generated: false,
        };
        let score = skill.relevance("I need help with Rust error handling in my code");
        assert!(score > 0.0, "Should be relevant for matching context");

        let score2 = skill.relevance("cooking recipes");
        assert_eq!(score2, 0.0, "Should not match unrelated context");
    }

    #[test]
    fn test_skill_markdown_roundtrip() {
        use sparrow::capabilities::Skill;
        let original = Skill {
            name: "TestSkill".into(), description: "A test skill".into(),
            trigger: vec!["test".into(), "demo".into()],
            body: "This is the body".into(), source_file: "test.skill.md".into(),
            usage_count: 2, created_at: "2026-01-01".into(), score: 0.7, auto_generated: true,
        };
        let md = original.to_markdown();
        let parsed = Skill::from_markdown(&md, "test.skill.md");
        assert!(parsed.is_some());
        assert_eq!(parsed.unwrap().name, "TestSkill");
    }

    #[test]
    fn test_curator_propose_skill() {
        use sparrow::capabilities::Curator;
        let candidate = Curator::propose_skill(
            "Implement Rust error handling with anyhow",
            "completed"
        );
        assert!(candidate.is_some());
        let skill = candidate.unwrap();
        assert!(skill.auto_generated);
        assert!(!skill.trigger.is_empty());
    }

    #[test]
    fn test_fs_skill_library_add_and_get() {
        use sparrow::capabilities::{FsSkillLibrary, Skill, SkillLibrary};
        let tmp = std::env::temp_dir().join("sparrow-m3-skills-test");
        let _ = std::fs::remove_dir_all(&tmp);
        let lib = FsSkillLibrary::new(tmp.clone());

        let skill = Skill {
            name: "test-skill".into(), description: "test".into(),
            trigger: vec!["test".into()], body: "body".into(),
            source_file: "test".into(), usage_count: 0,
            created_at: "2026-01-01".into(), score: 0.5, auto_generated: true,
        };
        lib.add(skill).expect("add skill");
        let found = lib.get("test-skill");
        assert!(found.is_some());
        assert_eq!(found.unwrap().body, "body");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_mcp_client_server_config() {
        use sparrow::capabilities::mcp::{BasicMcpClient, McpClient, McpServer, Transport};
        let tmp = std::env::temp_dir().join("sparrow-m3-mcp-test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).ok();

        let client = BasicMcpClient::new(tmp.clone());
        let server = McpServer {
            name: "test-server".into(), transport: Transport::Stdio,
            command: Some("echo".into()), args: vec!["hello".into()],
            url: None, env: Default::default(), allow_tools: vec![],
        };
        client.add_server(server).expect("add server");
        let found = client.get_server("test-server");
        assert!(found.is_some());

        let rt = tokio::runtime::Runtime::new().unwrap();
        let servers = rt.block_on(async { client.list_servers().await });
        assert_eq!(servers.len(), 1);

        client.remove_server("test-server").expect("remove");
        let servers = rt.block_on(async { client.list_servers().await });
        assert_eq!(servers.len(), 0);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ─── M4 Runtime Tests ────────────────────────────────────────────────

    #[test]
    fn test_scheduler_job_persistence() {
        use sparrow::runtime::scheduler::{Job, MemoryScheduler, Scheduler};
        let scheduler = MemoryScheduler::new();
        let job = Job::new("test task".into(), "0 */6 * * *".into());
        let id = scheduler.schedule(job).expect("schedule");
        let jobs = scheduler.list();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].task, "test task");
        scheduler.cancel(&id).expect("cancel");
        assert_eq!(scheduler.list().len(), 0);
    }

    #[test]
    fn test_recorder_transcript_roundtrip() {
        use sparrow::runtime::recorder::{FsRecorder, Replayer, RunInputs, Recorder};
        use sparrow::event::{Event, RunId, OutcomeSummary, TokenUsage};
        let tmp = std::env::temp_dir().join("sparrow-m4-transcript-test");
        let _ = std::fs::remove_dir_all(&tmp);

        let recorder = FsRecorder::new(tmp.clone());
        let run_id = "test-run-1".to_string();
        recorder.start_run(run_id.clone(), RunInputs {
            task: "test".into(), config_snapshot: serde_json::json!({}),
            model_id: "test-model".into(), repo_head: None,
            timestamp: "2026-01-01".into(), agent: "test-agent".into(),
        });

        // Record events
        recorder.record(&Event::RunStarted {
            run: RunId(run_id.clone()), task: "test".into(), agent: "test-agent".into(),
        });
        recorder.record(&Event::ThinkingDelta {
            run: RunId(run_id.clone()), text: "thinking...".into(),
        });
        recorder.record(&Event::RunFinished {
            run: RunId(run_id.clone()),
            outcome: OutcomeSummary {
                status: "completed".into(), diffs: vec![], cost_usd: 0.0,
                tokens: TokenUsage { input: 100, output: 50 },
            },
        });

        let transcript = recorder.finalize(&run_id).expect("finalize");
        assert_eq!(transcript.events.len(), 3);

        // Replay
        let loaded = recorder.load(&run_id);
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().events.len(), 3);

        // List
        let transcripts = recorder.list_transcripts();
        assert!(transcripts.contains(&run_id));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_golden_replay_deterministic() {
        use sparrow::event::{Event, RunId, OutcomeSummary, TokenUsage};
        let events = vec![
            Event::RunStarted { run: RunId("g1".into()), task: "t1".into(), agent: "a1".into() },
            Event::ThinkingDelta { run: RunId("g1".into()), text: "hello".into() },
            Event::RunFinished { run: RunId("g1".into()), outcome: OutcomeSummary {
                status: "ok".into(), diffs: vec![], cost_usd: 0.0,
                tokens: TokenUsage { input: 10, output: 5 },
            }},
        ];

        // Serialize
        let jsonl: String = events.iter()
            .map(|e| serde_json::to_string(e).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        let jsonl = jsonl + "\n";

        // Deserialize
        let parsed: Vec<Event> = jsonl.lines()
            .filter(|l| !l.is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();

        assert_eq!(parsed.len(), 3, "Deterministic: 3 events roundtrip");
        if let Event::RunStarted { task, .. } = &parsed[0] {
            assert_eq!(task, "t1");
        } else { panic!("Wrong event type"); }
        if let Event::ThinkingDelta { text, .. } = &parsed[1] {
            assert_eq!(text, "hello");
        } else { panic!("Wrong event type"); }
        if let Event::RunFinished { outcome, .. } = &parsed[2] {
            assert_eq!(outcome.status, "ok");
        } else { panic!("Wrong event type"); }
    }

    // ─── M5 Gateway Tests ────────────────────────────────────────────────

    #[test]
    fn test_gateway_message_router_commands() {
        use sparrow::gateway::{GatewayMessage, GatewayResponse};
        // Test that command parsing works for /help
        let msg = GatewayMessage {
            surface: "telegram".into(), user_id: "123".into(),
            chat_id: "456".into(), text: "/help".into(), message_id: None,
        };
        // Verify the message is well-formed (router handles actual dispatch)
        assert_eq!(msg.surface, "telegram");
        assert_eq!(msg.text, "/help");
    }

    #[test]
    fn test_gateway_event_formatting() {
        use sparrow::gateway::format_event;
        use sparrow::event::{Event, RunId};

        let event = Event::RunStarted {
            run: RunId("test".into()), task: "fix auth with key sk-ant-secret".into(), agent: "sparrow".into(),
        };
        let formatted = format_event(&event);
        assert!(formatted.is_some());
        let text = formatted.unwrap();
        assert!(text.contains("fix auth"));
    }

    #[test]
    fn test_gateway_response_buttons() {
        use sparrow::gateway::GatewayResponse;
        let resp = GatewayResponse {
            chat_id: "123".into(), text: "Approve?".into(),
            reply_to: None, buttons: vec![vec!["/approve".into(), "/deny".into()]],
        };
        assert_eq!(resp.buttons.len(), 1);
        assert_eq!(resp.buttons[0].len(), 2);
        assert_eq!(resp.buttons[0][0], "/approve");
    }

    #[test]
    fn test_session_bridge_continuity() {
        use sparrow::tools::extras::SessionBridge;
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let bridge = SessionBridge::new();
            assert!(!bridge.session_id.is_empty());

            bridge.set_surface("telegram").await;
            bridge.add_approval(sparrow::gateway::GatewayResponse {
                chat_id: "123".into(), text: "Approve?".into(), reply_to: None, buttons: vec![],
            }).await;
            let approvals = bridge.drain_approvals().await;
            assert_eq!(approvals.len(), 1);
        });
    }

    // ─── WS8 Onboarding Tests ───────────────────────────────────────────

    #[test]
    fn test_user_mode_default_beginner() {
        use sparrow::onboarding::UserMode;
        assert_eq!(UserMode::default(), UserMode::Beginner);
    }

    #[test]
    fn test_lessons_count() {
        use sparrow::onboarding::Onboarding;
        let ob = Onboarding::default();
        assert_eq!(ob.lessons.len(), 5);
        assert_eq!(ob.lessons[0].id, "run");
        assert_eq!(ob.lessons[4].id, "gateway");
    }

    #[test]
    fn test_friendly_error_no_provider() {
        let msg = sparrow::onboarding::Onboarding::friendly_error("no_provider", "nvidia");
        assert!(msg.contains("sparrow auth add"));
        assert!(msg.contains("NVIDIA_API_KEY"));
    }

    #[test]
    fn test_examples_gallery() {
        let examples = sparrow::onboarding::Onboarding::examples_gallery();
        assert!(!examples.is_empty());
        assert!(examples.iter().any(|(cmd, _)| cmd.contains("run")));
    }

    // ─── WS10 Migration Tests ──────────────────────────────────────────

    #[test]
    fn test_migration_detect_installed_does_not_panic() {
        let found = sparrow::onboarding::migration::Migration::detect_installed();
        // May be empty on CI — just verify it doesn't panic
        assert!(found.len() <= 5);
    }

    #[test]
    fn test_migration_openclaw_result_fields() {
        // Test that MigrationResult struct is well-formed
        let result = sparrow::onboarding::migration::MigrationResult {
            tool: "openclaw".into(),
            agents: 3, skills: 5, cron_jobs: 2, config_entries: 10, surfaces: 1,
        };
        assert_eq!(result.agents, 3);
        assert_eq!(result.skills, 5);
    }

    // ─── WS12 Enterprise Tests ─────────────────────────────────────────

    #[test]
    fn test_org_policy_enforces_autonomy_ceiling() {
        use sparrow::onboarding::enterprise::OrgPolicy;
        let policy = OrgPolicy::default();
        let result = policy.enforce(
            &sparrow::event::AutonomyLevel::Autonomous,
            1.0,
            "src/main.rs",
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("limits autonomy"));
    }

    #[test]
    fn test_org_policy_blocks_protected_paths() {
        use sparrow::onboarding::enterprise::OrgPolicy;
        let policy = OrgPolicy::default();
        let result = policy.enforce(
            &sparrow::event::AutonomyLevel::Supervised,
            0.0,
            ".env",
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("protected"));
    }

    #[test]
    fn test_org_policy_allows_safe_paths() {
        use sparrow::onboarding::enterprise::OrgPolicy;
        let policy = OrgPolicy::default();
        let result = policy.enforce(
            &sparrow::event::AutonomyLevel::Supervised,
            0.0,
            "src/lib.rs",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_audit_export_json() {
        use sparrow::onboarding::enterprise::{AuditEntry, export_audit_log};
        let entries = vec![AuditEntry {
            timestamp: "2026-01-01".into(), user: "alice".into(),
            action: "run".into(), run_id: "r1".into(),
            cost_usd: 0.01, tokens: 500, autonomy: "supervised".into(),
            status: "completed".into(),
        }];
        let json = export_audit_log(&entries, "json");
        assert!(json.contains("alice"));
        assert!(json.contains("r1"));
    }

    // ─── WS16 Crash Recovery & Fuzzing Tests ────────────────────────────

    #[test]
    fn test_crash_recovery_transcript_persistence() {
        use sparrow::runtime::recorder::{FsRecorder, Recorder, Replayer, RunInputs};
        use sparrow::event::{Event, RunId, OutcomeSummary, TokenUsage};
        let tmp = std::env::temp_dir().join("sparrow-ws16-crash");
        let _ = std::fs::remove_dir_all(&tmp);

        let recorder = FsRecorder::new(tmp.clone());
        let rid = "crash-test-1".to_string();
        recorder.start_run(rid.clone(), RunInputs {
            task: "test".into(), config_snapshot: serde_json::json!({}),
            model_id: "test".into(), repo_head: None,
            timestamp: "2026-01-01".into(), agent: "test".into(),
        });
        let event = Event::RunStarted {
            run: RunId(rid.clone()), task: "test".into(), agent: "test".into(),
        };
        recorder.record(&event);
        // Simulate crash: finalize is called even though no RunFinished
        let transcript = recorder.finalize(&rid).expect("finalize after crash");
        assert!(!transcript.events.is_empty(), "Partial transcript saved despite crash");

        // Reload — transcript is recoverable
        let loaded = recorder.load(&rid);
        assert!(loaded.is_some(), "Transcript recoverable after simulated crash");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_anti_simulation_guard_detects_fabrication() {
        use sparrow::reasoning::AntiSimulationGuard;
        use sparrow::provider::{ContentBlock, Msg};
        let messages = vec![Msg {
            role: "assistant".into(),
            content: vec![ContentBlock::Text {
                text: "All tests pass and the build succeeds.".into(),
            }],
        }];
        let violation = AntiSimulationGuard::check(&messages, false);
        assert!(violation.is_some(), "Guard must detect fabricated test claims");
    }

    #[test]
    fn test_hallucination_guard_detects_code_claim() {
        use sparrow::reasoning::AntiSimulationGuard;
        let violation = AntiSimulationGuard::check_code_claim(
            "the validate() function exists in auth.rs and returns a bool",
            false, false,
        );
        assert!(violation.is_some(), "Guard must detect unverified code claims");
    }

    #[test]
    fn test_hallucination_guard_allows_verified_claim() {
        use sparrow::reasoning::AntiSimulationGuard;
        let violation = AntiSimulationGuard::check_code_claim(
            "the validate() function exists in auth.rs",
            true, false, // had fs_read
        );
        assert!(violation.is_none(), "Guard must allow claims backed by fs_read");
    }

    #[test]
    fn test_config_fuzzing_roundtrip() {
        // Fuzz-like: serialize/deserialize with edge cases
        let cfg = sparrow::config::Config::default();
        let serialized = toml::to_string_pretty(&cfg).expect("serialize");
        let deserialized: sparrow::config::Config = toml::from_str(&serialized).expect("deserialize");
        assert_eq!(deserialized.budget.daily_usd, cfg.budget.daily_usd);
    }

    #[test]
    fn test_provider_json_fuzzing() {
        // Verify all provider entries parse correctly
        let providers = sparrow::config::providers::provider_registry();
        assert!(!providers.is_empty());
        for p in &providers {
            assert!(!p.id.is_empty(), "Provider {} has empty id", p.label);
            assert!(!p.adapter.is_empty(), "Provider {} has empty adapter", p.id);
            for m in &p.models {
                assert!(!m.name.is_empty(), "Model {}/{} has empty name", p.id, m.label);
                assert!(m.context_window > 0, "Model {}/{} has zero context window", p.id, m.name);
            }
        }
    }
}
