use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use futures::StreamExt;

use super::{
    Brain, BrainEvent, BrainRequest, BrainStream, LatencyClass, ModelCaps,
};

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
    pub fn new(
        model: &str,
        api_key: impl Into<String>,
        base_url: Option<&str>,
    ) -> Self {
        let model = model.to_string();
        Self {
            model,
            api_key: api_key.into(),
            base_url: base_url
                .unwrap_or("https://api.openai.com/v1")
                .to_string(),
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
        // Convert to Responses API format
        let mut input: Vec<serde_json::Value> = Vec::new();

        if let Some(sys) = &req.system {
            input.push(json!({
                "role": "system",
                "content": sys,
            }));
        }

        for msg in &req.messages {
            let content: String = msg
                .content
                .iter()
                .filter_map(|b| match b {
                    super::ContentBlock::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");

            input.push(json!({
                "role": msg.role,
                "content": content,
            }));
        }

        let body = json!({
            "model": self.model,
            "input": input,
            "stream": true,
            "temperature": req.temperature,
            "max_output_tokens": req.max_tokens,
        });

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
        let event_stream = futures::stream::unfold(
            (stream, false),
            |(mut stream, done)| async move {
                if done {
                    return None;
                }
                match futures::StreamExt::next(&mut stream).await {
                    Some(Ok(bytes)) => {
                        let text = String::from_utf8_lossy(&bytes);
                        let mut events = Vec::new();
                        for line in text.lines() {
                            let line = line.trim();
                            if line.is_empty() || !line.starts_with("data: ") {
                                continue;
                            }
                            let data = &line[6..];
                            if data == "[DONE]" {
                                events.push(BrainEvent::Done(
                                    crate::event::StopReason::EndTurn,
                                ));
                                continue;
                            }
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
                                if let Some(delta) = val["delta"].as_str() {
                                    events.push(BrainEvent::TextDelta(delta.to_string()));
                                }
                                if val["type"].as_str() == Some("response.completed") {
                                    events.push(BrainEvent::Done(
                                        crate::event::StopReason::EndTurn,
                                    ));
                                }
                            }
                        }
                        Some((futures::stream::iter(events), (stream, false)))
                    }
                    Some(Err(e)) => Some((
                        futures::stream::iter(vec![BrainEvent::Error(format!(
                            "stream error: {}",
                            e
                        ))]),
                        (stream, true),
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
    region: String,
    access_key: String,
    secret_key: String,
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

    async fn complete(&self, req: BrainRequest) -> anyhow::Result<BrainStream> {
        // Bedrock Converse API uses AWS Signature V4.
        // For M6+, we provide a simplified HTTP bridge.
        let url = format!(
            "https://bedrock-runtime.{}.amazonaws.com/model/{}/converse-stream",
            self.region, self.model_id
        );

        let mut messages: Vec<serde_json::Value> = Vec::new();
        for msg in &req.messages {
            messages.push(json!({
                "role": msg.role,
                "content": msg.content.iter().filter_map(|b| match b {
                    super::ContentBlock::Text { text } => Some(json!({"text": text})),
                    _ => None,
                }).collect::<Vec<_>>(),
            }));
        }

        let body = json!({
            "messages": messages,
            "system": req.system.map(|s| json!({"text": s})),
        });

        // In production, sign with AWS SigV4.
        // M6+ stub: uses pre-signed URL or IAM role.
        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("X-Amz-Access-Key", &self.access_key)
            .header("X-Amz-Secret-Key", &self.secret_key)
            .json(&body)
            .send()
            .await?;

        let status = response.status().as_u16();
        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Bedrock API error {}: {}",
                status,
                body
            ));
        }

        let stream = response.bytes_stream();
        let event_stream = futures::stream::unfold(
            (stream, false),
            |(mut stream, done)| async move {
                if done {
                    return None;
                }
                match futures::StreamExt::next(&mut stream).await {
                    Some(Ok(bytes)) => {
                        let text = String::from_utf8_lossy(&bytes);
                        let events = vec![BrainEvent::TextDelta(text.to_string())];
                        Some((futures::stream::iter(events), (stream, false)))
                    }
                    Some(Err(e)) => Some((
                        futures::stream::iter(vec![BrainEvent::Error(format!(
                            "stream error: {}",
                            e
                        ))]),
                        (stream, true),
                    )),
                    None => None,
                }
            },
        )
        .flatten();

        Ok(Box::pin(event_stream))
    }
}
