//! Interactive chat session for Sparrow.
//!
//! Persistent chat with history, save/load, and streaming responses.

use std::path::Path;

pub mod composer;

/// A single message in the chat history.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatMessage {
    pub role: String, // "user" or "assistant"
    pub content: String,
    pub timestamp: String,
}

/// A persistent chat session with full history.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatSession {
    pub messages: Vec<ChatMessage>,
    pub created_at: String,
    pub model: Option<String>,
}

impl ChatSession {
    /// Create a new empty chat session.
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            model: None,
        }
    }

    /// Add a user message to the history.
    pub fn add_user_message(&mut self, content: &str) {
        self.messages.push(ChatMessage {
            role: "user".to_string(),
            content: content.to_string(),
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
        });
    }

    /// Add an assistant message to the history.
    pub fn add_assistant_message(&mut self, content: &str) {
        self.messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: content.to_string(),
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
        });
    }

    /// Get the conversation history as (user, assistant) pairs.
    pub fn history(&self) -> Vec<(&str, &str)> {
        let mut pairs = Vec::new();
        let mut current_user: Option<&str> = None;

        for msg in &self.messages {
            match msg.role.as_str() {
                "user" => current_user = Some(&msg.content),
                "assistant" => {
                    if let Some(user_msg) = current_user.take() {
                        pairs.push((user_msg, msg.content.as_str()));
                    }
                }
                _ => {}
            }
        }

        pairs
    }

    /// Get the last N messages as context.
    pub fn last_context(&self, n: usize) -> Vec<&ChatMessage> {
        self.messages.iter().rev().take(n).rev().collect()
    }

    /// Save the session to a JSON file.
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load a session from a JSON file.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let session: ChatSession = serde_json::from_str(&json)?;
        Ok(session)
    }

    /// Clear all messages.
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// Get the number of messages.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Check if the session is empty.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

impl Default for ChatSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_session() {
        let mut session = ChatSession::new();
        assert!(session.is_empty());

        session.add_user_message("Hello");
        session.add_assistant_message("Hi there!");
        assert_eq!(session.len(), 2);

        let history = session.history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].0, "Hello");
        assert_eq!(history[0].1, "Hi there!");
    }

    #[test]
    fn test_save_load() {
        let mut session = ChatSession::new();
        session.add_user_message("test");
        session.add_assistant_message("response");

        let tmp = std::env::temp_dir().join("sparrow_test_chat.json");
        session.save(&tmp).unwrap();

        let loaded = ChatSession::load(&tmp).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.messages[0].content, "test");

        let _ = std::fs::remove_file(&tmp);
    }
}
