use tokio::sync::broadcast;

use crate::event::Event;

// ─── Event bus ──────────────────────────────────────────────────────────────────

/// Centralized pub/sub event distribution.
/// Every surface and the recorder subscribe to this.
pub struct EventBus {
    /// Global broadcast channel for all events
    tx: broadcast::Sender<Event>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Publish an event to all subscribers
    pub fn publish(&self, event: Event) {
        let _ = self.tx.send(event);
    }

    /// Subscribe to all events
    pub fn subscribe_all(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }

    /// Subscribe with a filter (by run_id or event type)
    pub fn subscribe_filtered(&self, _filter: &EventFilter) -> broadcast::Receiver<Event> {
        // For M4, we return the global channel.
        // Filtering happens on the receiver side.
        self.tx.subscribe()
    }

    /// Create a filtered subscription by run_id
    pub fn subscribe_run(&self, _run_id: &crate::event::RunId) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
        // Note: actual filtering is done by the receiver checking event.run field
    }

    /// Active subscriber count
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(1024)
    }
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}

// ─── Event filter ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EventFilter {
    pub run_id: Option<String>,
    pub event_types: Vec<String>,
}

impl EventFilter {
    pub fn by_run(run_id: &str) -> Self {
        Self {
            run_id: Some(run_id.to_string()),
            event_types: vec![],
        }
    }

    pub fn matches(&self, event: &Event) -> bool {
        // Check run_id filter
        if let Some(ref rid) = self.run_id {
            let event_run = match event {
                Event::RunStarted { run, .. } => &run.0,
                Event::RouteSelected { run, .. } => &run.0,
                Event::ModelSwitched { run, .. } => &run.0,
                Event::ThinkingDelta { run, .. } => &run.0,
                Event::ReasoningDelta { run, .. } => &run.0,
                Event::Message { run, .. } => &run.0,
                Event::ToolUseProposed { run, .. } => &run.0,
                Event::ApprovalRequested { run, .. } => &run.0,
                Event::ApprovalResolved { run, .. } => &run.0,
                Event::ToolUseStarted { run, .. } => &run.0,
                Event::ToolOutput { run, .. } => &run.0,
                Event::DiffProposed { run, .. } => &run.0,
                Event::DiffApplied { run, .. } => &run.0,
                Event::TestResult { run, .. } => &run.0,
                Event::AgentSpawned { run, .. } => &run.0,
                Event::AgentStatus { run, .. } => &run.0,
                Event::CheckpointCreated { run, .. } => &run.0,
                Event::SkillLearned { run, .. } => &run.0,
                Event::CostUpdate { run, .. } => &run.0,
                Event::TokenUsage { run, .. } => &run.0,
                Event::TokenUsageEstimated { run, .. } => &run.0,
                Event::AutonomyChanged { run, .. } => &run.0,
                Event::RunFinished { run, .. } => &run.0,
                Event::Error { run, .. } => &run.0,
                Event::Compacted { run, .. } => &run.0,
                // Global event with no per-run id — never matches a specific
                // subscriber, so route to the empty bucket.
                Event::UpdateAvailable { .. } => "",
            };
            if event_run != rid {
                return false;
            }
        }

        // Check event type filter
        if !self.event_types.is_empty() {
            let event_type = match event {
                Event::RunStarted { .. } => "RunStarted",
                Event::RouteSelected { .. } => "RouteSelected",
                Event::ModelSwitched { .. } => "ModelSwitched",
                Event::ThinkingDelta { .. } => "ThinkingDelta",
                Event::ReasoningDelta { .. } => "ReasoningDelta",
                Event::Message { .. } => "Message",
                Event::ToolUseProposed { .. } => "ToolUseProposed",
                Event::ApprovalRequested { .. } => "ApprovalRequested",
                Event::ApprovalResolved { .. } => "ApprovalResolved",
                Event::ToolUseStarted { .. } => "ToolUseStarted",
                Event::ToolOutput { .. } => "ToolOutput",
                Event::DiffProposed { .. } => "DiffProposed",
                Event::DiffApplied { .. } => "DiffApplied",
                Event::TestResult { .. } => "TestResult",
                Event::AgentSpawned { .. } => "AgentSpawned",
                Event::AgentStatus { .. } => "AgentStatus",
                Event::CheckpointCreated { .. } => "CheckpointCreated",
                Event::SkillLearned { .. } => "SkillLearned",
                Event::CostUpdate { .. } => "CostUpdate",
                Event::TokenUsage { .. } => "TokenUsage",
                Event::TokenUsageEstimated { .. } => "TokenUsageEstimated",
                Event::AutonomyChanged { .. } => "AutonomyChanged",
                Event::RunFinished { .. } => "RunFinished",
                Event::Error { .. } => "Error",
                Event::Compacted { .. } => "Compacted",
                Event::UpdateAvailable { .. } => "UpdateAvailable",
            };
            if !self.event_types.contains(&event_type.to_string()) {
                return false;
            }
        }

        true
    }
}
