// ─── Interactive chat session ────────────────────────────────────────────────
//
// Persistent multi-turn chat with history, save/load to JSON, and streaming
// response support. Designed for both CLI and TUI surfaces.

pub mod composer;

use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::chat::composer::InputComposer;
use crate::streaming::{create_channel, StreamEvent, StreamReceiver, StreamSender};

// ─── Chat message ────────────────────────────────────────────────────────────

/// A single message in the chat history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

// ─── Chat session ────────────────────────────────────────────────────────────

/// Persistent chat session with history and streaming.
pub struct ChatSession {
    /// Ordered conversation history (user/assistant pairs).
    history: Vec<ChatMessage>,
    /// Active streaming channel for current response.
    stream_tx: Option<StreamSender>,
    stream_rx: Option<StreamReceiver>,
    /// Accumulated response being built.
    pending_response: String,
    /// Whether the session has been modified since last save.
    dirty: bool,
}

impl ChatSession {
    /// Create a new empty chat session.
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            stream_tx: None,
            stream_rx: None,
            pending_response: String::new(),
            dirty: false,
        }
    }

    /// Send a user message and begin streaming the assistant response.
    /// Returns the streaming receiver so the caller can poll events.
    ///
    /// The returned `StreamReceiver` will emit `AgentMessage` events with the
    /// assistant's response chunks, then a `TaskComplete` when done.
    pub fn send(&mut self, message: &str) -> StreamReceiver {
        // Record user message
        self.history.push(ChatMessage {
            role: "user".to_string(),
            content: message.to_string(),
        });
        self.dirty = true;
        self.pending_response.clear();

        // Create a fresh channel for this response
        let (tx, rx) = create_channel();
        let tx_clone = tx.clone();

        // Spawn a task that will stream the response.
        // In practice, this would be replaced by actual agent invocation.
        // The channel stays open so callers can inject events from the agent.
        self.stream_tx = Some(tx);
        self.stream_rx = Some(StreamReceiver::bare(rx.rx));

        // Return a new receiver wrapping the raw mpsc receiver
        // (We need to extract it — the StreamReceiver we stored owns it)
        // For the caller, we create a fresh channel since we can't move out.
        let (out_tx, out_rx) = create_channel();
        self.stream_tx = Some(out_tx);

        // Store the receiver for the caller
        self.stream_rx = None;
        out_rx
    }

    /// Feed a stream event into the current response. Returns the new
    /// accumulated text if the event was an AgentMessage.
    pub fn feed_event(&mut self, event: StreamEvent) -> Option<String> {
        match event {
            StreamEvent::AgentMessage { role, text } if role == "assistant" => {
                self.pending_response.push_str(&text);
                Some(self.pending_response.clone())
            }
            StreamEvent::TaskComplete { summary } => {
                // Finalize the assistant message
                if !self.pending_response.is_empty() {
                    self.history.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: std::mem::take(&mut self.pending_response),
                    });
                } else if !summary.is_empty() {
                    self.history.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: summary,
                    });
                }
                self.dirty = true;
                None
            }
            StreamEvent::Error { message } => {
                self.history.push(ChatMessage {
                    role: "error".to_string(),
                    content: message,
                });
                self.dirty = true;
                self.pending_response.clear();
                None
            }
            _ => None,
        }
    }

    /// Get the conversation history as (role, content) pairs.
    pub fn history(&self) -> Vec<(String, String)> {
        self.history
            .iter()
            .map(|m| (m.role.clone(), m.content.clone()))
            .collect()
    }

    /// Get the raw chat messages.
    pub fn messages(&self) -> &[ChatMessage] {
        &self.history
    }

    /// Whether the session has unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clear the conversation history.
    pub fn clear(&mut self) {
        self.history.clear();
        self.pending_response.clear();
        self.dirty = true;
    }

    /// Number of messages in the history.
    pub fn len(&self) -> usize {
        self.history.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }

    /// Save the session to a JSON file.
    pub fn save(&self, path: &Path) -> Result<()> {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent)?;

        let json = serde_json::to_string_pretty(&SaveFormat {
            version: 1,
            messages: self.history.clone(),
        })?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load a session from a JSON file.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let saved: SaveFormat = serde_json::from_str(&content)?;

        if saved.version != 1 {
            anyhow::bail!(
                "Unsupported chat session version: {} (expected 1)",
                saved.version
            );
        }

        Ok(Self {
            history: saved.messages,
            stream_tx: None,
            stream_rx: None,
            pending_response: String::new(),
            dirty: false,
        })
    }
}

impl Default for ChatSession {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Serialization format ────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct SaveFormat {
    version: u32,
    messages: Vec<ChatMessage>,
}

// ─── Convenience: interactive loop ───────────────────────────────────────────

impl ChatSession {
    /// Run a simple interactive chat loop using stdin/stdout.
    /// The `responder` closure is called with the user message and must return
    /// a response string. This is a convenience for quick CLI chat.
    pub async fn run_interactive<F, Fut>(&mut self, mut responder: F) -> Result<()>
    where
        F: FnMut(String) -> Fut,
        Fut: std::future::Future<Output = Result<String>>,
    {
        use std::io::{self, Write};

        println!("═══ Sparrow Chat ═══");
        println!("Type your message and press Enter. Ctrl+D or /exit to quit.");
        println!();

        loop {
            print!("◆ you › ");
            io::stdout().flush()?;

            let mut input = String::new();
            let n = io::stdin().read_line(&mut input)?;
            if n == 0 {
                // EOF
                break;
            }
            let input = input.trim().to_string();
            if input.is_empty() {
                continue;
            }
            if input == "/exit" || input == "/quit" {
                break;
            }

            // Handle slash commands
            if input == "/clear" {
                self.clear();
                println!("── history cleared ──");
                continue;
            }
            if input == "/history" {
                for m in &self.history {
                    println!("  [{}] {}", m.role, m.content);
                }
                continue;
            }
            if input == "/save" {
                let path = dirs::data_local_dir()
                    .unwrap_or_default()
                    .join("sparrow")
                    .join("chat_sessions")
                    .join(format!("chat_{}.json", chrono::Utc::now().format("%Y%m%d_%H%M%S")));
                self.save(&path)?;
                println!("── saved to {} ──", path.display());
                continue;
            }

            // Record user message
            self.history.push(ChatMessage {
                role: "user".to_string(),
                content: input.clone(),
            });

            // Get response
            match responder(input).await {
                Ok(response) => {
                    println!("◆ sparrow › {}", response);
                    self.history.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: response,
                    });
                }
                Err(e) => {
                    eprintln!("✗ Error: {}", e);
                    self.history.push(ChatMessage {
                        role: "error".to_string(),
                        content: e.to_string(),
                    });
                }
            }
            println!();
        }

        Ok(())
    }
}
