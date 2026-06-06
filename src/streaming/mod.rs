// ─── Streaming infrastructure ─────────────────────────────────────────────────
//
// Real-time event streaming with progress bars, spinners, and live display.
// Feeds from agent runtime into terminal surfaces.

pub mod lane;
pub mod progress;

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use console::Term;
use parking_lot::Mutex;
use tokio::sync::mpsc;

use crate::streaming::lane::LaneDisplay;
use crate::streaming::progress::{MultiProgress, ProgressBar, Spinner};

// ─── Stream event model ──────────────────────────────────────────────────────

/// Events emitted by the agent runtime for streaming surfaces.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// A tool is about to be executed.
    ToolStart {
        name: String,
        id: String,
    },
    /// Output from a completed tool execution.
    ToolOutput {
        id: String,
        content: String,
    },
    /// A message from an agent (assistant or user).
    AgentMessage {
        role: String,
        text: String,
    },
    /// Progress update for a multi-step task.
    TaskProgress {
        current: u64,
        total: u64,
        description: String,
    },
    /// A task has completed successfully.
    TaskComplete {
        summary: String,
    },
    /// An error occurred.
    Error {
        message: String,
    },
}

// ─── Channel wrappers ────────────────────────────────────────────────────────

/// Send side of the streaming channel.
#[derive(Clone)]
pub struct StreamSender {
    tx: mpsc::Sender<StreamEvent>,
}

impl StreamSender {
    pub async fn send(&self, event: StreamEvent) -> Result<()> {
        self.tx.send(event).await?;
        Ok(())
    }

    /// Non-blocking send (drops if full).
    pub fn try_send(&self, event: StreamEvent) -> Result<()> {
        self.tx.try_send(event)?;
        Ok(())
    }

    pub fn is_closed(&self) -> bool {
        self.tx.is_closed()
    }
}

/// Receive side of the streaming channel.
pub struct StreamReceiver {
    rx: mpsc::Receiver<StreamEvent>,
    display: Option<LiveDisplay>,
}

impl StreamReceiver {
    /// Create a receiver without a live display — events must be polled manually.
    pub fn bare(rx: mpsc::Receiver<StreamEvent>) -> Self {
        Self {
            rx,
            display: None,
        }
    }

    /// Create a receiver with a live display that auto-renders events.
    pub fn with_display(rx: mpsc::Receiver<StreamEvent>) -> Self {
        Self {
            rx,
            display: Some(LiveDisplay::new()),
        }
    }

    /// Await the next event.
    pub async fn recv(&mut self) -> Option<StreamEvent> {
        self.rx.recv().await
    }

    /// Receive an event and update the live display (if present).
    pub async fn recv_and_update(&mut self) -> Option<StreamEvent> {
        let event = self.rx.recv().await?;
        if let Some(ref mut display) = self.display {
            display.update(event.clone());
        }
        Some(event)
    }

    /// Drain all pending events and update display, then call `finish()`.
    /// Returns the last event seen (if any).
    pub async fn drain_and_finish(&mut self) -> Option<StreamEvent> {
        let mut last: Option<StreamEvent> = None;
        // Non-blocking drain
        loop {
            match self.rx.try_recv() {
                Ok(event) => {
                    if let Some(ref mut display) = self.display {
                        display.update(event.clone());
                    }
                    last = Some(event);
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => break,
            }
        }
        if let Some(ref mut display) = self.display {
            display.finish();
        }
        last
    }

    /// Access the live display for manual updates.
    pub fn display_mut(&mut self) -> Option<&mut LiveDisplay> {
        self.display.as_mut()
    }

    /// Finish the live display.
    pub fn finish_display(&mut self) {
        if let Some(ref mut display) = self.display {
            display.finish();
        }
    }
}

// ─── Factory ─────────────────────────────────────────────────────────────────

/// Create a new streaming channel pair.
pub fn create_channel() -> (StreamSender, StreamReceiver) {
    let (tx, rx) = mpsc::channel(256);
    let sender = StreamSender { tx };
    let receiver = StreamReceiver::bare(rx);
    (sender, receiver)
}

/// Create a channel with a live display attached.
pub fn create_channel_with_display() -> (StreamSender, StreamReceiver) {
    let (tx, rx) = mpsc::channel(256);
    let sender = StreamSender { tx };
    let receiver = StreamReceiver::with_display(rx);
    (sender, receiver)
}

// ─── Live display ────────────────────────────────────────────────────────────

/// Renders streaming events in real-time using indicatif progress bars
/// and styled console output.
pub struct LiveDisplay {
    /// Multi-progress container for spinners and bars.
    multi: MultiProgress,
    /// Spinner for the current tool being executed.
    tool_spinner: Option<Spinner>,
    /// Progress bar for multi-step tasks.
    task_bar: Option<ProgressBar>,
    /// Current tool name being shown.
    current_tool: Option<String>,
    /// Accumulated agent message text.
    agent_buffer: String,
    /// Wall-clock start time.
    start_time: Instant,
    /// Terminal width for formatting.
    term_width: u16,
    /// Task progress state.
    task_current: u64,
    task_total: u64,
    task_description: String,
}

impl LiveDisplay {
    pub fn new() -> Self {
        let multi = MultiProgress::new();
        let term_width = Term::stdout().size_checked().map(|(_, w)| w).unwrap_or(80);
        Self {
            multi,
            tool_spinner: None,
            task_bar: None,
            current_tool: None,
            agent_buffer: String::new(),
            start_time: Instant::now(),
            term_width,
            task_current: 0,
            task_total: 0,
            task_description: String::new(),
        }
    }

    /// Process a stream event and update the display.
    pub fn update(&mut self, event: StreamEvent) {
        match event {
            StreamEvent::ToolStart { name, id } => {
                // Finish previous spinner
                if let Some(s) = self.tool_spinner.take() {
                    s.finish_with_message(format!("✓ {}", s.message()));
                }
                let spinner = Spinner::new(&format!("⚙ {}", name));
                self.multi.add_child(spinner.clone());
                self.current_tool = Some(name);
                self.tool_spinner = Some(spinner);
            }
            StreamEvent::ToolOutput { id: _id, content } => {
                // Tool finished — tick the spinner and leave it.
                if let Some(ref s) = self.tool_spinner {
                    s.set_message(format!(
                        "✓ {}",
                        self.current_tool.as_deref().unwrap_or("tool")
                    ));
                }
                // Print truncated output (non-blocking, no progress bar interference)
                if !content.is_empty() {
                    let preview: String = content
                        .lines()
                        .take(3)
                        .map(|l| l.chars().take(self.term_width.saturating_sub(4) as usize).collect::<String>())
                        .collect::<Vec<_>>()
                        .join("\n  ");
                    if let Some(ref s) = self.tool_spinner {
                        s.println(format!("  {}", preview));
                    }
                }
            }
            StreamEvent::AgentMessage { role, text } => {
                let prefix = match role.as_str() {
                    "assistant" => "◆",
                    "user" => "◇",
                    _ => "○",
                };
                if !self.agent_buffer.is_empty() {
                    // Flush previous buffer
                    let msg = std::mem::take(&mut self.agent_buffer);
                    println!("{} {}", prefix, msg);
                }
                self.agent_buffer = text;
            }
            StreamEvent::TaskProgress {
                current,
                total,
                description,
            } => {
                self.task_current = current;
                self.task_total = total;
                self.task_description = description;

                if self.task_bar.is_none() {
                    let bar = ProgressBar::new(total);
                    bar.set_style("▰▱");
                    self.multi.add_child(bar.clone());
                    self.task_bar = Some(bar);
                }
                if let Some(ref bar) = self.task_bar {
                    bar.set_position(current);
                    bar.set_message(format!(
                        "{} [{}/{}]",
                        self.task_description, current, total
                    ));
                }
            }
            StreamEvent::TaskComplete { summary } => {
                if let Some(bar) = self.task_bar.take() {
                    bar.finish_with_message(format!("✓ {}", summary));
                }
                if let Some(s) = self.tool_spinner.take() {
                    s.finish_and_clear();
                }
            }
            StreamEvent::Error { message } => {
                // Print error prominently
                if let Some(ref s) = self.tool_spinner {
                    s.println(format!("✗ ERROR: {}", message));
                } else {
                    eprintln!("✗ ERROR: {}", message);
                }
                if let Some(bar) = self.task_bar.take() {
                    bar.finish_with_message(format!("✗ Failed"));
                }
            }
        }
    }

    /// Flush the agent buffer and print the final summary.
    pub fn finish(&mut self) {
        // Flush any buffered agent message
        if !self.agent_buffer.is_empty() {
            println!("◆ {}", std::mem::take(&mut self.agent_buffer));
        }
        // Finish remaining spinner
        if let Some(s) = self.tool_spinner.take() {
            s.finish_and_clear();
        }
        // Finish progress bar
        if let Some(bar) = self.task_bar.take() {
            bar.finish_and_clear();
        }
        let elapsed = self.start_time.elapsed();
        println!();
        println!("── completed in {:.1}s ──", elapsed.as_secs_f64());
    }

    /// Get elapsed time since display was created.
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Print a line without interfering with progress bars.
    pub fn println(&self, msg: &str) {
        if let Some(ref s) = self.tool_spinner {
            s.println(msg.to_string());
        } else {
            println!("{}", msg);
        }
    }
}

impl Default for LiveDisplay {
    fn default() -> Self {
        Self::new()
    }
}
