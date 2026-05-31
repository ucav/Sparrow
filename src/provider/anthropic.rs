use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use reqwest::Client;
use serde_json::json;
use std::collections::HashMap;

use super::{Brain, BrainEvent, BrainRequest, BrainStream, ContentBlock, LatencyClass, ModelCaps};

pub struct AnthropicAdapter {
    model: String,
    api_key: String,
    base_url: String,
    client: Client,
    caps: ModelCaps,
}

impl AnthropicAdapter {
    pub fn new(model: &str, api_key: impl Into<String>, base_url: Option<&str>) -> Self {
        let model = model.to_string();
        let caps = Self::model_caps(&model);
        Self {
            model,
            api_key: api_key.into(),
            base_url: base_url.unwrap_or("https://api.anthropic.com").to_string(),
            client: Client::new(),
            caps,
        }
    }

    fn model_caps(model: &str) -> ModelCaps {
        if model.contains("opus") {
            ModelCaps {
                context_window: 200_000,
                max_output: 32_000,
                tools: true,
                vision: true,
                cost_input_per_mtok: 15.0,
                cost_output_per_mtok: 75.0,
                latency: LatencyClass::Slow,
            }
        } else if model.contains("sonnet") {
            ModelCaps {
                context_window: 200_000,
                max_output: 16_000,
                tools: true,
                vision: true,
                cost_input_per_mtok: 3.0,
                cost_output_per_mtok: 15.0,
                latency: LatencyClass::Medium,
            }
        } else {
            // haiku
            ModelCaps {
                context_window: 200_000,
                max_output: 8_000,
                tools: true,
                vision: true,
                cost_input_per_mtok: 0.8,
                cost_output_per_mtok: 4.0,
                latency: LatencyClass::Fast,
            }
        }
    }
}

#[async_trait]
impl Brain for AnthropicAdapter {
    fn id(&self) -> &str {
        &self.model
    }

    fn caps(&self) -> ModelCaps {
        self.caps.clone()
    }

    async fn complete(&self, req: BrainRequest) -> anyhow::Result<BrainStream> {
        let system: Option<String> = req.system;
        let mut messages = Vec::new();

        // Build Anthropic-formatted messages from our Msg vec
        for msg in &req.messages {
            let mut content: Vec<serde_json::Value> = Vec::new();

            for block in &msg.content {
                match block {
                    ContentBlock::Text { text } => {
                        content.push(json!({"type": "text", "text": text}));
                    }
                    ContentBlock::Image { source } => match source {
                        super::ImageSource::Base64 { media_type, data } => {
                            content.push(json!({
                                "type": "image",
                                "source": {
                                    "type": "base64",
                                    "media_type": media_type,
                                    "data": data,
                                }
                            }));
                        }
                        super::ImageSource::Url { url } => {
                            content.push(json!({
                                "type": "image",
                                "source": {
                                    "type": "url",
                                    "url": url,
                                }
                            }));
                        }
                    },
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content: tool_content,
                        is_error,
                    } => {
                        let inner: Vec<serde_json::Value> = tool_content
                            .iter()
                            .map(|b| match b {
                                ContentBlock::Text { text } => {
                                    json!({"type": "text", "text": text})
                                }
                                _ => json!({"type": "text", "text": format!("{:?}", b)}),
                            })
                            .collect();
                        let mut val = json!({
                            "type": "tool_result",
                            "tool_use_id": tool_use_id,
                            "content": inner,
                        });
                        if let Some(true) = is_error {
                            val["is_error"] = json!(true);
                        }
                        content.push(val);
                    }
                    ContentBlock::ToolUse { .. } => {}
                }
            }

            messages.push(json!({
                "role": msg.role,
                "content": content,
            }));
        }

        // Build tools
        let tools: Vec<serde_json::Value> = if req.tools.is_empty() {
            vec![]
        } else {
            req.tools
                .iter()
                .map(|t| {
                    json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.input_schema,
                    })
                })
                .collect()
        };

        let mut body = json!({
            "model": self.model,
            "max_tokens": req.max_tokens,
            "temperature": req.temperature,
            "messages": messages,
            "stream": true,
        });

        if let Some(sys) = &system {
            body["system"] = json!(sys);
        }
        if !tools.is_empty() {
            body["tools"] = json!(tools);
        }
        if !req.stop.is_empty() {
            body["stop_sequences"] = json!(req.stop);
        }

        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Anthropic API error {}: {}", status, body));
        }

        let stream = response.bytes_stream();
        let model = self.model.clone();

        let event_stream = stream
            .scan(HashMap::<u64, String>::new(), move |tool_ids, chunk| {
                let _model = model.clone();
                let events = match chunk {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        let mut events = Vec::new();
                        for line in text.lines() {
                            let line = line.trim();
                            if line.is_empty() || !line.starts_with("data: ") {
                                continue;
                            }
                            let data = &line[6..]; // Strip "data: "
                            let event: serde_json::Value = match serde_json::from_str(data) {
                                Ok(v) => v,
                                Err(_) => continue,
                            };

                            let event_type = event["type"].as_str().unwrap_or("");
                            match event_type {
                                "content_block_start" => {
                                    let index = event["index"].as_u64().unwrap_or(0);
                                    let content_type =
                                        event["content_block"]["type"].as_str().unwrap_or("");
                                    if content_type == "tool_use" {
                                        let id = event["content_block"]["id"]
                                            .as_str()
                                            .unwrap_or("")
                                            .to_string();
                                        let name = event["content_block"]["name"]
                                            .as_str()
                                            .unwrap_or("")
                                            .to_string();
                                        if !id.is_empty() {
                                            tool_ids.insert(index, id.clone());
                                        }
                                        events.push(BrainEvent::ToolUseStart { id, name });
                                    }
                                }
                                "content_block_delta" => {
                                    let delta_type = event["delta"]["type"].as_str().unwrap_or("");
                                    if delta_type == "text_delta" {
                                        let text = event["delta"]["text"]
                                            .as_str()
                                            .unwrap_or("")
                                            .to_string();
                                        events.push(BrainEvent::TextDelta(text));
                                    } else if delta_type == "input_json_delta" {
                                        let partial = event["delta"]["partial_json"]
                                            .as_str()
                                            .unwrap_or("")
                                            .to_string();
                                        let index = event["index"].as_u64().unwrap_or(0);
                                        let id = tool_ids
                                            .get(&index)
                                            .cloned()
                                            .unwrap_or_else(|| index.to_string());
                                        events.push(BrainEvent::ToolUseDelta { id, json: partial });
                                    }
                                }
                                "content_block_stop" => {
                                    let index = event["index"].as_u64().unwrap_or(0);
                                    let id = tool_ids
                                        .remove(&index)
                                        .unwrap_or_else(|| index.to_string());
                                    events.push(BrainEvent::ToolUseEnd { id });
                                }
                                "message_delta" => {
                                    if let Some(usage) = event["usage"].as_object() {
                                        events.push(BrainEvent::Usage(crate::event::TokenUsage {
                                            input: usage["input_tokens"].as_u64().unwrap_or(0),
                                            output: usage["output_tokens"].as_u64().unwrap_or(0),
                                        }));
                                    }
                                    let stop_reason = event["delta"]["stop_reason"]
                                        .as_str()
                                        .unwrap_or("end_turn");
                                    let reason = match stop_reason {
                                        "end_turn" => crate::event::StopReason::EndTurn,
                                        "max_tokens" => crate::event::StopReason::MaxTokens,
                                        "tool_use" => crate::event::StopReason::ToolUse,
                                        s => crate::event::StopReason::StopSequence(s.to_string()),
                                    };
                                    events.push(BrainEvent::Done(reason));
                                }
                                "message_stop" => {}
                                _ => {}
                            }
                        }
                        events
                    }
                    Err(e) => {
                        vec![BrainEvent::Error(format!("stream error: {}", e))]
                    }
                };
                futures::future::ready(Some(stream::iter(events)))
            })
            .flatten();

        Ok(Box::pin(event_stream))
    }
}
