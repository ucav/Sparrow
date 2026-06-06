// ─── Multi-agent lane display ────────────────────────────────────────────────
//
// Displays multiple concurrent agent progress lanes in a htop-style layout.
// Each lane shows: agent name + current action + spinner + elapsed time.

use std::collections::HashMap;
use std::time::Instant;

use console::Term;

use crate::streaming::progress::{MultiProgress, Spinner};

// ─── Individual lane ─────────────────────────────────────────────────────────

/// State of a single agent lane.
#[derive(Debug, Clone)]
struct Lane {
    /// Agent display name.
    name: String,
    /// Current status/action text.
    status: String,
    /// When the lane was created.
    created_at: Instant,
    /// Last update time.
    updated_at: Instant,
    /// Associated spinner in the multi-progress display.
    spinner: Spinner,
}

impl Lane {
    fn new(name: &str) -> Self {
        let now = Instant::now();
        Self {
            name: name.to_string(),
            status: "initializing…".to_string(),
            created_at: now,
            updated_at: now,
            spinner: Spinner::new(&format!("{:12} {}", name, "initializing…")),
        }
    }

    fn update_status(&mut self, status: &str) {
        self.status = status.to_string();
        self.updated_at = Instant::now();
        self.spinner
            .set_message(format!("{:12} {}", self.name, status));
    }

    fn elapsed(&self) -> std::time::Duration {
        self.updated_at - self.created_at
    }
}

// ─── Lane display ────────────────────────────────────────────────────────────

/// Manages multiple concurrent agent progress lanes.
pub struct LaneDisplay {
    /// Active lanes indexed by agent name.
    lanes: HashMap<String, Lane>,
    /// Multi-progress container for rendering.
    multi: MultiProgress,
    /// Terminal to query size.
    term: Term,
    /// Maximum number of lanes to show (based on terminal height).
    max_lanes: usize,
}

impl LaneDisplay {
    /// Create a new lane display. Auto-caps lanes at (terminal_height - 4) / 2.
    pub fn new() -> Self {
        let term = Term::stdout();
        let height = term.size_checked().map(|(h, _)| h as usize).unwrap_or(24);
        let max_lanes = (height.saturating_sub(4) / 2).max(3);
        Self {
            lanes: HashMap::new(),
            multi: MultiProgress::new(),
            term,
            max_lanes,
        }
    }

    /// Add a new agent lane.
    pub fn add_lane(&mut self, name: &str) {
        if self.lanes.contains_key(name) {
            return;
        }
        // If at capacity, remove the oldest idle lane
        if self.lanes.len() >= self.max_lanes {
            self.evict_oldest();
        }
        let lane = Lane::new(name);
        self.multi.add_child(lane.spinner.clone());
        self.lanes.insert(name.to_string(), lane);
    }

    /// Update the status text for an existing lane. Creates the lane if missing.
    pub fn update_lane(&mut self, name: &str, status: &str) {
        if let Some(lane) = self.lanes.get_mut(name) {
            lane.update_status(status);
        } else {
            // Auto-create lane if it doesn't exist
            self.add_lane(name);
            if let Some(lane) = self.lanes.get_mut(name) {
                lane.update_status(status);
            }
        }
    }

    /// Remove a lane from the display.
    pub fn remove_lane(&mut self, name: &str) {
        if let Some(lane) = self.lanes.remove(name) {
            lane.spinner.finish_with_message(format!(
                "{:12} ✓ done ({:.1}s)",
                name,
                lane.elapsed().as_secs_f64()
            ));
        }
    }

    /// Mark a lane as complete (shows checkmark and elapsed).
    pub fn complete_lane(&mut self, name: &str) {
        if let Some(lane) = self.lanes.remove(name) {
            lane.spinner.finish_with_message(format!(
                "{:12} ✓ done ({:.1}s)",
                name,
                lane.elapsed().as_secs_f64()
            ));
        }
    }

    /// Mark a lane as errored.
    pub fn error_lane(&mut self, name: &str, error: &str) {
        if let Some(lane) = self.lanes.remove(name) {
            lane.spinner.finish_with_message(format!(
                "{:12} ✗ {} ({:.1}s)",
                name,
                error,
                lane.elapsed().as_secs_f64()
            ));
        }
    }

    /// Get the count of active lanes.
    pub fn lane_count(&self) -> usize {
        self.lanes.len()
    }

    /// Check if a lane exists.
    pub fn has_lane(&self, name: &str) -> bool {
        self.lanes.contains_key(name)
    }

    /// Remove the oldest lane (by creation time).
    fn evict_oldest(&mut self) {
        let oldest = self
            .lanes
            .iter()
            .min_by_key(|(_, l)| l.created_at)
            .map(|(k, _)| k.clone());

        if let Some(name) = oldest {
            self.remove_lane(&name);
        }
    }

    /// Finish all lanes and clear the display.
    pub fn finish_all(&mut self) {
        let names: Vec<String> = self.lanes.keys().cloned().collect();
        for name in names {
            self.complete_lane(&name);
        }
    }
}

impl Default for LaneDisplay {
    fn default() -> Self {
        Self::new()
    }
}
