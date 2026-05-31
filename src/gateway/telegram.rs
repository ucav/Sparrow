use tokio::sync::mpsc;

use super::{GatewayMessage, GatewayResponse, GatewayTransport};

// ─── Telegram Bot transport ─────────────────────────────────────────────────────

pub struct TelegramTransport {
    bot_token: String,
    api_url: String,
    client: reqwest::Client,
    allowed_users: Vec<String>,
}

impl TelegramTransport {
    pub fn new(bot_token: String, allowed_users: Vec<String>) -> Self {
        let api_url = format!("https://api.telegram.org/bot{}", bot_token);
        Self {
            bot_token,
            api_url,
            client: reqwest::Client::new(),
            allowed_users,
        }
    }

    /// Send a message via Telegram Bot API
    async fn send_message(&self, chat_id: &str, text: &str) -> anyhow::Result<()> {
        self.client
            .post(format!("{}/sendMessage", self.api_url))
            .json(&serde_json::json!({
                "chat_id": chat_id,
                "text": text,
                "parse_mode": "Markdown",
            }))
            .send()
            .await?;
        Ok(())
    }

    /// Send a message with inline keyboard buttons
    async fn send_message_with_buttons(
        &self,
        chat_id: &str,
        text: &str,
        buttons: &[Vec<String>],
    ) -> anyhow::Result<()> {
        let inline_keyboard: Vec<Vec<serde_json::Value>> = buttons
            .iter()
            .map(|row| {
                row.iter()
                    .map(|b| {
                        serde_json::json!({
                            "text": b,
                            "callback_data": b,
                        })
                    })
                    .collect()
            })
            .collect();

        self.client
            .post(format!("{}/sendMessage", self.api_url))
            .json(&serde_json::json!({
                "chat_id": chat_id,
                "text": text,
                "parse_mode": "Markdown",
                "reply_markup": {
                    "inline_keyboard": inline_keyboard,
                }
            }))
            .send()
            .await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl GatewayTransport for TelegramTransport {
    fn name(&self) -> &str {
        "telegram"
    }

    async fn start(
        &self,
        tx: mpsc::UnboundedSender<GatewayMessage>,
    ) -> anyhow::Result<()> {
        let token = self.bot_token.clone();
        let api_url = self.api_url.clone();
        let client = self.client.clone();
        let allowed = self.allowed_users.clone();

        tracing::info!("Telegram gateway starting (bot token: {}...)", &token[..8]);

        tokio::spawn(async move {
            let mut offset: i64 = 0;

            loop {
                // Long polling: getUpdates
                let resp = client
                    .post(format!("{}/getUpdates", api_url))
                    .json(&serde_json::json!({
                        "offset": offset,
                        "timeout": 30,
                        "allowed_updates": ["message"],
                    }))
                    .send()
                    .await;

                match resp {
                    Ok(r) => {
                        if let Ok(json) = r.json::<serde_json::Value>().await {
                            if let Some(updates) = json["result"].as_array() {
                                for update in updates {
                                    if let Some(update_id) = update["update_id"].as_i64() {
                                        offset = update_id + 1;
                                    }

                                    if let Some(msg) = update["message"].as_object() {
                                        let chat = &msg["chat"];
                                        let chat_id = chat["id"]
                                            .as_i64()
                                            .map(|i| i.to_string())
                                            .unwrap_or_default();
                                        let user_id = msg["from"]["id"]
                                            .as_i64()
                                            .map(|i| i.to_string())
                                            .unwrap_or_default();
                                        let text = msg["text"]
                                            .as_str()
                                            .unwrap_or("")
                                            .to_string();
                                        let message_id = msg["message_id"]
                                            .as_i64()
                                            .map(|i| i.to_string());

                                        if !text.is_empty() {
                                            let _ = tx.send(GatewayMessage {
                                                surface: "telegram".into(),
                                                user_id,
                                                chat_id,
                                                text,
                                                message_id,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Telegram poll error: {}", e);
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    }
                }
            }
        });

        Ok(())
    }

    async fn send(&self, response: GatewayResponse) -> anyhow::Result<()> {
        if response.buttons.is_empty() {
            self.send_message(&response.chat_id, &response.text).await
        } else {
            self.send_message_with_buttons(
                &response.chat_id,
                &response.text,
                &response.buttons,
            )
            .await
        }
    }

    async fn stop(&self) -> anyhow::Result<()> {
        tracing::info!("Telegram gateway stopped");
        Ok(())
    }
}
