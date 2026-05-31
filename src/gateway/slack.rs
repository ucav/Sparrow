use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use futures::{SinkExt, StreamExt};

use super::{GatewayMessage, GatewayResponse, GatewayTransport};

// ─── Slack Bot transport (Socket Mode) ──────────────────────────────────────────

pub struct SlackTransport {
    app_token: String,
    bot_token: String,
    allowed_users: Vec<String>,
}

impl SlackTransport {
    pub fn new(
        app_token: String,
        bot_token: String,
        allowed_users: Vec<String>,
    ) -> Self {
        Self {
            app_token,
            bot_token,
            allowed_users,
        }
    }

    /// Get WebSocket URL from Slack's apps.connections.open
    async fn get_socket_url(&self) -> anyhow::Result<String> {
        let client = reqwest::Client::new();
        let resp: serde_json::Value = client
            .post("https://slack.com/api/apps.connections.open")
            .header(
                "Authorization",
                format!("Bearer {}", self.app_token),
            )
            .send()
            .await?
            .json()
            .await?;

        if !resp["ok"].as_bool().unwrap_or(false) {
            anyhow::bail!(
                "Slack Socket Mode connection failed: {}",
                resp["error"].as_str().unwrap_or("unknown")
            );
        }

        Ok(resp["url"].as_str().unwrap_or("").to_string())
    }

    /// Post message via Slack Web API
    async fn post_message(
        &self,
        channel: &str,
        text: &str,
        buttons: &[Vec<String>],
    ) -> anyhow::Result<()> {
        let client = reqwest::Client::new();

        let mut blocks: Vec<serde_json::Value> = vec![serde_json::json!({
            "type": "section",
            "text": {
                "type": "mrkdwn",
                "text": text,
            }
        })];

        if !buttons.is_empty() {
            let elements: Vec<serde_json::Value> = buttons
                .iter()
                .flat_map(|row| {
                    row.iter().map(|label| {
                        serde_json::json!({
                            "type": "button",
                            "text": {
                                "type": "plain_text",
                                "text": label,
                            },
                            "value": label,
                            "action_id": label,
                        })
                    })
                })
                .collect();
            blocks.push(serde_json::json!({
                "type": "actions",
                "elements": elements,
            }));
        }

        client
            .post("https://slack.com/api/chat.postMessage")
            .header("Authorization", format!("Bearer {}", self.bot_token))
            .json(&serde_json::json!({
                "channel": channel,
                "blocks": blocks,
            }))
            .send()
            .await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl GatewayTransport for SlackTransport {
    fn name(&self) -> &str {
        "slack"
    }

    async fn start(
        &self,
        tx: mpsc::UnboundedSender<GatewayMessage>,
    ) -> anyhow::Result<()> {
        let allowed = self.allowed_users.clone();

        tracing::info!("Slack gateway starting (Socket Mode)");

        let socket_url = self.get_socket_url().await?;

        tokio::spawn(async move {
            match connect_async(&socket_url).await {
                Ok((mut ws_stream, _)) => {
                    while let Some(Ok(msg)) = ws_stream.next().await {
                        if let WsMessage::Text(text) = msg {
                            if let Ok(payload) =
                                serde_json::from_str::<serde_json::Value>(&text)
                            {
                                let event_type = payload["type"].as_str().unwrap_or("");

                                if event_type == "events_api" {
                                    let event =
                                        &payload["payload"]["event"];
                                    let ev_type =
                                        event["type"].as_str().unwrap_or("");

                                    if ev_type == "message"
                                        && event["subtype"].is_null()
                                    {
                                        let user = event["user"]
                                            .as_str()
                                            .unwrap_or("")
                                            .to_string();
                                        let channel = event["channel"]
                                            .as_str()
                                            .unwrap_or("")
                                            .to_string();
                                        let text = event["text"]
                                            .as_str()
                                            .unwrap_or("")
                                            .to_string();
                                        let ts = event["ts"]
                                            .as_str()
                                            .map(|s| s.to_string());

                                        if !allowed.is_empty() && !allowed.contains(&user) {
                                            continue;
                                        }

                                        if !text.is_empty() {
                                            let _ = tx.send(GatewayMessage {
                                                surface: "slack".into(),
                                                user_id: user,
                                                chat_id: channel,
                                                text,
                                                message_id: ts,
                                            });
                                        }
                                    }
                                }

                                // Send envelope ACK if needed
                                if let Some(envelope_id) =
                                    payload["envelope_id"].as_str()
                                {
                                    let ack = serde_json::json!({
                                        "envelope_id": envelope_id,
                                    });
                                    let _ = ws_stream
                                        .send(WsMessage::Text(
                                            ack.to_string().into(),
                                        ))
                                        .await;
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "Slack WebSocket connection failed: {}",
                        e
                    );
                }
            }
        });

        Ok(())
    }

    async fn send(&self, response: GatewayResponse) -> anyhow::Result<()> {
        self.post_message(&response.chat_id, &response.text, &response.buttons)
            .await
    }

    async fn stop(&self) -> anyhow::Result<()> {
        tracing::info!("Slack gateway stopped");
        Ok(())
    }
}
