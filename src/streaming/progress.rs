//! Progress tracking for streaming tasks.
//!
//! Thin wrappers around indicatif for styled progress bars and spinners.

use indicatif::{ProgressBar as IndiBar, ProgressStyle};

/// A styled progress bar for tracking task completion.
pub struct ProgressBar {
    bar: IndiBar,
}

impl ProgressBar {
    /// Create a new progress bar with the given total steps.
    pub fn new(total: u64) -> Self {
        let bar = IndiBar::new(total);
        bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.cyan} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("▰▱"),
        );
        Self { bar }
    }

    /// Set the current position.
    pub fn set_position(&self, pos: u64) {
        self.bar.set_position(pos);
    }

    /// Set the status message.
    pub fn set_message(&self, msg: String) {
        self.bar.set_message(msg);
    }

    /// Mark the bar as finished with a message.
    pub fn finish_with_message(&self, msg: String) {
        self.bar.finish_with_message(msg);
    }

    /// Get the inner indicatif bar.
    pub fn inner(&self) -> &IndiBar {
        &self.bar
    }

    /// Add this bar to a MultiProgress and return the bar.
    pub fn add_to(&self, multi: &indicatif::MultiProgress) -> IndiBar {
        let bar = multi.add(self.bar.clone());
        bar
    }
}

/// A styled spinner for indeterminate operations.
pub struct Spinner {
    spinner: indicatif::ProgressBar,
}

impl Spinner {
    /// Create a new spinner with a message.
    pub fn new(message: &str) -> Self {
        let spinner = indicatif::ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.cyan} {msg}")
                .unwrap(),
        );
        spinner.set_message(message.to_string());
        Self { spinner }
    }

    /// Update the spinner message.
    pub fn set_message(&self, msg: &str) {
        self.spinner.set_message(msg.to_string());
    }

    /// Finish the spinner with a completion message.
    pub fn finish_with_message(&self, msg: &str) {
        self.spinner.finish_with_message(msg.to_string());
    }

    /// Get the inner spinner.
    pub fn inner(&self) -> &indicatif::ProgressBar {
        &self.spinner
    }
}

/// Manages multiple progress bars displayed concurrently.
pub struct MultiProgress {
    inner: indicatif::MultiProgress,
}

impl MultiProgress {
    /// Create a new MultiProgress.
    pub fn new() -> Self {
        Self {
            inner: indicatif::MultiProgress::new(),
        }
    }

    /// Add a spinner child to the multi-progress.
    pub fn add_spinner(&self, spinner: &Spinner) {
        self.inner.add(spinner.inner().clone());
    }

    /// Get the inner indicatif MultiProgress.
    pub fn inner(&self) -> &indicatif::MultiProgress {
        &self.inner
    }
}

impl Default for MultiProgress {
    fn default() -> Self {
        Self::new()
    }
}
