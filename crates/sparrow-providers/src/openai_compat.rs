use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use reqwest::Client;
use serde_json::json;
use std::collections::HashMap;

use super::{Brain, BrainEvent, BrainRequest, BrainStream, ContentBlock, LatencyClass, ModelCaps};

/// Process-monotonic counter for synthesized tool-call ids (B8): markup-derived
/// and id-less native calls get a unique id so two turns in one run can't
/// collide on `markup-call-0` and confuse id-keyed approval/replay state.
static SYNTH_TOOL_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn next_synth_id(kind: &str) -> String {
    let n = SYNTH_TOOL_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{kind}-call-{n}")
}

/// Sorted indices of a tool-call accumulator, ascending. Used to emit
/// `ToolUseEnd` in the order the model declared the calls (index order), not
/// the arbitrary order a `HashMap` drains in (A1/A2).
fn sorted_indices(keys: impl Iterator<Item = u64>) -> Vec<u64> {
    let mut idxs: Vec<u64> = keys.collect();
    idxs.sort_unstable();
    idxs
}

/// OpenAI-compatible adapter. Covers OpenAI, Groq, NVIDIA NIM, Together, Cerebras,
/// OpenRouter, NovitaAI, Nous Portal, HuggingFace, Ollama, and custom endpoints.
pub struct OpenAICompatAdapter {
    model: String,
    api_key: String,
    base_url: String,
    client: Client,
    caps: ModelCaps,
    echo_reasoning: bool,
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
            echo_reasoning: true,
        }
    }

    pub fn with_caps(mut self, caps: ModelCaps) -> Self {
        self.caps = caps;
        self
    }

    pub fn with_echo_reasoning(mut self, echo_reasoning: bool) -> Self {
        self.echo_reasoning = echo_reasoning;
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

fn build_chat_body(model: &str, req: &BrainRequest, echo_reasoning: bool) -> serde_json::Value {
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
                ContentBlock::Reasoning { text } if echo_reasoning => {
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
                ContentBlock::Reasoning { .. } => {}
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
    // NOTE: we deliberately never emit `prompt_cache_key` / `prompt_cache_retention`
    // here. This one adapter fronts dozens of OpenAI-compatible endpoints and
    // proxies (opencode-go, NVIDIA NIM, Groq, stepfun, Ollama, …). Many reject
    // unknown parameters with HTTP 400 — e.g. opencode-go:
    //   "Validation: Unsupported parameter(s): prompt_cache_retention,
    //    prompt_cache_key"
    // which made the first model in every routing chain fail and waste a turn.
    // Prompt caching stays an Anthropic-only feature (handled in anthropic.rs,
    // which keys off `req.cache.enabled` independently). The engine may still
    // set `req.cache.enabled` for the run; this adapter simply ignores it.

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
        let body = build_chat_body(&self.model, &req, self.echo_reasoning);

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
            /// Accumulated assistant `content` text for this completion. Used
            /// to recover tool calls a provider emitted as inline XML/DSML
            /// markup inside `content` rather than as native `tool_calls`
            /// (see provider::tool_markup).
            content_buf: String,
            /// True once we've decided the content is inline tool-call markup
            /// and should be suppressed from the visible text stream.
            suppress_text: bool,
            /// Text held while the beginning of `content` is ambiguous: it may
            /// still become inline tool-call markup once more chunks arrive.
            pending_text: String,
            /// B4: true once reasoning has been seen on the streaming `delta`
            /// path. Providers also repeat the full reasoning under
            /// `message.reasoning_content` on the final chunk; without this
            /// flag the engine concatenated both and echoed doubled reasoning
            /// back (context bloat + 400 risk). We take delta OR message,
            /// never both.
            reasoning_seen: bool,
        }

        let event_stream = stream
            .scan(
                SseState {
                    tools: HashMap::new(),
                    lines: super::sse_buffer::LineBuffer::new(),
                    content_buf: String::new(),
                    suppress_text: false,
                    pending_text: String::new(),
                    reasoning_seen: false,
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
                                                    state.content_buf.push_str(text);
                                                    state.pending_text.push_str(text);
                                                    // If this completion's content turns
                                                    // out to be inline tool-call markup
                                                    // (DeepSeek DSML / Anthropic-style
                                                    // <invoke>), suppress it from the
                                                    // visible text stream — it'll be
                                                    // converted to real tool calls at
                                                    // finish_reason.
                                                    if !state.suppress_text
                                                        && super::tool_markup::looks_like_tool_markup(
                                                            &state.content_buf,
                                                        )
                                                    {
                                                        state.suppress_text = true;
                                                        state.pending_text.clear();
                                                    }
                                                    if !state.suppress_text
                                                        && !super::tool_markup::could_be_tool_markup_prefix(
                                                            &state.content_buf,
                                                        )
                                                        && !state.pending_text.is_empty()
                                                    {
                                                        parsed.push(BrainEvent::TextDelta(
                                                            std::mem::take(&mut state.pending_text),
                                                        ));
                                                    }
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
                                                        state.reasoning_seen = true;
                                                        parsed.push(BrainEvent::ReasoningDelta(
                                                            rtext.to_string(),
                                                        ));
                                                    }
                                                }
                                            }
                                        }
                                        // Some providers bundle the reasoning under
                                        // `message.reasoning_content` on the final chunk
                                        // rather than streaming it through `delta`. B4:
                                        // only use it when nothing streamed via delta —
                                        // otherwise it's the SAME trace repeated and
                                        // concatenating both doubles it.
                                        if !state.reasoning_seen {
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
                                                            state.reasoning_seen = true;
                                                            parsed.push(BrainEvent::ReasoningDelta(
                                                                rtext.to_string(),
                                                            ));
                                                        }
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
                                                                    // B8: unique even when
                                                                    // the provider omits the
                                                                    // id, across turns.
                                                                    state.id =
                                                                        next_synth_id("tool");
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
                                                    "stop" => {
                                                        // A2: a provider may stream native
                                                        // tool_calls and then finish with
                                                        // "stop" (not "tool_calls"). Drain
                                                        // any pending native calls FIRST so
                                                        // they actually execute instead of
                                                        // being silently dropped.
                                                        let mut native = false;
                                                        for idx in sorted_indices(
                                                            tool_state.keys().copied(),
                                                        ) {
                                                            if let Some(st) =
                                                                tool_state.remove(&idx)
                                                            {
                                                                if !st.id.is_empty() {
                                                                    parsed.push(
                                                                        BrainEvent::ToolUseEnd {
                                                                            id: st.id,
                                                                        },
                                                                    );
                                                                    native = true;
                                                                }
                                                            }
                                                        }
                                                        // Otherwise recover tool calls a
                                                        // provider emitted as inline
                                                        // XML/DSML markup in `content` (with
                                                        // finish_reason "stop") instead of
                                                        // native tool_calls — without this
                                                        // the call leaks as raw text and
                                                        // never runs.
                                                        let calls = if !native
                                                            && super::tool_markup::looks_like_tool_markup(
                                                                &state.content_buf,
                                                            )
                                                        {
                                                            super::tool_markup::extract_tool_calls(
                                                                &state.content_buf,
                                                            )
                                                        } else {
                                                            Vec::new()
                                                        };
                                                        if native {
                                                            sparrow_core::event::StopReason::ToolUse
                                                        } else if calls.is_empty() {
                                                            if !state.suppress_text
                                                                && !state.pending_text.is_empty()
                                                            {
                                                                parsed.push(
                                                                    BrainEvent::TextDelta(
                                                                        std::mem::take(
                                                                            &mut state.pending_text,
                                                                        ),
                                                                    ),
                                                                );
                                                            }
                                                            sparrow_core::event::StopReason::EndTurn
                                                        } else {
                                                            for call in calls.into_iter() {
                                                                // B8: unique id per
                                                                // synthesized call so two
                                                                // markup turns in one run
                                                                // never collide.
                                                                let id = next_synth_id("markup");
                                                                parsed.push(
                                                                    BrainEvent::ToolUseStart {
                                                                        id: id.clone(),
                                                                        name: call.name,
                                                                    },
                                                                );
                                                                parsed.push(
                                                                    BrainEvent::ToolUseDelta {
                                                                        id: id.clone(),
                                                                        json: call
                                                                            .args
                                                                            .to_string(),
                                                                    },
                                                                );
                                                                parsed.push(
                                                                    BrainEvent::ToolUseEnd { id },
                                                                );
                                                            }
                                                            sparrow_core::event::StopReason::ToolUse
                                                        }
                                                    }
                                                    "length" => sparrow_core::event::StopReason::MaxTokens,
                                                    "tool_calls" => {
                                                        // A1/A2: emit Ends in index order,
                                                        // not HashMap-arbitrary order.
                                                        for idx in sorted_indices(
                                                            tool_state.keys().copied(),
                                                        ) {
                                                            if let Some(st) =
                                                                tool_state.remove(&idx)
                                                            {
                                                                if !st.id.is_empty() {
                                                                    parsed.push(
                                                                        BrainEvent::ToolUseEnd {
                                                                            id: st.id,
                                                                        },
                                                                    );
                                                                }
                                                            }
                                                        }
                                                        sparrow_core::event::StopReason::ToolUse
                                                    }
                                                    s => sparrow_core::event::StopReason::StopSequence(
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
                                    parsed.push(BrainEvent::Usage(sparrow_core::event::TokenUsage {
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
    use crate::{Msg, PromptCacheConfig, PromptCacheTtl};
    use futures::StreamExt;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn openai_chat_body_never_sends_prompt_cache_params() {
        // Regression for the v0.8.x 400: many OpenAI-compatible proxies
        // (opencode-go, …) reject `prompt_cache_key`/`prompt_cache_retention`
        // with HTTP 400, which made the first routed model fail every run.
        // These params are Anthropic-only (see anthropic.rs); this adapter
        // must never emit them — even when the run enabled caching.
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

        let body = build_chat_body("gpt-test", &req, true);
        assert!(
            body.get("prompt_cache_key").is_none(),
            "prompt_cache_key must never be sent to an OpenAI-compatible endpoint"
        );
        assert!(
            body.get("prompt_cache_retention").is_none(),
            "prompt_cache_retention must never be sent to an OpenAI-compatible endpoint"
        );
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
                        source: crate::ImageSource::Base64 {
                            media_type: "image/png".into(),
                            data: "iVBORw0KGgo=".into(),
                        },
                    },
                ],
            }],
            ..BrainRequest::default()
        };

        let body = build_chat_body("gpt-test", &req, true);
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

        let body = build_chat_body("deepseek-test", &req, true);
        assert_eq!(body["messages"][0]["content"], "visible answer");
        assert_eq!(
            body["messages"][0]["reasoning_content"],
            "opaque provider reasoning"
        );
    }

    #[test]
    fn openai_chat_body_can_disable_reasoning_echo() {
        let req = BrainRequest {
            messages: vec![Msg {
                role: "assistant".into(),
                content: vec![
                    ContentBlock::Reasoning {
                        text: "provider-private reasoning".into(),
                    },
                    ContentBlock::Text {
                        text: "visible answer".into(),
                    },
                ],
            }],
            ..BrainRequest::default()
        };

        let body = build_chat_body("provider-no-echo", &req, false);
        assert_eq!(body["messages"][0]["content"], "visible answer");
        assert!(
            body["messages"][0].get("reasoning_content").is_none(),
            "provider flagged echo_reasoning=false must not receive reasoning_content"
        );
    }

    #[test]
    fn multi_tool_turn_is_one_assistant_message_with_reasoning() {
        // Regression for the v0.5.5 fix: a single model turn that emits N tool
        // calls must serialize as ONE assistant message carrying
        // reasoning_content + a tool_calls array of length N. Splitting it into
        // one message per tool dropped reasoning_content from the 2nd+ calls,
        // which DeepSeek/Qwen/Moonshot thinking-mode rejects with HTTP 400 and
        // which aborted multi-file tasks half-way.
        let req = BrainRequest {
            messages: vec![Msg {
                role: "assistant".into(),
                content: vec![
                    ContentBlock::Reasoning {
                        text: "thinking about two files".into(),
                    },
                    ContentBlock::ToolUse {
                        id: "call_0".into(),
                        name: "fs_write".into(),
                        input: serde_json::json!({"path": "reverse.py"}),
                    },
                    ContentBlock::ToolUse {
                        id: "call_1".into(),
                        name: "fs_write".into(),
                        input: serde_json::json!({"path": "test_reverse.py"}),
                    },
                ],
            }],
            ..BrainRequest::default()
        };

        let body = build_chat_body("deepseek-test", &req, true);
        // exactly one assistant message
        assert_eq!(body["messages"].as_array().unwrap().len(), 1);
        // reasoning_content present on it
        assert_eq!(
            body["messages"][0]["reasoning_content"],
            "thinking about two files"
        );
        // both tool calls in a single tool_calls array
        let calls = body["messages"][0]["tool_calls"].as_array().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0]["id"], "call_0");
        assert_eq!(calls[1]["id"], "call_1");
        assert_eq!(calls[0]["function"]["name"], "fs_write");
    }

    #[tokio::test]
    async fn b1_partial_markup_stream_never_emits_visible_text() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0_u8; 4096];
            let _ = socket.read(&mut buf).await.unwrap();
            let chunks = [
                "<",
                "｜｜DSML｜｜invoke name=\"read\">",
                "<｜｜DSML｜｜parameter name=\"file_path\" string=\"true\">",
                "config.py",
                "</｜｜DSML｜｜parameter>",
                "</｜｜DSML｜｜invoke>",
            ];
            let mut body = String::new();
            for chunk in chunks {
                body.push_str("data: ");
                body.push_str(
                    &serde_json::json!({
                        "choices": [{
                            "delta": {"content": chunk},
                            "finish_reason": null
                        }]
                    })
                    .to_string(),
                );
                body.push_str("\n\n");
            }
            body.push_str("data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n");
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });

        let adapter =
            OpenAICompatAdapter::new("deepseek-test", "test-key", &format!("http://{}", addr));
        let mut stream = adapter.complete(BrainRequest::default()).await.unwrap();

        let mut text = String::new();
        let mut tool_name = None;
        let mut tool_args = String::new();
        let mut done = None;
        while let Some(event) = stream.next().await {
            match event {
                BrainEvent::TextDelta(delta) => text.push_str(&delta),
                BrainEvent::ToolUseStart { name, .. } => tool_name = Some(name),
                BrainEvent::ToolUseDelta { json, .. } => tool_args.push_str(&json),
                BrainEvent::ToolUseEnd { .. } => {}
                BrainEvent::Done(reason) => done = Some(reason),
                other => panic!("unexpected event: {other:?}"),
            }
        }
        server.await.unwrap();

        assert_eq!(
            text, "",
            "partial inline markup must not leak as visible text"
        );
        assert_eq!(tool_name.as_deref(), Some("read"));
        let args: serde_json::Value = serde_json::from_str(&tool_args).unwrap();
        assert_eq!(args["file_path"], "config.py");
        assert!(matches!(done, Some(sparrow_core::event::StopReason::ToolUse)));
    }
}
