use async_trait::async_trait;
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

use crate::event::{StopReason, TokenUsage};

pub mod anthropic;
pub mod openai_compat;
pub mod responses;

// ─── Model capabilities ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCaps {
    /// Context window size in tokens
    pub context_window: u64,
    /// Maximum output tokens
    pub max_output: u64,
    /// Whether the model supports tool calling
    pub tools: bool,
    /// Whether the model supports vision/image inputs
    pub vision: bool,
    /// Cost per million input tokens (USD)
    pub cost_input_per_mtok: f64,
    /// Cost per million output tokens (USD)
    pub cost_output_per_mtok: f64,
    /// Latency class
    pub latency: LatencyClass,
}

impl Default for ModelCaps {
    fn default() -> Self {
        Self {
            context_window: 128_000,
            max_output: 16_000,
            tools: true,
            vision: false,
            cost_input_per_mtok: 0.0,
            cost_output_per_mtok: 0.0,
            latency: LatencyClass::Medium,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LatencyClass {
    Fast,
    Medium,
    Slow,
}

// ─── Message types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Msg {
    pub role: String,
    pub content: Vec<ContentBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image {
        source: ImageSource,
    },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: Vec<ContentBlock>,
        is_error: Option<bool>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ImageSource {
    #[serde(rename = "base64")]
    Base64 {
        media_type: String,
        data: String,
    },
    #[serde(rename = "url")]
    Url { url: String },
}

// ─── Tool specification (for Brain request) ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

// ─── Brain request ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BrainRequest {
    pub system: Option<String>,
    pub messages: Vec<Msg>,
    pub tools: Vec<ToolSpec>,
    pub max_tokens: u32,
    pub temperature: f32,
    pub stop: Vec<String>,
}

impl Default for BrainRequest {
    fn default() -> Self {
        Self {
            system: None,
            messages: vec![],
            tools: vec![],
            max_tokens: 4096,
            temperature: 0.0,
            stop: vec![],
        }
    }
}

// ─── Brain events (unified stream) ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum BrainEvent {
    TextDelta(String),
    ToolUseStart {
        id: String,
        name: String,
    },
    ToolUseDelta {
        id: String,
        json: String,
    },
    ToolUseEnd {
        id: String,
    },
    Usage(TokenUsage),
    Done(StopReason),
    Error(String),
}

// ─── Brain stream ───────────────────────────────────────────────────────────────

pub type BrainStream = Pin<Box<dyn Stream<Item = BrainEvent> + Send>>;

// ─── THE BRAIN TRAIT ────────────────────────────────────────────────────────────

/// Uniform interface over every model vendor.
/// Normalizes messages, streaming, and tool-calling so the rest of the system is vendor-agnostic.
#[async_trait]
pub trait Brain: Send + Sync {
    /// Full provider:model identifier, e.g. "anthropic:claude-sonnet-4-6"
    fn id(&self) -> &str;
    /// Model capabilities
    fn caps(&self) -> ModelCaps;
    /// Stream a completion
    async fn complete(&self, req: BrainRequest) -> anyhow::Result<BrainStream>;
}

// ─── Brain error ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum BrainError {
    RateLimit { retry_after: Option<u64> },
    ServerError { status: u16, body: String },
    Timeout,
    Refusal(String),
    Unknown(String),
}

impl std::fmt::Display for BrainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BrainError::RateLimit { retry_after } => {
                write!(f, "rate limited (retry after {:?}s)", retry_after)
            }
            BrainError::ServerError { status, body } => {
                write!(f, "server error {}: {}", status, body)
            }
            BrainError::Timeout => write!(f, "timeout"),
            BrainError::Refusal(msg) => write!(f, "refusal: {}", msg),
            BrainError::Unknown(msg) => write!(f, "unknown: {}", msg),
        }
    }
}
