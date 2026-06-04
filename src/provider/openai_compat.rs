use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use reqwest::Client;
use serde_json::json;
use std::collections::HashMap;

use super::{Brain, BrainEvent, BrainRequest, BrainStream, ContentBlock, LatencyClass, ModelCaps};

/// OpenAI-compatible adapter. Covers OpenAI, Groq, NVIDIA NIM, Together, Cerebras,
/// OpenRouter, NovitaAI, Nous Portal, HuggingFace, Ollama, and custom endpoints.
pub struct OpenAICompatAdapter {
    model: String,
    api_key: String,
    base_url: String,
    client: Client,
    caps: ModelCaps,
}

impl OpenAICompatAdapter {
    pub fn new(model: &str, api_key: impl Into<String>, base_url: &str) -> Self {
        let model = model.to_string();
        Self {
            model,
            api_key: api_key.into(),
            base_url: base_url.to_string(),
            client: Client::new(),
            caps: ModelCaps::default(),
        }
    }

    pub fn with_caps(mut self, caps: ModelCaps) -> Self {
        self.caps = caps;
        self
    }

    /// Create an Ollama adapter (OpenAI-compatible API on localhost)
    pub fn ollama(model: &str, base_url: &str) -> Self {
        // Ollama doesn't require an API key
        Self::new(model, "ollama", base_url).with_caps(ModelCaps {
            context_window: 32_768,
            max_output: 8_000,
            tools: true,
            vision: false,
            cost_input_per_mtok: 0.0,
            cost_output_per_mtok: 0.0,
            latency: LatencyClass::Medium,
        })
    }
}

fn build_chat_body(model: &str, req: &BrainRequest) -> serde_json::Value {
    let mut messages: Vec<serde_json::Value> = Vec::new();

    // Add system message
    if let Some(sys) = &req.system {
        messages.push(json!({
            "role": "system",
            "content": sys,
        }));
    }

    // Convert messages
    for msg in &req.messages {
        if msg.role == "system" {
            messages.push(json!({
                "role": "system",
                "content": msg.content.iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            }));
            continue;
        }

        let mut content: Vec<serde_json::Value> = Vec::new();
        let mut tool_calls: Vec<serde_json::Value> = Vec::new();
        let mut reasoning_buf = String::new();
        let mut emitted_tool_result = false;

        for block in &msg.content {
            match block {
                ContentBlock::Text { text } => {
                    content.push(json!({"type": "text", "text": text}));
                }
                ContentBlock::Image { source } => {
                    content.push(json!({
                        "type": "image_url",
                        "image_url": {
                            "url": image_source_url(source),
                        }
                    }));
                }
                ContentBlock::Reasoning { text } => {
                    // DeepSeek / Moonshot / Qwen "thinking mode" require the
                    // model's previous reasoning_content to be echoed back
                    // on the next turn or the API rejects with 400. We aggregate
                    // all reasoning blocks of this message and ship them as a
                    // single `reasoning_content` field.
                    if !reasoning_buf.is_empty() {
                        reasoning_buf.push('\n');
                    }
                    reasoning_buf.push_str(text);
                }
                ContentBlock::ToolUse { id, name, input } => {
                    tool_calls.push(json!({
                        "id": id,
                        "type": "function",
                        "function": {
                            "name": name,
                            "arguments": serde_json::to_string(input).unwrap_or_default(),
                        }
                    }));
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    content: tool_content,
                    ..
                } => {
                    let text = tool_content
                        .iter()
                        .filter_map(|b| match b {
                            ContentBlock::Text { text } => Some(text.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": tool_use_id,
                        "content": text,
                    }));
                    emitted_tool_result = true;
                    continue; // tool results are separate messages
                }
            }
        }

        if emitted_tool_result && content.is_empty() && tool_calls.is_empty() {
            continue;
        }

        let mut msg_json = json!({ "role": msg.role });

        if !tool_calls.is_empty() {
            msg_json["tool_calls"] = json!(tool_calls);
        }
        if !content.is_empty() {
            if content.len() == 1 && content[0]["type"] == "text" {
                msg_json["content"] = json!(content[0]["text"]);
            } else {
                msg_json["content"] = json!(content);
            }
        }
        if !reasoning_buf.is_empty() && msg.role == "assistant" {
            msg_json["reasoning_content"] = json!(reasoning_buf);
        }

        messages.push(msg_json);
    }

    // Build tools
    let tools: Vec<serde_json::Value> = req
        .tools
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
        .collect();

    let mut body = json!({
        "model": model,
        "messages": messages,
        "stream": true,
        "stream_options": {
            "include_usage": true
        },
        "temperature": req.temperature,
    });

    if req.max_tokens > 0 {
        body["max_tokens"] = json!(req.max_tokens);
    }
    if !tools.is_empty() {
        body["tools"] = json!(tools);
    }
    if !req.stop.is_empty() {
        body["stop"] = json!(req.stop);
    }
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

#[async_trait]
impl Brain for OpenAICompatAdapter {
    fn id(&self) -> &str {
        &self.model
    }

    fn caps(&self) -> ModelCaps {
        self.caps.clone()
    }

    async fn complete(&self, req: BrainRequest) -> anyhow::Result<BrainStream> {
        let body = build_chat_body(&self.model, &req);

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "OpenAI-compatible API error {}: {}",
                status,
                body
            ));
        }

        #[derive(Default)]
        struct ToolCallState {
            id: String,
            started: bool,
        }

        let stream = response.bytes_stream();

        // SSE state: tool-call accumulator + line buffer that survives chunk
        // boundaries. Without the buffer, a JSON event split across two TCP
        // chunks was parsed in halves and silently dropped — producing the
        // "à rebours" → "àours" mangling.
        struct SseState {
            tools: HashMap<u64, ToolCallState>,
            lines: super::sse_buffer::LineBuffer,
        }

        let event_stream = stream
            .scan(
                SseState {
                    tools: HashMap::new(),
                    lines: super::sse_buffer::LineBuffer::new(),
                },
                |state, chunk| {
                    let events: Vec<BrainEvent> = match chunk {
                        Ok(bytes) => {
                            let lines = state.lines.push(&bytes);
                            let tool_state = &mut state.tools;
                            let mut parsed = Vec::new();
                            for line in lines {
                                let line = line.trim();
                                if line.is_empty() || !line.starts_with("data: ") {
                                    continue;
                                }
                                let data = &line[6..];
                                if data == "[DONE]" {
                                    continue;
                                }
                                let event: serde_json::Value = match serde_json::from_str(data) {
                                    Ok(v) => v,
                                    Err(e) => {
                                        tracing::debug!(
                                            "JSON parse error: {} — data: {}",
                                            e,
                                            &data[..data.len().min(200)]
                                        );
                                        continue;
                                    }
                                };

                                if let Some(choices) = event["choices"].as_array() {
                                    for choice in choices {
                                        if let Some(delta) = choice["delta"].as_object() {
                                            if let Some(text) =
                                                delta.get("content").and_then(|v| v.as_str())
                                            {
                                                if !text.is_empty() {
                                                    parsed.push(BrainEvent::TextDelta(
                                                        text.to_string(),
                                                    ));
                                                }
                                            }
                                            // DeepSeek / Moonshot thinking-mode emit
                                            // reasoning trace alongside content. Capture
                                            // it as a dedicated event so the engine can
                                            // echo it back on the next turn (required
                                            // by DeepSeek's contract).
                                            // Several providers report this under
                                            // different keys; check the known aliases.
                                            for key in [
                                                "reasoning_content",
                                                "reasoning",
                                                "thinking",
                                                "thought",
                                            ] {
                                                if let Some(rtext) =
                                                    delta.get(key).and_then(|v| v.as_str())
                                                {
                                                    if !rtext.is_empty() {
                                                        parsed.push(BrainEvent::ReasoningDelta(
                                                            rtext.to_string(),
                                                        ));
                                                    }
                                                }
                                            }
                                        }
                                        // Some providers (non-streaming chunk at end of
                                        // turn) bundle the reasoning under
                                        // `message.reasoning_content` rather than
                                        // streaming it through `delta`. Cover that path
                                        // too — duplicate captures are harmless because
                                        // the engine joins them.
                                        if let Some(msg_obj) =
                                            choice.get("message").and_then(|v| v.as_object())
                                        {
                                            for key in
                                                ["reasoning_content", "reasoning", "thinking"]
                                            {
                                                if let Some(rtext) =
                                                    msg_obj.get(key).and_then(|v| v.as_str())
                                                {
                                                    if !rtext.is_empty() {
                                                        parsed.push(BrainEvent::ReasoningDelta(
                                                            rtext.to_string(),
                                                        ));
                                                    }
                                                }
                                            }
                                        }
                                        if let Some(delta) = choice["delta"].as_object() {
                                            // (Re-open the original tool_calls block.)
                                            let _ = delta; // keep this branch syntactically anchored
                                            if let Some(tool_calls) =
                                                delta.get("tool_calls").and_then(|v| v.as_array())
                                            {
                                                for tc in tool_calls {
                                                    let idx = tc
                                                        .get("index")
                                                        .and_then(|v| v.as_u64())
                                                        .unwrap_or(0);
                                                    let id = tc
                                                        .get("id")
                                                        .and_then(|v| v.as_str())
                                                        .map(|s| s.to_string());
                                                    let state = tool_state.entry(idx).or_default();
                                                    if let Some(id) = id {
                                                        state.id = id;
                                                    }
                                                    if let Some(func) = tc
                                                        .get("function")
                                                        .and_then(|v| v.as_object())
                                                    {
                                                        if let Some(name) = func
                                                            .get("name")
                                                            .and_then(|v| v.as_str())
                                                        {
                                                            if !state.started {
                                                                if state.id.is_empty() {
                                                                    state.id = format!(
                                                                        "tool-call-{}",
                                                                        idx
                                                                    );
                                                                }
                                                                state.started = true;
                                                                parsed.push(
                                                                    BrainEvent::ToolUseStart {
                                                                        id: state.id.clone(),
                                                                        name: name.to_string(),
                                                                    },
                                                                );
                                                            }
                                                        }
                                                        if let Some(args) = func
                                                            .get("arguments")
                                                            .and_then(|v| v.as_str())
                                                        {
                                                            if !state.id.is_empty()
                                                                && !args.is_empty()
                                                            {
                                                                parsed.push(
                                                                    BrainEvent::ToolUseDelta {
                                                                        id: state.id.clone(),
                                                                        json: args.to_string(),
                                                                    },
                                                                );
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        if let Some(reason) =
                                            choice.get("finish_reason").and_then(|v| v.as_str())
                                        {
                                            if !reason.is_empty() && reason != "null" {
                                                let stop = match reason {
                                                    "stop" => crate::event::StopReason::EndTurn,
                                                    "length" => crate::event::StopReason::MaxTokens,
                                                    "tool_calls" => {
                                                        for (_, state) in tool_state.drain() {
                                                            if !state.id.is_empty() {
                                                                parsed.push(
                                                                    BrainEvent::ToolUseEnd {
                                                                        id: state.id,
                                                                    },
                                                                );
                                                            }
                                                        }
                                                        crate::event::StopReason::ToolUse
                                                    }
                                                    s => crate::event::StopReason::StopSequence(
                                                        s.to_string(),
                                                    ),
                                                };
                                                parsed.push(BrainEvent::Done(stop));
                                            }
                                        }
                                    }
                                }

                                if let Some(usage) = event.get("usage").and_then(|u| u.as_object())
                                {
                                    // Use .get() — indexing a serde_json::Map with [] panics on a
                                    // missing key, and some providers (e.g. MiniMax) omit fields.
                                    parsed.push(BrainEvent::Usage(crate::event::TokenUsage {
                                        input: usage
                                            .get("prompt_tokens")
                                            .and_then(|v| v.as_u64())
                                            .unwrap_or(0),
                                        output: usage
                                            .get("completion_tokens")
                                            .and_then(|v| v.as_u64())
                                            .unwrap_or(0),
                                    }));
                                }
                            }
                            parsed
                        }
                        Err(e) => vec![BrainEvent::Error(format!("stream error: {}", e))],
                    };
                    futures::future::ready(Some(stream::iter(events)))
                },
            )
            .flatten();

        Ok(Box::pin(event_stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{Msg, PromptCacheConfig, PromptCacheTtl};

    #[test]
    fn openai_chat_body_adds_prompt_cache_controls() {
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

        let body = build_chat_body("gpt-test", &req);
        assert_eq!(body["prompt_cache_key"], "sparrow-repo-abc");
        assert_eq!(body["prompt_cache_retention"], "in_memory");
    }

    #[test]
    fn openai_chat_body_serializes_image_blocks() {
        let req = BrainRequest {
            messages: vec![Msg {
                role: "user".into(),
                content: vec![
                    ContentBlock::Text {
                        text: "what is in this image?".into(),
                    },
                    ContentBlock::Image {
                        source: crate::provider::ImageSource::Base64 {
                            media_type: "image/png".into(),
                            data: "iVBORw0KGgo=".into(),
                        },
                    },
                ],
            }],
            ..BrainRequest::default()
        };

        let body = build_chat_body("gpt-test", &req);
        assert_eq!(body["messages"][0]["content"][0]["type"], "text");
        assert_eq!(body["messages"][0]["content"][1]["type"], "image_url");
        assert_eq!(
            body["messages"][0]["content"][1]["image_url"]["url"],
            "data:image/png;base64,iVBORw0KGgo="
        );
    }

    #[test]
    fn openai_chat_body_reinjects_assistant_reasoning_content() {
        let req = BrainRequest {
            messages: vec![Msg {
                role: "assistant".into(),
                content: vec![
                    ContentBlock::Reasoning {
                        text: "opaque provider reasoning".into(),
                    },
                    ContentBlock::Text {
                        text: "visible answer".into(),
                    },
                ],
            }],
            ..BrainRequest::default()
        };

        let body = build_chat_body("deepseek-test", &req);
        assert_eq!(body["messages"][0]["content"], "visible answer");
        assert_eq!(
            body["messages"][0]["reasoning_content"],
            "opaque provider reasoning"
        );
    }
}
