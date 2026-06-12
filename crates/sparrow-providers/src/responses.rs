use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde_json::json;

use super::{Brain, BrainEvent, BrainRequest, BrainStream, LatencyClass, ModelCaps};

// ─── OpenAI Responses API adapter ───────────────────────────────────────────────

/// OpenAI's newer Responses API (vs Chat Completions).
/// Uses the /v1/responses endpoint with its own message format.
pub struct OpenAIResponsesAdapter {
    model: String,
    api_key: String,
    base_url: String,
    client: Client,
    caps: ModelCaps,
}

impl OpenAIResponsesAdapter {
    pub fn new(model: &str, api_key: impl Into<String>, base_url: Option<&str>) -> Self {
        let model = model.to_string();
        Self {
            model,
            api_key: api_key.into(),
            base_url: base_url.unwrap_or("https://api.openai.com/v1").to_string(),
            client: Client::new(),
            caps: ModelCaps {
                context_window: 128_000,
                max_output: 16_000,
                tools: true,
                vision: true,
                cost_input_per_mtok: 2.5,
                cost_output_per_mtok: 10.0,
                latency: LatencyClass::Medium,
            },
        }
    }

    /// Override capabilities with those from the static registry (cost, context, etc.)
    pub fn with_caps(mut self, caps: ModelCaps) -> Self {
        self.caps = caps;
        self
    }
}

fn build_responses_body(model: &str, req: &BrainRequest) -> serde_json::Value {
    // Convert to Responses API format
    let mut input: Vec<serde_json::Value> = Vec::new();

    if let Some(sys) = &req.system {
        input.push(json!({
            "role": "system",
            "content": sys,
        }));
    }

    for msg in &req.messages {
        let mut reasoning_buf = String::new();
        let mut content_blocks: Vec<serde_json::Value> = Vec::new();
        for block in &msg.content {
            match block {
                super::ContentBlock::Text { text } => {
                    if msg.role == "assistant" {
                        content_blocks.push(json!({
                            "type": "output_text",
                            "text": text,
                        }));
                    } else {
                        content_blocks.push(json!({
                            "type": "input_text",
                            "text": text,
                        }));
                    }
                }
                super::ContentBlock::Image { source } => {
                    content_blocks.push(json!({
                        "type": "input_image",
                        "image_url": image_source_url(source),
                    }));
                }
                super::ContentBlock::Reasoning { text } => {
                    if !reasoning_buf.is_empty() {
                        reasoning_buf.push('\n');
                    }
                    reasoning_buf.push_str(text);
                }
                _ => {}
            }
        }
        let content = if content_blocks.len() == 1
            && content_blocks[0]["type"].as_str() == Some("input_text")
        {
            content_blocks[0]["text"].clone()
        } else if content_blocks.len() == 1
            && content_blocks[0]["type"].as_str() == Some("output_text")
        {
            content_blocks[0]["text"].clone()
        } else {
            json!(content_blocks)
        };

        let mut item = json!({
            "role": msg.role,
            "content": content,
        });
        if msg.role == "assistant" && !reasoning_buf.is_empty() {
            item["reasoning_content"] = json!(reasoning_buf);
        }
        input.push(item);
    }

    let mut body = json!({
        "model": model,
        "input": input,
        "stream": true,
        "temperature": req.temperature,
        "max_output_tokens": req.max_tokens,
    });

    if req.cache.enabled {
        if let Some(key) = &req.cache.key {
            body["prompt_cache_key"] = json!(key);
        }
        body["prompt_cache_retention"] = json!(req.cache.ttl.openai_retention());
    }

    body
}

fn image_source_url(source: &super::ImageSource) -> String {
    match source {
        super::ImageSource::Base64 { media_type, data } => {
            format!("data:{};base64,{}", media_type, data)
        }
        super::ImageSource::Url { url } => url.clone(),
    }
}

fn push_responses_events(val: &serde_json::Value, events: &mut Vec<BrainEvent>) {
    let event_type = val["type"].as_str().unwrap_or("");
    if let Some(delta) = val["delta"].as_str() {
        if event_type.contains("reasoning") || event_type.contains("thinking") {
            events.push(BrainEvent::ReasoningDelta(delta.to_string()));
        } else {
            events.push(BrainEvent::TextDelta(delta.to_string()));
        }
    }

    for key in [
        "reasoning_content",
        "reasoning",
        "thinking",
        "reasoning_summary_text",
    ] {
        if let Some(text) = val.get(key).and_then(|v| v.as_str()) {
            if !text.is_empty() {
                events.push(BrainEvent::ReasoningDelta(text.to_string()));
            }
        }
    }

    if let Some(response) = val.get("response") {
        collect_nested_reasoning(response, events);
    }
    if event_type == "response.completed" {
        events.push(BrainEvent::Done(sparrow_core::event::StopReason::EndTurn));
    }
}

fn collect_nested_reasoning(value: &serde_json::Value, events: &mut Vec<BrainEvent>) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                collect_nested_reasoning(item, events);
            }
        }
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                if matches!(
                    key.as_str(),
                    "reasoning_content" | "reasoning" | "thinking" | "reasoning_summary_text"
                ) {
                    if let Some(text) = value.as_str() {
                        if !text.is_empty() {
                            events.push(BrainEvent::ReasoningDelta(text.to_string()));
                        }
                    }
                }
                collect_nested_reasoning(value, events);
            }
        }
        _ => {}
    }
}

#[async_trait]
impl Brain for OpenAIResponsesAdapter {
    fn id(&self) -> &str {
        &self.model
    }

    fn caps(&self) -> ModelCaps {
        self.caps.clone()
    }

    async fn complete(&self, req: BrainRequest) -> anyhow::Result<BrainStream> {
        let body = build_responses_body(&self.model, &req);

        let response = self
            .client
            .post(format!("{}/responses", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "OpenAI Responses API error {}: {}",
                status,
                body
            ));
        }

        let stream = response.bytes_stream();
        // SSE frames split across TCP chunks must be reassembled — see
        // provider/sse_buffer.rs. Without this, words/characters mid-stream
        // silently disappear (the original symptom: streamed text mangling).
        let event_stream = futures::stream::unfold(
            (stream, false, super::sse_buffer::LineBuffer::new()),
            |(mut stream, done, mut buf)| async move {
                if done {
                    return None;
                }
                match futures::StreamExt::next(&mut stream).await {
                    Some(Ok(bytes)) => {
                        let mut events = Vec::new();
                        for line in buf.push(&bytes) {
                            let line = line.trim();
                            if line.is_empty() || !line.starts_with("data: ") {
                                continue;
                            }
                            let data = &line[6..];
                            if data == "[DONE]" {
                                events.push(BrainEvent::Done(sparrow_core::event::StopReason::EndTurn));
                                continue;
                            }
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
                                push_responses_events(&val, &mut events);
                            }
                        }
                        Some((futures::stream::iter(events), (stream, false, buf)))
                    }
                    Some(Err(e)) => Some((
                        futures::stream::iter(vec![BrainEvent::Error(format!(
                            "stream error: {}",
                            e
                        ))]),
                        (stream, true, buf),
                    )),
                    None => None,
                }
            },
        )
        .flatten();

        Ok(Box::pin(event_stream))
    }
}

// ─── AWS Bedrock adapter (Converse API) ────────────────────────────────────────

pub struct BedrockAdapter {
    model_id: String,
    // Kept for future SigV4 implementation; suppressed from dead-code warnings.
    #[allow(dead_code)]
    region: String,
    #[allow(dead_code)]
    access_key: String,
    #[allow(dead_code)]
    secret_key: String,
    #[allow(dead_code)]
    client: Client,
    caps: ModelCaps,
}

impl BedrockAdapter {
    pub fn new(
        model_id: &str,
        region: &str,
        access_key: impl Into<String>,
        secret_key: impl Into<String>,
    ) -> Self {
        let model_id = model_id.to_string();
        Self {
            model_id,
            region: region.to_string(),
            access_key: access_key.into(),
            secret_key: secret_key.into(),
            client: Client::new(),
            caps: ModelCaps {
                context_window: 200_000,
                max_output: 8_000,
                tools: true,
                vision: true,
                cost_input_per_mtok: 3.0,
                cost_output_per_mtok: 15.0,
                latency: LatencyClass::Medium,
            },
        }
    }
}

#[async_trait]
impl Brain for BedrockAdapter {
    fn id(&self) -> &str {
        &self.model_id
    }

    fn caps(&self) -> ModelCaps {
        self.caps.clone()
    }

    async fn complete(&self, _req: BrainRequest) -> anyhow::Result<BrainStream> {
        // Bedrock Converse requires AWS SigV4 request signing and parses an
        // EventStream binary frame format on the response. Neither is implemented
        // here — the previous code sent fake `X-Amz-Access-Key` / `X-Amz-Secret-Key`
        // headers (which Bedrock ignores) and wrapped raw response bytes in a
        // single `TextDelta`, producing garbled output even when authentication
        // somehow succeeded. Rather than ship a stub that silently fails, we
        // surface a clear error so callers can route around it.
        anyhow::bail!(
            "Bedrock provider is not implemented (model={}). \
             AWS SigV4 signing + Bedrock EventStream parsing are missing. \
             Use anthropic:* or openai:* directly, or pin a different provider in your config.",
            self.model_id
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ContentBlock, Msg, PromptCacheConfig, PromptCacheTtl};

    #[test]
    fn responses_body_adds_prompt_cache_controls() {
        let req = BrainRequest {
            system: Some("stable sparrow system".into()),
            messages: vec![Msg {
                role: "user".into(),
                content: vec![ContentBlock::Text {
                    text: "dynamic task".into(),
                }],
            }],
            cache: PromptCacheConfig {
                enabled: true,
                ttl: PromptCacheTtl::OneHour,
                key: Some("sparrow-repo-abc".into()),
            },
            ..BrainRequest::default()
        };

        let body = build_responses_body("gpt-test", &req);
        assert_eq!(body["prompt_cache_key"], "sparrow-repo-abc");
        assert_eq!(body["prompt_cache_retention"], "in_memory");
    }

    #[test]
    fn responses_body_reinjects_assistant_reasoning_content() {
        let req = BrainRequest {
            messages: vec![Msg {
                role: "assistant".into(),
                content: vec![
                    ContentBlock::Reasoning {
                        text: "private reasoning state".into(),
                    },
                    ContentBlock::Text {
                        text: "visible answer".into(),
                    },
                ],
            }],
            ..BrainRequest::default()
        };

        let body = build_responses_body("gpt-test", &req);
        assert_eq!(body["input"][0]["content"], "visible answer");
        assert_eq!(
            body["input"][0]["reasoning_content"],
            "private reasoning state"
        );
    }

    #[test]
    fn responses_body_serializes_image_blocks() {
        let req = BrainRequest {
            messages: vec![Msg {
                role: "user".into(),
                content: vec![
                    ContentBlock::Text {
                        text: "describe this".into(),
                    },
                    ContentBlock::Image {
                        source: crate::ImageSource::Base64 {
                            media_type: "image/png".into(),
                            data: "iVBORw0KGgo=".into(),
                        },
                    },
                ],
            }],
            ..BrainRequest::default()
        };

        let body = build_responses_body("gpt-test", &req);
        assert_eq!(body["input"][0]["content"][0]["type"], "input_text");
        assert_eq!(body["input"][0]["content"][1]["type"], "input_image");
        assert_eq!(
            body["input"][0]["content"][1]["image_url"],
            "data:image/png;base64,iVBORw0KGgo="
        );
    }

    #[test]
    fn responses_events_capture_reasoning_delta_without_visible_text() {
        let event = json!({
            "type": "response.reasoning_summary_text.delta",
            "delta": "reasoning chunk"
        });
        let mut events = Vec::new();
        push_responses_events(&event, &mut events);

        assert!(matches!(
            events.as_slice(),
            [BrainEvent::ReasoningDelta(text)] if text == "reasoning chunk"
        ));
    }
}
