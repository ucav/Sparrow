//! Reproduction: a single model turn emitting N native tool calls through the
//! OpenAI-compatible adapter arrives at the engine as
//!   Start(0) · Delta(0) · Start(1) · Delta(1) · … · End(j) · End(i)
//! because `openai_compat.rs` only emits ToolUseEnd at `finish_reason:
//! "tool_calls"`, draining a HashMap (arbitrary order). DeepSeek/Qwen
//! thinking-mode models emit multi-tool turns constantly.
//!
//! The engine keeps ONE `current_tool_name` / `current_tool_json` buffer and
//! ignores the `id` on ToolUseDelta, so the second Start wipes the first
//! call's streamed arguments.

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

#[derive(Clone)]
struct ScriptedBrain {
    id: String,
    responses: Arc<StdMutex<VecDeque<Vec<BrainEvent>>>>,
}

impl ScriptedBrain {
    fn new(responses: Vec<Vec<BrainEvent>>) -> Self {
        Self {
            id: "local:scripted".into(),
            responses: Arc::new(StdMutex::new(responses.into())),
        }
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
    for args in [
        vec!["init"],
        vec!["config", "user.email", "test@sparrow.dev"],
        vec!["config", "user.name", "Sparrow Test"],
    ] {
        std::process::Command::new("git")
            .args(&args)
            .current_dir(root)
            .output()
            .unwrap();
    }
}

fn engine_for(brain: ScriptedBrain) -> Engine {
    let config = Config::default();
    let mut providers: HashMap<String, Vec<Arc<dyn Brain>>> = HashMap::new();
    providers.insert("local".into(), vec![Arc::new(brain)]);
    let router = Arc::new(BasicRouter::new(&config, providers));
    Engine::new(router, config)
}

/// Two edits streamed the way the OpenAI-compat adapter actually delivers a
/// 2-tool turn (Ends grouped at finish_reason, reverse drain order is legal).
/// Both files must end up edited with the arguments each call streamed.
#[tokio::test]
async fn two_native_tool_calls_in_one_turn_both_execute_with_their_own_args() {
    let workspace = temp_workspace("multi-tool-turn");
    init_git_repo(&workspace);
    std::fs::write(workspace.join("a.txt"), "alpha").unwrap();
    std::fs::write(workspace.join("b.txt"), "beta").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&workspace)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&workspace)
        .output()
        .unwrap();
    std::env::set_current_dir(&workspace).unwrap();

    let brain = ScriptedBrain::new(vec![
        vec![
            BrainEvent::ToolUseStart {
                id: "call-a".into(),
                name: "edit".into(),
            },
            BrainEvent::ToolUseDelta {
                id: "call-a".into(),
                json: r#"{"path":"a.txt","old":"alpha","new":"ALPHA"}"#.into(),
            },
            BrainEvent::ToolUseStart {
                id: "call-b".into(),
                name: "edit".into(),
            },
            BrainEvent::ToolUseDelta {
                id: "call-b".into(),
                json: r#"{"path":"b.txt","old":"beta","new":"BETA"}"#.into(),
            },
            // finish_reason: "tool_calls" → adapter drains its HashMap; with 2+
            // entries either order can come out. Worst case: b before a.
            BrainEvent::ToolUseEnd {
                id: "call-b".into(),
            },
            BrainEvent::ToolUseEnd {
                id: "call-a".into(),
            },
            BrainEvent::Done(StopReason::ToolUse),
        ],
        vec![
            BrainEvent::TextDelta("Both files updated.".into()),
            BrainEvent::Done(StopReason::EndTurn),
        ],
    ]);
    let engine = engine_for(brain);

    let (tx, mut rx) = mpsc::unbounded_channel();
    let outcome = engine
        .drive(
            Task {
                description: "rename markers in both files".into(),
                context: vec![],
            },
            tx,
        )
        .await
        .unwrap();
    let mut events: Vec<Event> = Vec::new();
    while let Some(event) = rx.recv().await {
        events.push(event);
    }

    assert_eq!(outcome.status, "completed");

    let a = std::fs::read_to_string(workspace.join("a.txt")).unwrap();
    let b = std::fs::read_to_string(workspace.join("b.txt")).unwrap();
    assert_eq!(
        (a.as_str(), b.as_str()),
        ("ALPHA", "BETA"),
        "each tool call must run with the arguments IT streamed; events seen: {:#?}",
        events
            .iter()
            .filter(|e| matches!(
                e,
                Event::ToolUseStarted { .. } | Event::ToolUseProposed { .. }
            ))
            .collect::<Vec<_>>()
    );
}
