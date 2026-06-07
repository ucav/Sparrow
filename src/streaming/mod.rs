//! Live streaming infrastructure for Sparrow.
//!
//! Provides channels for real-time event streaming and a LiveDisplay
//! that renders progress bars, spinners, and status updates in the terminal.

use std::time::Instant;

pub mod lane;
pub mod progress;

/// Events emitted during task execution.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// A tool is starting
    ToolStart { name: String, description: String },
    /// Tool produced output
    ToolOutput { name: String, output: String },
    /// The agent sent a message
    AgentMessage { content: String },
    /// Task progress update
    TaskProgress {
        current: u64,
        total: u64,
        description: String,
    },
    /// Task completed
    TaskComplete { summary: String },
    /// An error occurred
    Error { message: String },
}

/// Sender side of the stream channel.
pub struct StreamSender {
    tx: tokio::sync::mpsc::UnboundedSender<StreamEvent>,
}

impl StreamSender {
    pub fn send(&self, event: StreamEvent) {
        let _ = self.tx.send(event);
    }
}

/// Receiver side with live terminal display.
pub struct StreamReceiver {
    rx: tokio::sync::mpsc::UnboundedReceiver<StreamEvent>,
    display: LiveDisplay,
}

impl StreamReceiver {
    pub fn display(&mut self) -> &mut LiveDisplay {
        &mut self.display
    }

    /// Process incoming events and update the display.
    pub async fn process(&mut self) -> Option<StreamEvent> {
        match self.rx.recv().await {
            Some(event) => {
                self.display.update(&event);
                Some(event)
            }
            None => None,
        }
    }
}

/// Create a streaming channel pair.
pub fn create_channel() -> (StreamSender, StreamReceiver) {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let sender = StreamSender { tx };
    let receiver = StreamReceiver {
        rx,
        display: LiveDisplay::new(),
    };
    (sender, receiver)
}

/// Live terminal display for streaming task execution.
pub struct LiveDisplay {
    start_time: Instant,
    current_tool: Option<String>,
    task_description: String,
    steps_done: u64,
    steps_total: u64,
}

impl LiveDisplay {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            current_tool: None,
            task_description: String::new(),
            steps_done: 0,
            steps_total: 0,
        }
    }

    /// Update the display with a new event.
    pub fn update(&mut self, event: &StreamEvent) {
        match event {
            StreamEvent::ToolStart { name, description } => {
                self.current_tool = Some(name.clone());
                let elapsed = self.start_time.elapsed().as_secs();
                eprintln!(
                    "\x1b[36m[{elapsed:>3}s]\x1b[0m \x1b[33m🔧 {name}\x1b[0m — {description}"
                );
            }
            StreamEvent::ToolOutput { name: _, output } => {
                let preview: String = output
                    .lines()
                    .take(3)
                    .map(|l| format!("  │ {l}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                if !preview.is_empty() {
                    eprintln!("{preview}");
                    if output.lines().count() > 3 {
                        eprintln!(
                            "  │ \x1b[2m... ({} more lines)\x1b[0m",
                            output.lines().count() - 3
                        );
                    }
                }
            }
            StreamEvent::AgentMessage { content } => {
                eprintln!("\x1b[35m💬\x1b[0m {content}");
            }
            StreamEvent::TaskProgress {
                current,
                total,
                description,
            } => {
                self.steps_done = *current;
                self.steps_total = *total;
                self.task_description = description.clone();
                let pct = if *total > 0 {
                    (current * 100) / total
                } else {
                    0
                };
                let bar = "█".repeat(pct as usize / 5) + &"░".repeat(20 - pct as usize / 5);
                let elapsed = self.start_time.elapsed().as_secs();
                eprintln!("\x1b[36m[{elapsed:>3}s]\x1b[0m [{bar}] {pct}% — {description}");
            }
            StreamEvent::TaskComplete { summary } => {
                let elapsed = self.start_time.elapsed().as_secs_f64();
                eprintln!("\x1b[32m✓\x1b[0m {summary} \x1b[2m({elapsed:.1}s)\x1b[0m");
            }
            StreamEvent::Error { message } => {
                eprintln!("\x1b[31m✗\x1b[0m {message}");
            }
        }
    }

    /// Finish the display and print a summary.
    pub fn finish(&mut self) {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        eprintln!(
            "\n\x1b[1mDone in {elapsed:.1}s\x1b[0m — {}/{} steps completed",
            self.steps_done, self.steps_total
        );
    }

    /// Get the elapsed time in seconds.
    pub fn elapsed(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }
}
