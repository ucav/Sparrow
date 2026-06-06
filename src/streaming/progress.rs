// ─── Progress tracking ───────────────────────────────────────────────────────
//
// Wrappers around indicatif for Sparrow-styled progress bars, spinners,
// and multi-progress displays (htop-like lanes).

use std::sync::Arc;
use std::time::{Duration, Instant};

use indicatif::{ProgressBar as IndiBar, ProgressStyle, MultiProgress as IndiMulti};

// ─── Task progress model ─────────────────────────────────────────────────────

/// Tracks progress of a multi-step task.
#[derive(Debug, Clone)]
pub struct TaskProgress {
    /// Total number of steps.
    pub total: u64,
    /// Current step (0-based).
    pub current: u64,
    /// Human-readable description.
    pub description: String,
    /// Time since task started.
    pub started_at: Instant,
}

impl TaskProgress {
    pub fn new(total: u64, description: impl Into<String>) -> Self {
        Self {
            total,
            current: 0,
            description: description.into(),
            started_at: Instant::now(),
        }
    }

    pub fn advance(&mut self, by: u64) {
        self.current = (self.current + by).min(self.total);
    }

    pub fn set(&mut self, current: u64) {
        self.current = current.min(self.total);
    }

    pub fn percent(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.current as f64 / self.total as f64) * 100.0
        }
    }

    pub fn is_done(&self) -> bool {
        self.current >= self.total
    }

    pub fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }

    /// Estimated time remaining.
    pub fn eta(&self) -> Option<Duration> {
        if self.current == 0 || self.total == 0 {
            return None;
        }
        let elapsed = self.elapsed();
        let per_step = elapsed.as_secs_f64() / self.current as f64;
        let remaining_steps = self.total - self.current;
        Some(Duration::from_secs_f64(per_step * remaining_steps as f64))
    }
}

// ─── Progress bar wrapper ────────────────────────────────────────────────────

/// Sparrow-styled progress bar wrapping indicatif.
#[derive(Clone)]
pub struct ProgressBar {
    inner: Arc<IndiBar>,
}

impl ProgressBar {
    pub fn new(total: u64) -> Self {
        let bar = IndiBar::new(total);
        bar.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}",
            )
            .unwrap()
            .progress_chars("▰▱"),
        );
        Self { inner: Arc::new(bar) }
    }

    pub fn set_style(&self, chars: &str) {
        let style = ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}",
        )
        .unwrap()
        .progress_chars(chars);
        self.inner.set_style(style);
    }

    pub fn set_position(&self, pos: u64) {
        self.inner.set_position(pos);
    }

    pub fn set_message(&self, msg: impl Into<String>) {
        self.inner.set_message(msg.into());
    }

    pub fn inc(&self, delta: u64) {
        self.inner.inc(delta);
    }

    pub fn finish_with_message(&self, msg: impl Into<String>) {
        self.inner.finish_with_message(msg.into());
    }

    pub fn finish_and_clear(&self) {
        self.inner.finish_and_clear();
    }

    pub fn println(&self, msg: impl Into<String>) {
        self.inner.println(msg.into());
    }

    /// Get the inner indicatif bar (for MultiProgress integration).
    pub fn inner(&self) -> &IndiBar {
        &self.inner
    }
}

// ─── Spinner wrapper ─────────────────────────────────────────────────────────

/// Sparrow-styled spinner wrapping indicatif.
#[derive(Clone)]
pub struct Spinner {
    inner: Arc<IndiBar>,
}

impl Spinner {
    pub fn new(msg: &str) -> Self {
        let spinner = IndiBar::new_spinner();
        spinner.set_style(
            ProgressStyle::with_template("{spinner:.green} {msg}")
                .unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        spinner.set_message(msg.to_string());
        spinner.enable_steady_tick(Duration::from_millis(80));
        Self {
            inner: Arc::new(spinner),
        }
    }

    pub fn set_message(&self, msg: impl Into<String>) {
        self.inner.set_message(msg.into());
    }

    pub fn message(&self) -> String {
        self.inner.message()
    }

    pub fn finish_with_message(&self, msg: impl Into<String>) {
        self.inner.finish_with_message(msg.into());
    }

    pub fn finish_and_clear(&self) {
        self.inner.finish_and_clear();
    }

    pub fn println(&self, msg: impl Into<String>) {
        self.inner.println(msg.into());
    }

    /// Get the inner indicatif bar.
    pub fn inner(&self) -> &IndiBar {
        &self.inner
    }
}

// ─── Multi-progress ──────────────────────────────────────────────────────────

/// Container for multiple concurrent progress bars / spinners (htop-style lanes).
pub struct MultiProgress {
    inner: IndiMulti,
}

impl MultiProgress {
    pub fn new() -> Self {
        Self {
            inner: IndiMulti::new(),
        }
    }

    /// Add a spinner as a child lane.
    pub fn add_child(&self, spinner: Spinner) {
        self.inner.add(spinner.inner().clone());
    }

    /// Add a progress bar as a child lane.
    pub fn add_bar(&self, bar: ProgressBar) {
        self.inner.add(bar.inner().clone());
    }

    /// Remove a child from the display.
    pub fn remove_child(&self, bar: &IndiBar) {
        self.inner.remove(bar);
    }

    /// Get the inner indicatif MultiProgress.
    pub fn inner(&self) -> &IndiMulti {
        &self.inner
    }

    /// Clear all children and finish.
    pub fn clear(&self) -> Result<(), indicatif::MultiProgressClearError> {
        self.inner.clear()
    }
}

impl Default for MultiProgress {
    fn default() -> Self {
        Self::new()
    }
}
