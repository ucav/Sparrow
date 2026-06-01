//! Multimodal tools: image generation + text-to-speech (§15).
//!
//! Both target OpenAI-compatible endpoints so any provider that exposes
//! `/images/generations` or `/audio/speech` works. The user supplies the key
//! (env var, resolved at call time). No fake success: a missing key or a
//! non-2xx response returns a real error.

use async_trait::async_trait;
use serde_json::json;

use super::{Tool, ToolCtx, ToolResult};
use crate::event::{Block, RiskLevel};

fn resolve_key(env_names: &[&str]) -> Option<String> {
    for name in env_names {
        if let Ok(v) = std::env::var(name) {
            if !v.trim().is_empty() {
                return Some(v);
            }
        }
    }
    None
}

// ─── Image generation ─────────────────────────────────────────────────────────

/// Generate an image from a prompt via an OpenAI-compatible images endpoint.
pub struct ImageGen {
    base_url: String,
    model: String,
}

impl ImageGen {
    pub fn new() -> Self {
        Self {
            base_url: std::env::var("IMAGE_API_BASE")
                .unwrap_or_else(|_| "https://api.openai.com/v1".into()),
            model: std::env::var("IMAGE_MODEL").unwrap_or_else(|_| "gpt-image-1".into()),
        }
    }
}

#[async_trait]
impl Tool for ImageGen {
    fn name(&self) -> &str {
        "image_generate"
    }
    fn description(&self) -> &str {
        "Generate an image from a text prompt. Saves a PNG into the workspace and returns its path."
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "prompt": { "type": "string", "description": "Image description" },
                "filename": { "type": "string", "description": "Output filename (default: generated.png)" },
                "size": { "type": "string", "description": "e.g. 1024x1024" }
            },
            "required": ["prompt"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Network
    }
    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let Some(key) = resolve_key(&["IMAGE_API_KEY", "OPENAI_API_KEY"]) else {
            return Ok(ToolResult::error(
                "No image API key. Set IMAGE_API_KEY or OPENAI_API_KEY.",
            ));
        };
        let prompt = args["prompt"].as_str().unwrap_or("");
        let size = args["size"].as_str().unwrap_or("1024x1024");
        let filename = args["filename"].as_str().unwrap_or("generated.png");

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{}/images/generations", self.base_url.trim_end_matches('/')))
            .bearer_auth(&key)
            .json(&json!({
                "model": self.model,
                "prompt": prompt,
                "size": size,
                "n": 1,
                "response_format": "b64_json"
            }))
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Ok(ToolResult::error(format!(
                "image API error {}: {}",
                status, body
            )));
        }
        let value: serde_json::Value = resp.json().await?;
        let b64 = value["data"][0]["b64_json"].as_str();
        let url = value["data"][0]["url"].as_str();

        if let Some(b64) = b64 {
            let bytes = base64_decode::decode(b64)
                .map_err(|e| anyhow::anyhow!("invalid base64 image: {}", e))?;
            let path = super::resolve_workspace_path(&ctx.workspace_root, filename)?;
            std::fs::write(&path, &bytes)?;
            Ok(ToolResult::ok(vec![Block::Text(format!(
                "image saved to {} ({} bytes)",
                path.display(),
                bytes.len()
            ))]))
        } else if let Some(url) = url {
            Ok(ToolResult::ok(vec![Block::Text(format!(
                "image generated: {}",
                url
            ))]))
        } else {
            Ok(ToolResult::error("image API returned no data"))
        }
    }
}

// ─── Text to speech ─────────────────────────────────────────────────────────

pub struct Tts {
    base_url: String,
    model: String,
}

impl Tts {
    pub fn new() -> Self {
        Self {
            base_url: std::env::var("TTS_API_BASE")
                .unwrap_or_else(|_| "https://api.openai.com/v1".into()),
            model: std::env::var("TTS_MODEL").unwrap_or_else(|_| "gpt-4o-mini-tts".into()),
        }
    }
}

#[async_trait]
impl Tool for Tts {
    fn name(&self) -> &str {
        "text_to_speech"
    }
    fn description(&self) -> &str {
        "Synthesize speech from text via an OpenAI-compatible /audio/speech endpoint. Saves an audio file into the workspace."
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "text": { "type": "string" },
                "voice": { "type": "string", "description": "e.g. alloy" },
                "filename": { "type": "string", "description": "default: speech.mp3" }
            },
            "required": ["text"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Network
    }
    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let Some(key) = resolve_key(&["TTS_API_KEY", "OPENAI_API_KEY"]) else {
            return Ok(ToolResult::error(
                "No TTS API key. Set TTS_API_KEY or OPENAI_API_KEY.",
            ));
        };
        let text = args["text"].as_str().unwrap_or("");
        let voice = args["voice"].as_str().unwrap_or("alloy");
        let filename = args["filename"].as_str().unwrap_or("speech.mp3");

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{}/audio/speech", self.base_url.trim_end_matches('/')))
            .bearer_auth(&key)
            .json(&json!({ "model": self.model, "input": text, "voice": voice }))
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Ok(ToolResult::error(format!("tts API error {}: {}", status, body)));
        }
        let bytes = resp.bytes().await?;
        let path = super::resolve_workspace_path(&ctx.workspace_root, filename)?;
        std::fs::write(&path, &bytes)?;
        Ok(ToolResult::ok(vec![Block::Text(format!(
            "audio saved to {} ({} bytes)",
            path.display(),
            bytes.len()
        ))]))
    }
}

// Minimal base64 decoder (avoid adding a crate). Standard alphabet, no padding strictness.
mod base64_decode {
    pub fn decode(s: &str) -> Result<Vec<u8>, &'static str> {
        fn val(c: u8) -> Option<u8> {
            match c {
                b'A'..=b'Z' => Some(c - b'A'),
                b'a'..=b'z' => Some(c - b'a' + 26),
                b'0'..=b'9' => Some(c - b'0' + 52),
                b'+' => Some(62),
                b'/' => Some(63),
                _ => None,
            }
        }
        let mut out = Vec::with_capacity(s.len() / 4 * 3);
        let mut buf = 0u32;
        let mut bits = 0u32;
        for &c in s.as_bytes() {
            if c == b'=' || c == b'\n' || c == b'\r' {
                continue;
            }
            let v = match val(c) {
                Some(v) => v as u32,
                None => return Err("invalid base64 char"),
            };
            buf = (buf << 6) | v;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((buf >> bits) as u8);
            }
        }
        Ok(out)
    }
}
