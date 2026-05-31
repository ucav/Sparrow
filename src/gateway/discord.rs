use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use futures::{SinkExt, StreamExt};

use super::{GatewayMessage, GatewayResponse, GatewayTransport};

// ─── Discord Bot transport ──────────────────────────────────────────────────────

pub struct DiscordTransport {
    bot_token: String,
    allowed_users: Vec<String>,
}

impl DiscordTransport {
    pub fn new(bot_token: String, allowed_users: Vec<String>) -> Self {
        Self {
            bot_token,
            allowed_users,
        }
    }

    /// Get the WebSocket gateway URL from Discord API
    async fn get_gateway_url(&self) -> anyhow::Result<String> {
        let client = reqwest::Client::new();
        let resp: serde_json::Value = client
            .get("https://discord.com/api/v10/gateway/bot")
            .header("Authorization", format!("Bot {}", self.bot_token))
            .send()
            .await?
            .json()
            .await?;

        let url = resp["url"]
            .as_str()
            .unwrap_or("wss://gateway.discord.gg/?v=10&encoding=json")
            .to_string();

        Ok(format!("{}?v=10&encoding=json", url.trim_end_matches("?v=10&encoding=json")))
    }
}

#[async_trait::async_trait]
impl GatewayTransport for DiscordTransport {
    fn name(&self) -> &str {
        "discord"
    }

    async fn start(
        &self,
        tx: mpsc::UnboundedSender<GatewayMessage>,
    ) -> anyhow::Result<()> {
        let token = self.bot_token.clone();
        let allowed = self.allowed_users.clone();

        tracing::info!("Discord gateway starting");

        let gateway_url = self.get_gateway_url().await?;

        tokio::spawn(async move {
            match connect_async(&gateway_url).await {
                Ok((mut ws_stream, _)) => {
                    // Wait for Hello (opcode 10)
                    let mut heartbeat_interval: u64 = 41250;
                    let mut seq: Option<u64> = None;

                    while let Some(Ok(msg)) = ws_stream.next().await {
                        match msg {
                            WsMessage::Text(text) => {
                                if let Ok(payload) =
                                    serde_json::from_str::<serde_json::Value>(&text)
                                {
                                    let op = payload["op"].as_u64().unwrap_or(0);

                                    match op {
                                        10 => {
                                            // Hello: start heartbeat
                                            heartbeat_interval = payload["d"]["heartbeat_interval"]
                                                .as_u64()
                                                .unwrap_or(41250);

                                            // Send Identify
                                            let identify = serde_json::json!({
                                                "op": 2,
                                                "d": {
                                                    "token": token,
                                                    "intents": 1 << 9, // GUILD_MESSAGES
                                                    "properties": {
                                                        "os": "linux",
                                                        "browser": "sparrow",
                                                        "device": "sparrow"
                                                    }
                                                }
                                            });
                                            let _ = ws_stream
                                                .send(WsMessage::Text(
                                                    identify.to_string().into(),
                                                ))
                                                .await;
                                        }
                                        11 => {
                                            // Heartbeat ACK
                                        }
                                        0 => {
                                            // Dispatch
                                            seq = payload["s"].as_u64();
                                            let event_type = payload["t"]
                                                .as_str()
                                                .unwrap_or("");

                                            if event_type == "MESSAGE_CREATE" {
                                                let author_id = payload["d"]["author"]["id"]
                                                    .as_str()
                                                    .unwrap_or("")
                                                    .to_string();
                                                let channel_id = payload["d"]["channel_id"]
                                                    .as_str()
                                                    .unwrap_or("")
                                                    .to_string();
                                                let content = payload["d"]["content"]
                                                    .as_str()
                                                    .unwrap_or("")
                                                    .to_string();
                                                let message_id = payload["d"]["id"]
                                                    .as_str()
                                                    .map(|s| s.to_string());

                                                // Ignore bot's own messages
                                                if !content.is_empty() && author_id != token {
                                                    let _ = tx.send(GatewayMessage {
                                                        surface: "discord".into(),
                                                        user_id: author_id,
                                                        chat_id: channel_id,
                                                        text: content,
                                                        message_id,
                                                    });
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            WsMessage::Ping(data) => {
                                let _ = ws_stream.send(WsMessage::Pong(data)).await;
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Discord WebSocket connection failed: {}", e);
                }
            }
        });

        Ok(())
    }

    async fn send(&self, response: GatewayResponse) -> anyhow::Result<()> {
        let client = reqwest::Client::new();
        let url = format!(
            "https://discord.com/api/v10/channels/{}/messages",
            response.chat_id
        );

        let mut payload = serde_json::json!({
            "content": response.text,
        });

        if !response.buttons.is_empty() {
            // Discord uses message components for buttons
            let components: Vec<serde_json::Value> = vec![serde_json::json!({
                "type": 1,
                "components": response.buttons.iter().map(|row| {
                    row.iter().map(|label| {
                        serde_json::json!({
                            "type": 2,
                            "label": label,
                            "style": 1,
                            "custom_id": label,
                        })
                    }).collect::<Vec<_>>()
                }).collect::<Vec<_>>(),
            })];
            payload["components"] = serde_json::json!(components);
        }

        client
            .post(&url)
            .header("Authorization", format!("Bot {}", self.bot_token))
            .json(&payload)
            .send()
            .await?;

        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        tracing::info!("Discord gateway stopped");
        Ok(())
    }
}
