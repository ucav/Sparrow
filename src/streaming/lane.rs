//! Multi-agent lane display for streaming output.
//!
//! Shows multiple concurrent agent progress lanes in the terminal,
//! similar to `docker compose logs` or `htop`.

use std::collections::HashMap;
use std::time::Instant;

/// A single agent lane showing current action and elapsed time.
struct AgentLane {
    name: String,
    status: String,
    started: Instant,
}

/// Displays multiple agent lanes concurrently.
pub struct LaneDisplay {
    lanes: HashMap<String, AgentLane>,
}

impl LaneDisplay {
    /// Create a new lane display.
    pub fn new() -> Self {
        Self {
            lanes: HashMap::new(),
        }
    }

    /// Add a new lane for an agent.
    pub fn add_lane(&mut self, name: &str) {
        self.lanes.insert(
            name.to_string(),
            AgentLane {
                name: name.to_string(),
                status: "starting...".to_string(),
                started: Instant::now(),
            },
        );
    }

    /// Update the status of an agent lane.
    pub fn update_lane(&mut self, name: &str, status: &str) {
        if let Some(lane) = self.lanes.get_mut(name) {
            lane.status = status.to_string();
        }
    }

    /// Remove an agent lane.
    pub fn remove_lane(&mut self, name: &str) {
        self.lanes.remove(name);
    }

    /// Render all lanes to the terminal.
    pub fn render(&self) {
        // Sort by name for consistent display
        let mut sorted: Vec<&AgentLane> = self.lanes.values().collect();
        sorted.sort_by(|a, b| a.name.cmp(&b.name));

        for lane in &sorted {
            let elapsed = lane.started.elapsed().as_secs();
            eprintln!(
                "  \x1b[36m[{elapsed:>3}s]\x1b[0m \x1b[1m{}\x1b[0m — {}",
                lane.name, lane.status
            );
        }
    }

    /// Check if there are any active lanes.
    pub fn is_empty(&self) -> bool {
        self.lanes.is_empty()
    }

    /// Get the number of active lanes.
    pub fn len(&self) -> usize {
        self.lanes.len()
    }
}

impl Default for LaneDisplay {
    fn default() -> Self {
        Self::new()
    }
}
