#![allow(clippy::bool_assert_comparison)]

use async_trait::async_trait;
use futures::stream;
use sparrow::config::Config;
use sparrow::engine::{Engine, Task};
use sparrow::event::{Event, StopReason};
use sparrow::provider::{Brain, BrainEvent, BrainRequest, BrainStream, LatencyClass, ModelCaps};
use sparrow::router::BasicRouter;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::mpsc;

static CURRENT_DIR_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A brain whose `complete()` blocks far longer than any sane wall cap, to
/// prove the engine bounds the model call itself (not just the stream).
#[derive(Clone)]
struct SlowBrain {
    id: String,
}

#[async_trait]
impl Brain for SlowBrain {
    fn id(&self) -> &str {
        &self.id
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
        // Simulate a slow/hung provider that never starts streaming.
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        Ok(Box::pin(stream::iter(vec![BrainEvent::Done(
            StopReason::EndTurn,
        )])))
    }
}

#[tokio::test]
async fn wall_clock_cap_stops_a_slow_model_call() {
    // Regression: --max-wall-secs was parsed but never enforced. A hung/slow
    // completion must be interrupted near the cap, not run unbounded.
    let mut config = Config::default();
    config.budget.max_wall_secs = Some(1);
    let mut providers: HashMap<String, Vec<Arc<dyn Brain>>> = HashMap::new();
    providers.insert(
        "local".into(),
        vec![Arc::new(SlowBrain {
            id: "local:slow".into(),
        })],
    );
    let router = Arc::new(BasicRouter::new(&config, providers));
    let engine = Engine::new(router, config);

    let started = std::time::Instant::now();
    let (outcome, _events) = drive_and_collect(engine, "do something slow").await;
    let elapsed = started.elapsed();

    assert!(
        outcome.status.contains("wall-clock") || outcome.status.starts_with("error"),
        "expected a wall-clock stop, got {:?}",
        outcome.status
    );
    assert!(
        elapsed < std::time::Duration::from_secs(10),
        "run should stop near the 1s cap, took {:?}",
        elapsed
    );
}

#[derive(Clone)]
struct ScriptedBrain {
    id: String,
    responses: Arc<StdMutex<VecDeque<Vec<BrainEvent>>>>,
    calls: Arc<StdMutex<usize>>,
}

impl ScriptedBrain {
    fn new(responses: Vec<Vec<BrainEvent>>) -> Self {
        Self {
            id: "local:scripted".into(),
            responses: Arc::new(StdMutex::new(responses.into())),
            calls: Arc::new(StdMutex::new(0)),
        }
    }

    fn call_count(&self) -> usize {
        *self.calls.lock().unwrap()
    }
}

#[async_trait]
impl Brain for ScriptedBrain {
    fn id(&self) -> &str {
        &self.id
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
        *self.calls.lock().unwrap() += 1;
        let events = self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| vec![BrainEvent::Done(StopReason::EndTurn)]);
        Ok(Box::pin(stream::iter(events)))
    }
}

fn temp_workspace(name: &str) -> PathBuf {
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("sparrow-{name}-{id}"))
}

fn init_git_repo(root: &Path) {
    std::fs::create_dir_all(root).unwrap();
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(root)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@sparrow.dev"])
        .current_dir(root)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Sparrow Test"])
        .current_dir(root)
        .output()
        .unwrap();
}

fn engine_for(brain: ScriptedBrain) -> Engine {
    let config = Config::default();
    let mut providers: HashMap<String, Vec<Arc<dyn Brain>>> = HashMap::new();
    providers.insert("local".into(), vec![Arc::new(brain)]);
    let router = Arc::new(BasicRouter::new(&config, providers));
    Engine::new(router, config)
}

async fn drive_and_collect(
    engine: Engine,
    task: &str,
) -> (sparrow::event::OutcomeSummary, Vec<Event>) {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let outcome = engine
        .drive(
            Task {
                description: task.into(),
                context: vec![],
            },
            tx,
        )
        .await
        .unwrap();
    let mut events = Vec::new();
    while let Some(event) = rx.recv().await {
        events.push(event);
    }
    (outcome, events)
}

#[tokio::test]
async fn anti_simulation_rejects_fabricated_result_claim_and_reasks() {
    let brain = ScriptedBrain::new(vec![
        vec![
            BrainEvent::TextDelta("All tests pass, build succeeds, no errors.".into()),
            BrainEvent::Done(StopReason::EndTurn),
        ],
        vec![
            BrainEvent::TextDelta(
                "I need to execute the relevant tool before reporting results.".into(),
            ),
            BrainEvent::Done(StopReason::EndTurn),
        ],
    ]);
    let calls = brain.clone();
    let engine = engine_for(brain);

    let (outcome, events) =
        drive_and_collect(engine, "run cargo test and report the real result").await;

    // The run finishes without error. Both turns are text-only (no tools, no
    // diffs), so the honest status is "no actions taken" — not "completed",
    // which is now reserved for runs that actually changed something.
    assert!(
        !outcome.status.starts_with("error"),
        "run should not error, got {:?}",
        outcome.status
    );
    // The guard must have re-asked, so the model was called a second time.
    assert_eq!(calls.call_count(), 2);
    assert!(events.iter().any(|event| matches!(
        event,
        Event::Message { role, text, .. }
            if role == "guard" && text.contains("anti-simulation")
    )));
}

#[tokio::test]
async fn checkpoint_is_created_before_mutating_edit_tool_runs() {
    let _guard = CURRENT_DIR_LOCK.lock().await;
    let workspace = temp_workspace("checkpoint-before-edit");
    init_git_repo(&workspace);
    std::fs::write(workspace.join("note.txt"), "original").unwrap();
    std::process::Command::new("git")
        .args(["add", "note.txt"])
        .current_dir(&workspace)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&workspace)
        .output()
        .unwrap();

    let previous_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&workspace).unwrap();

    let brain = ScriptedBrain::new(vec![
        vec![
            BrainEvent::ToolUseStart {
                id: "edit-1".into(),
                name: "edit".into(),
            },
            BrainEvent::ToolUseDelta {
                id: "edit-1".into(),
                json: r#"{"path":"note.txt","old":"original","new":"changed"}"#.into(),
            },
            BrainEvent::ToolUseEnd {
                id: "edit-1".into(),
            },
            BrainEvent::Done(StopReason::ToolUse),
        ],
        vec![
            BrainEvent::TextDelta("Edited note.txt using the tool result.".into()),
            BrainEvent::Done(StopReason::EndTurn),
        ],
    ]);
    let engine = engine_for(brain);
    let (outcome, events) = drive_and_collect(engine, "edit note.txt and replace original").await;
    std::env::set_current_dir(previous_dir).unwrap();

    assert_eq!(outcome.status, "completed");
    assert_eq!(
        std::fs::read_to_string(workspace.join("note.txt")).unwrap(),
        "changed"
    );

    let checkpoint_index = events
        .iter()
        .position(|event| matches!(event, Event::CheckpointCreated { .. }))
        .expect("mutating edit should create a checkpoint");
    let tool_started_index = events
        .iter()
        .position(|event| matches!(event, Event::ToolUseStarted { id, .. } if id == "edit-1"))
        .expect("edit tool should start");
    assert!(checkpoint_index < tool_started_index);
    assert!(events.iter().any(|event| matches!(
        event,
        Event::Message { role, text, .. }
            if role == "autonomy" && text.contains("trusted autonomy")
    )));

    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn no_checkpoint_setting_skips_checkpoint_creation() {
    // Regression: `--no-checkpoint` (config.defaults.checkpointing = false) was
    // parsed but never enforced. With it off, no CheckpointCreated event fires
    // — yet the edit still applies.
    let _guard = CURRENT_DIR_LOCK.lock().await;
    let workspace = temp_workspace("no-checkpoint");
    init_git_repo(&workspace);
    std::fs::write(workspace.join("note.txt"), "original").unwrap();
    std::process::Command::new("git")
        .args(["add", "note.txt"])
        .current_dir(&workspace)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&workspace)
        .output()
        .unwrap();

    let previous_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&workspace).unwrap();

    let brain = ScriptedBrain::new(vec![
        vec![
            BrainEvent::ToolUseStart {
                id: "edit-1".into(),
                name: "edit".into(),
            },
            BrainEvent::ToolUseDelta {
                id: "edit-1".into(),
                json: r#"{"path":"note.txt","old":"original","new":"changed"}"#.into(),
            },
            BrainEvent::ToolUseEnd {
                id: "edit-1".into(),
            },
            BrainEvent::Done(StopReason::ToolUse),
        ],
        vec![
            BrainEvent::TextDelta("done.".into()),
            BrainEvent::Done(StopReason::EndTurn),
        ],
    ]);

    let mut config = Config::default();
    config.defaults.checkpointing = false;
    let mut providers: HashMap<String, Vec<Arc<dyn Brain>>> = HashMap::new();
    providers.insert("local".into(), vec![Arc::new(brain)]);
    let router = Arc::new(BasicRouter::new(&config, providers));
    let engine = Engine::new(router, config);

    let (outcome, events) = drive_and_collect(engine, "edit note.txt").await;
    std::env::set_current_dir(previous_dir).unwrap();

    assert_eq!(outcome.status, "completed");
    // The edit still applied…
    assert_eq!(
        std::fs::read_to_string(workspace.join("note.txt")).unwrap(),
        "changed"
    );
    // …but no checkpoint was created.
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::CheckpointCreated { .. })),
        "no checkpoint should be created when checkpointing is disabled"
    );

    let _ = std::fs::remove_dir_all(workspace);
}
