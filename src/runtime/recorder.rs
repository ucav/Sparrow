use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::event::Event;

// ─── Transcript ─────────────────────────────────────────────────────────────────

/// A run transcript: inputs.json + events.jsonl + checkpoint refs.
/// Makes runs replayable, diffable, shareable, auditable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcript {
    pub run_id: String,
    pub inputs: RunInputs,
    pub events: Vec<Event>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunInputs {
    pub task: String,
    pub config_snapshot: serde_json::Value,
    pub model_id: String,
    pub repo_head: Option<String>,
    pub timestamp: String,
    pub agent: String,
}

impl Transcript {
    pub fn new(run_id: String, inputs: RunInputs) -> Self {
        Self {
            run_id,
            inputs,
            events: Vec::new(),
            created_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        }
    }

    pub fn push_event(&mut self, event: &Event) {
        self.events.push(event.clone());
    }

    pub fn event_count(&self) -> usize {
        self.events.len()
    }
}

// ─── THE RECORDER TRAIT ─────────────────────────────────────────────────────────

pub trait Recorder: Send + Sync {
    fn record(&self, event: &Event);
    fn finalize(&self, run_id: &str) -> anyhow::Result<Transcript>;
}

// ─── THE REPLAYER TRAIT ─────────────────────────────────────────────────────────

pub trait Replayer: Send + Sync {
    fn load(&self, run_id: &str) -> Option<Transcript>;
    fn list_transcripts(&self) -> Vec<String>;
    fn delete(&self, run_id: &str) -> anyhow::Result<()>;
}

// ─── Filesystem-backed recorder/replayer ────────────────────────────────────────

pub struct FsRecorder {
    transcripts_dir: PathBuf,
    active: std::sync::Mutex<HashMap<String, Transcript>>,
}

impl FsRecorder {
    pub fn new(transcripts_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&transcripts_dir).ok();
        Self {
            transcripts_dir,
            active: std::sync::Mutex::new(HashMap::new()),
        }
    }

    pub fn start_run(&self, run_id: String, inputs: RunInputs) {
        let transcript = Transcript::new(run_id, inputs);
        self.active
            .lock()
            .unwrap()
            .insert(transcript.run_id.clone(), transcript);
    }

    fn run_dir(&self, run_id: &str) -> PathBuf {
        self.transcripts_dir.join(run_id)
    }
}

impl Recorder for FsRecorder {
    fn record(&self, event: &Event) {
        let run_id = event_run_id(event);
        if let Some(transcript) = self.active.lock().unwrap().get_mut(run_id) {
            transcript.push_event(event);
        }
    }

    fn finalize(&self, run_id: &str) -> anyhow::Result<Transcript> {
        let transcript = self
            .active
            .lock()
            .unwrap()
            .remove(run_id)
            .ok_or_else(|| anyhow::anyhow!("No active transcript for {}", run_id))?;

        let run_dir = self.run_dir(run_id);
        std::fs::create_dir_all(&run_dir)?;

        // Write inputs.json
        let inputs_json = serde_json::to_string_pretty(&transcript.inputs)?;
        std::fs::write(run_dir.join("inputs.json"), inputs_json)?;

        // Write events.jsonl
        let mut events_jsonl = String::new();
        for event in &transcript.events {
            events_jsonl.push_str(&serde_json::to_string(event)?);
            events_jsonl.push('\n');
        }
        std::fs::write(run_dir.join("events.jsonl"), events_jsonl)?;

        // Write metadata
        let meta = serde_json::json!({
            "run_id": transcript.run_id,
            "event_count": transcript.events.len(),
            "created_at": transcript.created_at,
        });
        std::fs::write(
            run_dir.join("meta.json"),
            serde_json::to_string_pretty(&meta)?,
        )?;

        tracing::info!(
            "Transcript saved: {} ({} events)",
            run_id,
            transcript.events.len()
        );

        Ok(transcript)
    }
}

fn event_run_id(event: &Event) -> &str {
    match event {
        Event::RunStarted { run, .. }
        | Event::RouteSelected { run, .. }
        | Event::ModelSwitched { run, .. }
        | Event::ThinkingDelta { run, .. }
        | Event::Message { run, .. }
        | Event::ToolUseProposed { run, .. }
        | Event::ApprovalRequested { run, .. }
        | Event::ApprovalResolved { run, .. }
        | Event::ToolUseStarted { run, .. }
        | Event::ToolOutput { run, .. }
        | Event::DiffProposed { run, .. }
        | Event::DiffApplied { run, .. }
        | Event::TestResult { run, .. }
        | Event::AgentSpawned { run, .. }
        | Event::AgentStatus { run, .. }
        | Event::CheckpointCreated { run, .. }
        | Event::SkillLearned { run, .. }
        | Event::CostUpdate { run, .. }
        | Event::TokenUsage { run, .. }
        | Event::TokenUsageEstimated { run, .. }
        | Event::AutonomyChanged { run, .. }
        | Event::RunFinished { run, .. }
        | Event::Error { run, .. } => &run.0,
    }
}

impl Replayer for FsRecorder {
    fn load(&self, run_id: &str) -> Option<Transcript> {
        let run_dir = self.run_dir(run_id);
        if !run_dir.exists() {
            return None;
        }

        let inputs: RunInputs =
            serde_json::from_str(&std::fs::read_to_string(run_dir.join("inputs.json")).ok()?)
                .ok()?;

        let events_text = std::fs::read_to_string(run_dir.join("events.jsonl")).ok()?;
        let events: Vec<Event> = events_text
            .lines()
            .filter(|l| !l.is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();

        Some(Transcript {
            run_id: run_id.to_string(),
            inputs,
            events,
            created_at: String::new(),
        })
    }

    fn list_transcripts(&self) -> Vec<String> {
        let mut ids = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.transcripts_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        ids.push(name.to_string());
                    }
                }
            }
        }
        ids.sort();
        ids
    }

    fn delete(&self, run_id: &str) -> anyhow::Result<()> {
        let run_dir = self.run_dir(run_id);
        if run_dir.exists() {
            std::fs::remove_dir_all(&run_dir)?;
        }
        Ok(())
    }
}
