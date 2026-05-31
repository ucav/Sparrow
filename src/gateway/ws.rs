use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Mutex};
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::accept_async;

use super::{GatewayMessage, GatewayResponse, GatewayTransport};

// ─── WebSocket API Server ───────────────────────────────────────────────────────

pub struct WebSocketApi {
    bind_addr: String,
    clients: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<String>>>>,
}

impl WebSocketApi {
    pub fn new(bind_addr: impl Into<String>) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            clients: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl GatewayTransport for WebSocketApi {
    fn name(&self) -> &str {
        "ws-api"
    }

    async fn start(
        &self,
        tx: mpsc::UnboundedSender<GatewayMessage>,
    ) -> anyhow::Result<()> {
        let listener = TcpListener::bind(&self.bind_addr).await?;
        let clients = self.clients.clone();
        tracing::info!("WebSocket API listening on {}", self.bind_addr);

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        tracing::debug!("WS connection from {}", addr);
                        let tx = tx.clone();
                        let clients = clients.clone();

                        tokio::spawn(async move {
                            match accept_async(stream).await {
                                Ok(ws_stream) => {
                                    let (mut write, mut read) = ws_stream.split();
                                    let chat_id = addr.to_string();
                                    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();
                                    clients.lock().await.insert(chat_id.clone(), out_tx);

                                    loop {
                                        tokio::select! {
                                            Some(outbound) = out_rx.recv() => {
                                                if write
                                                    .send(tokio_tungstenite::tungstenite::Message::Text(outbound.into()))
                                                    .await
                                                    .is_err()
                                                {
                                                    break;
                                                }
                                            }
                                            incoming = read.next() => {
                                                match incoming {
                                                    Some(Ok(msg)) => {
                                                        if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
                                                            let _ = tx.send(GatewayMessage {
                                                                surface: "ws-api".into(),
                                                                user_id: "ws-user".into(),
                                                                chat_id: chat_id.clone(),
                                                                text: text.to_string(),
                                                                message_id: None,
                                                            });

                                                            let ack = serde_json::json!({"ack": "received"}).to_string();
                                                            let _ = write
                                                                .send(tokio_tungstenite::tungstenite::Message::Text(ack.into()))
                                                                .await;
                                                        }
                                                    }
                                                    Some(Err(e)) => {
                                                        tracing::error!("WS error: {}", e);
                                                        break;
                                                    }
                                                    None => break,
                                                }
                                            }
                                        }
                                    }
                                    clients.lock().await.remove(&chat_id);
                                }
                                Err(e) => {
                                    tracing::error!("WS handshake error: {}", e);
                                }
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("Accept error: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    async fn send(&self, response: GatewayResponse) -> anyhow::Result<()> {
        let payload = serde_json::json!({
            "type": "message",
            "text": response.text,
            "reply_to": response.reply_to,
            "buttons": response.buttons,
        })
        .to_string();

        if let Some(client) = self.clients.lock().await.get(&response.chat_id).cloned() {
            client
                .send(payload)
                .map_err(|_| anyhow::anyhow!("WebSocket client is no longer connected"))?;
            Ok(())
        } else {
            anyhow::bail!("WebSocket client not connected: {}", response.chat_id)
        }
    }

    async fn stop(&self) -> anyhow::Result<()> {
        tracing::info!("WebSocket API stopped");
        Ok(())
    }
}
