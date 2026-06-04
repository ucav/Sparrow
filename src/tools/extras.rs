use std::path::PathBuf;

// ─── Additional tools for Hermes-parity (§15) ──────────────────────────────────

use crate::event::{Block, RiskLevel};
use crate::tools::{Tool, ToolCtx, ToolResult};

// ─── Browser automation tool ────────────────────────────────────────────────────

pub struct BrowserAutomation;

#[async_trait::async_trait]
impl Tool for BrowserAutomation {
    fn name(&self) -> &str {
        "browser"
    }
    fn description(&self) -> &str {
        "Control a Playwright headless browser to navigate pages, take screenshots, and interact with elements"
    }
    fn schema(&self) -> serde_json::Value {
        crate::tools::browser_sandbox::BrowserTool.schema()
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Network
    }
    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        crate::tools::browser_sandbox::BrowserTool
            .call(args, ctx)
            .await
    }
}

// ─── Vision input tool ──────────────────────────────────────────────────────────

pub struct VisionInput;

#[async_trait::async_trait]
impl Tool for VisionInput {
    fn name(&self) -> &str {
        "vision"
    }
    fn description(&self) -> &str {
        "Analyze an image file using the model's vision capabilities"
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to image file" },
                "question": { "type": "string", "description": "Question about the image" }
            },
            "required": ["path"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::ReadOnly
    }
    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let path = args["path"].as_str().unwrap_or("");
        let question = args["question"].as_str().unwrap_or("Describe this image");
        let full_path = ctx.workspace_root.join(path);

        if !full_path.exists() {
            return Ok(ToolResult::error(format!("Image not found: {}", path)));
        }

        // Read and base64-encode for vision models
        let data = std::fs::read(&full_path)?;
        let mime = mime_guess::from_path(&full_path)
            .first_or_octet_stream()
            .to_string();
        let _b64 = base64_encode(&data);

        Ok(ToolResult::ok(vec![
            Block::Text(format!(
                "Image loaded: {} ({} bytes, {})\nQuestion: {}\n\nBase64 data ready for vision model.",
                path,
                data.len(),
                mime,
                question
            )),
            Block::Image { data, mime },
        ]))
    }
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[(triple >> 18) as usize & 63] as char);
        result.push(CHARS[(triple >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            result.push(CHARS[(triple >> 6) as usize & 63] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[triple as usize & 63] as char);
        } else {
            result.push('=');
        }
    }
    result
}

// ─── Image generation tool (stub) ───────────────────────────────────────────────

pub struct ImageGeneration;

#[async_trait::async_trait]
impl Tool for ImageGeneration {
    fn name(&self) -> &str {
        "image_gen"
    }
    fn description(&self) -> &str {
        "Generate an image from a text prompt using the configured image model"
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "prompt": { "type": "string", "description": "Image generation prompt" },
                "size": { "type": "string", "enum": ["512x512", "1024x1024", "1792x1024"] }
            },
            "required": ["prompt"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Network
    }
    async fn call(&self, args: serde_json::Value, _ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let prompt = args["prompt"].as_str().unwrap_or("");
        let size = args["size"].as_str().unwrap_or("1024x1024");

        Ok(ToolResult::text(format!(
            "Image generation requested: '{}' ({})\n\
             Configure an image generation provider (DALL-E, Stable Diffusion, etc.) \
             or use an MCP server for image generation.",
            prompt, size
        )))
    }
}

// ─── Text-to-speech tool (stub) ─────────────────────────────────────────────────

pub struct TextToSpeech;

#[async_trait::async_trait]
impl Tool for TextToSpeech {
    fn name(&self) -> &str {
        "tts"
    }
    fn description(&self) -> &str {
        "Convert text to speech and return audio"
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "Text to speak" },
                "voice": { "type": "string", "description": "Voice name" }
            },
            "required": ["text"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Network
    }
    async fn call(&self, args: serde_json::Value, _ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let text = args["text"].as_str().unwrap_or("");
        let voice = args["voice"].as_str().unwrap_or("default");

        Ok(ToolResult::text(format!(
            "TTS requested: '{}' (voice: {})\n\
             Configure TTS provider (OpenAI TTS, ElevenLabs, etc.) \
             or use an MCP server for speech synthesis.",
            &text[..text.len().min(100)],
            voice
        )))
    }
}

// ─── Hot-reload config watcher ──────────────────────────────────────────────────

pub fn spawn_config_watcher(
    config_path: PathBuf,
    reload_tx: tokio::sync::mpsc::UnboundedSender<crate::config::Config>,
) {
    tokio::spawn(async move {
        let mut last_mtime = std::time::SystemTime::UNIX_EPOCH;
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            if let Ok(meta) = std::fs::metadata(&config_path) {
                if let Ok(mtime) = meta.modified() {
                    if mtime > last_mtime {
                        last_mtime = mtime;
                        if let Ok(content) = std::fs::read_to_string(&config_path) {
                            if let Ok(cfg) = toml::from_str::<crate::config::Config>(&content) {
                                tracing::info!("Config reloaded from disk");
                                let _ = reload_tx.send(cfg);
                            }
                        }
                    }
                }
            }
        }
    });
}

// ─── NDJSON output helper ───────────────────────────────────────────────────────

pub fn ndjson_output(event: &crate::event::Event) -> String {
    if !event.is_public() {
        return String::new();
    }
    match serde_json::to_string(event) {
        Ok(json) => format!("{}\n", json),
        Err(e) => format!("{{\"error\":\"{}\"}}\n", e),
    }
}

// ─── Multi-surface session bridge ───────────────────────────────────────────────

use std::sync::Arc;
use tokio::sync::Mutex;

pub struct SessionBridge {
    pub session_id: String,
    pub active_surface: Mutex<String>,
    pub pending_approvals: Mutex<Vec<crate::gateway::GatewayResponse>>,
    pub engine: Option<Arc<crate::engine::Engine>>,
}

impl SessionBridge {
    pub fn new() -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            active_surface: Mutex::new("cli".into()),
            pending_approvals: Mutex::new(Vec::new()),
            engine: None,
        }
    }

    pub async fn set_surface(&self, surface: &str) {
        *self.active_surface.lock().await = surface.to_string();
    }

    pub async fn add_approval(&self, response: crate::gateway::GatewayResponse) {
        self.pending_approvals.lock().await.push(response);
    }

    pub async fn drain_approvals(&self) -> Vec<crate::gateway::GatewayResponse> {
        self.pending_approvals.lock().await.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::ndjson_output;

    #[test]
    fn ndjson_output_suppresses_internal_reasoning_delta() {
        let event = crate::event::Event::ReasoningDelta {
            run: crate::event::RunId("run-1".into()),
            text: " hidden provider state".into(),
        };

        assert_eq!(ndjson_output(&event), "");
        assert!(!event.is_public());
    }

    #[test]
    fn ndjson_output_keeps_public_events() {
        let event = crate::event::Event::ThinkingDelta {
            run: crate::event::RunId("run-1".into()),
            text: "visible answer".into(),
        };

        let line = ndjson_output(&event);
        assert!(line.contains("\"ThinkingDelta\""));
        assert!(line.ends_with('\n'));
        assert!(event.is_public());
    }
}
