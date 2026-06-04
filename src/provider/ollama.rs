use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use reqwest::Client;
use serde_json::json;

use super::{Brain, BrainEvent, BrainRequest, BrainStream, ContentBlock, LatencyClass, ModelCaps};

/// Native Ollama adapter using `/api/chat` with NDJSON streaming.
/// Ollama does not use OpenAI-compatible tool format; it has its own.
pub struct OllamaAdapter {
    model: String,
    base_url: String,
    client: Client,
    caps: ModelCaps,
}

impl OllamaAdapter {
    pub fn new(model: &str, base_url: &str) -> Self {
        Self {
            model: model.to_string(),
            base_url: base_url
                .trim_end_matches("/v1")
                .trim_end_matches('/')
                .to_string(),
            client: Client::new(),
            caps: ModelCaps {
                context_window: 32_768,
                max_output: 8_000,
                tools: true,
                vision: false,
                cost_input_per_mtok: 0.0,
                cost_output_per_mtok: 0.0,
                latency: LatencyClass::Medium,
            },
        }
    }

    pub fn with_caps(mut self, caps: ModelCaps) -> Self {
        self.caps = caps;
        self
    }

    /// Convert Sparrow Msg into Ollama's native format
    fn build_ollama_messages(req: &BrainRequest) -> Vec<serde_json::Value> {
        let mut messages: Vec<serde_json::Value> = Vec::new();

        if let Some(sys) = &req.system {
            messages.push(json!({"role": "system", "content": sys}));
        }

        for msg in &req.messages {
            let role = match msg.role.as_str() {
                "assistant" => "assistant",
                _ => "user",
            };

            let mut content = String::new();
            let mut tool_calls: Vec<serde_json::Value> = Vec::new();

            for block in &msg.content {
                match block {
                    ContentBlock::Text { text } => {
                        content.push_str(text);
                    }
                    ContentBlock::ToolUse { id: _, name, input } => {
                        tool_calls.push(json!({
                            "function": {
                                "name": name,
                                "arguments": input,
                            }
                        }));
                    }
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content: blocks,
                        is_error: _,
                    } => {
                        let text: String = blocks
                            .iter()
                            .filter_map(|b| match b {
                                ContentBlock::Text { text } => Some(text.as_str()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        // Ollama native: tool results are user messages with tool_call_id
                        messages.push(json!({
                            "role": "tool",
                            "content": text,
                            "tool_call_id": tool_use_id,
                        }));
                    }
                    _ => {}
                }
            }

            if !content.is_empty() || tool_calls.is_empty() {
                let mut msg_json = json!({"role": role, "content": content});
                if !tool_calls.is_empty() {
                    msg_json["tool_calls"] = json!(tool_calls);
                }
                messages.push(msg_json);
            }
        }

        messages
    }

    /// Convert Sparrow ToolSpec to Ollama tool format
    fn build_ollama_tools(tools: &[super::ToolSpec]) -> Vec<serde_json::Value> {
        if tools.is_empty() {
            return vec![];
        }
        tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.input_schema,
                    }
                })
            })
            .collect()
    }
}

#[async_trait]
impl Brain for OllamaAdapter {
    fn id(&self) -> &str {
        &self.model
    }

    fn caps(&self) -> ModelCaps {
        self.caps.clone()
    }

    async fn complete(&self, req: BrainRequest) -> anyhow::Result<BrainStream> {
        let messages = Self::build_ollama_messages(&req);
        let tools = Self::build_ollama_tools(&req.tools);

        let mut body = json!({
            "model": self.model,
            "messages": messages,
            "stream": true,
            "options": {
                "temperature": req.temperature as f64,
            }
        });

        if req.max_tokens > 0 {
            body["options"]["num_predict"] = json!(req.max_tokens);
        }
        if !tools.is_empty() {
            body["tools"] = json!(tools);
        }

        let url = format!("{}/api/chat", self.base_url);

        let response = self.client.post(&url).json(&body).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Ollama API error {}: {}", status, body));
        }

        let stream = response.bytes_stream();

        // NDJSON across chunk boundaries needs the same line buffer the SSE
        // providers use (see provider/sse_buffer.rs) — without it a JSON object
        // split between two TCP chunks gets dropped silently.
        let event_stream = stream.scan(
            super::sse_buffer::LineBuffer::new(),
            |line_buf, chunk| {
            let result: Option<futures::stream::Iter<std::vec::IntoIter<BrainEvent>>> = (|| {
            let events: Vec<BrainEvent> = match chunk {
                Ok(bytes) => {
                    let lines = line_buf.push(&bytes);
                    let mut parsed = Vec::new();
                    for line in lines {
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        let event: serde_json::Value = match serde_json::from_str(line) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };

                        // Ollama NDJSON: {"message":{"content":"..."}} or {"message":{"tool_calls":[...]}}
                        if let Some(msg) = event.get("message") {
                            // Text delta (Ollama streams full message each line, not deltas)
                            if let Some(content) = msg.get("content").and_then(|v| v.as_str()) {
                                if !content.is_empty() {
                                    parsed.push(BrainEvent::TextDelta(content.to_string()));
                                }
                            }
                            // Tool calls
                            if let Some(tc_array) = msg.get("tool_calls").and_then(|v| v.as_array())
                            {
                                for tc in tc_array {
                                    if let Some(func) = tc.get("function") {
                                        let name =
                                            func.get("name").and_then(|v| v.as_str()).unwrap_or("");
                                        let args = func.get("arguments");
                                        // Ollama sends tool_calls as objects; we emit start+end
                                        let id = format!("tc_{}", name);
                                        parsed.push(BrainEvent::ToolUseStart {
                                            id: id.clone(),
                                            name: name.to_string(),
                                        });
                                        if let Some(args) = args {
                                            parsed.push(BrainEvent::ToolUseDelta {
                                                id: id.clone(),
                                                json: args.to_string(),
                                            });
                                        }
                                        parsed.push(BrainEvent::ToolUseEnd { id });
                                    }
                                }
                            }
                        }

                        // Usage
                        if let (Some(prompt), Some(completion)) = (
                            event.get("prompt_eval_count").and_then(|v| v.as_u64()),
                            event.get("eval_count").and_then(|v| v.as_u64()),
                        ) {
                            parsed.push(BrainEvent::Usage(crate::event::TokenUsage {
                                input: prompt,
                                output: completion,
                            }));
                        }

                        // Done
                        if event.get("done").and_then(|v| v.as_bool()).unwrap_or(false) {
                            let reason = event
                                .get("done_reason")
                                .and_then(|v| v.as_str())
                                .unwrap_or("stop");
                            let stop = match reason {
                                "stop" => crate::event::StopReason::EndTurn,
                                "length" => crate::event::StopReason::MaxTokens,
                                "tool_calls" => crate::event::StopReason::ToolUse,
                                s => crate::event::StopReason::StopSequence(s.to_string()),
                            };
                            parsed.push(BrainEvent::Done(stop));
                        }
                    }
                    parsed
                }
                Err(e) => vec![BrainEvent::Error(format!("Ollama stream error: {}", e))],
            };
            Some(stream::iter(events))
            })();
            async move { result }
        })
        .flatten();

        Ok(Box::pin(event_stream))
    }
}
