use async_trait::async_trait;
use futures::stream;
use sparrow::config::Config;
use sparrow::event::{Event, StopReason};
use sparrow::memory::{Memory, SqliteMemory};
use sparrow::orchestrator::{DefaultOrchestrator, Orchestrator, SwarmPlan};
use sparrow::provider::{Brain, BrainEvent, BrainRequest, BrainStream, LatencyClass, ModelCaps};
use sparrow::router::BasicRouter;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

#[derive(Clone)]
struct ScriptedBrain {
    responses: Arc<Mutex<VecDeque<Vec<BrainEvent>>>>,
}

#[async_trait]
impl Brain for ScriptedBrain {
    fn id(&self) -> &str {
        "local:swarm-scripted"
    }

    fn caps(&self) -> ModelCaps {
        ModelCaps {
            context_window: 32_768,
            max_output: 4096,
            tools: true,
            vision: false,
            cost_input_per_mtok: 0.0,
            cost_output_per_mtok: 0.0,
            latency: LatencyClass::Fast,
        }
    }

    async fn complete(&self, _req: BrainRequest) -> anyhow::Result<BrainStream> {
        let events = self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| vec![BrainEvent::Done(StopReason::EndTurn)]);
        Ok(Box::pin(stream::iter(events)))
    }
}

fn temp_path(name: &str) -> PathBuf {
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("sparrow-{name}-{id}"))
}

#[tokio::test]
async fn orchestrator_emits_diff_applied_only_after_verifier_pass() {
    let workspace = temp_path("orchestrator-gate");
    std::fs::create_dir_all(workspace.join("src")).unwrap();
    std::fs::write(
        workspace.join("src").join("lib.rs"),
        "pub fn value() -> u8 { 1 }\n",
    )
    .unwrap();

    let brain = ScriptedBrain {
        responses: Arc::new(Mutex::new(
            vec![
                vec![
                    BrainEvent::TextDelta("Implement the requested change.".into()),
                    BrainEvent::Done(StopReason::EndTurn),
                ],
                vec![
                    BrainEvent::TextDelta("Edited src/lib.rs: first attempt".into()),
                    BrainEvent::Done(StopReason::EndTurn),
                ],
                vec![
                    BrainEvent::TextDelta("✗ REWORK\n1. src/lib.rs: value is still wrong".into()),
                    BrainEvent::Done(StopReason::EndTurn),
                ],
                vec![
                    BrainEvent::TextDelta("Edited src/lib.rs: fixed attempt".into()),
                    BrainEvent::Done(StopReason::EndTurn),
                ],
                vec![
                    BrainEvent::TextDelta("✓ PASS\n(no issues found)".into()),
                    BrainEvent::Done(StopReason::EndTurn),
                ],
            ]
            .into(),
        )),
    };

    let config = Config::default();
    let mut providers: HashMap<String, Vec<Arc<dyn Brain>>> = HashMap::new();
    providers.insert("local".into(), vec![Arc::new(brain)]);
    let router = Arc::new(BasicRouter::new(&config, providers));
    let db = temp_path("orchestrator-gate-db").join("memory.db");
    let memory: Arc<dyn Memory> = Arc::new(SqliteMemory::open(&db).unwrap());
    let orchestrator = DefaultOrchestrator::new(router, config, memory);

    let (tx, mut rx) = mpsc::unbounded_channel();
    let outcome = orchestrator
        .run_swarm(
            SwarmPlan {
                task: "fix value".into(),
                workspace: workspace.clone(),
                max_reworks: 1,
            },
            tx,
        )
        .await
        .unwrap();

    let mut events = Vec::new();
    while let Some(event) = rx.recv().await {
        events.push(event);
    }

    assert_eq!(outcome.status, "PASS");
    let pass_index = events
        .iter()
        .position(|event| {
            matches!(
                event,
                Event::TestResult {
                    passed: 1,
                    failed: 0,
                    ..
                }
            )
        })
        .expect("verifier PASS event should be emitted");
    let first_applied_index = events
        .iter()
        .position(|event| matches!(event, Event::DiffApplied { .. }))
        .expect("diff should be marked applied after PASS");
    assert!(first_applied_index > pass_index);

    let _ = std::fs::remove_dir_all(workspace);
    if let Some(parent) = db.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}
