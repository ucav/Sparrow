use tokio::net::TcpListener;
use tokio::sync::mpsc;
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::accept_async;

use super::{GatewayMessage, GatewayResponse, GatewayTransport};

// ─── WebSocket API Server ───────────────────────────────────────────────────────

pub struct WebSocketApi {
    bind_addr: String,
}

impl WebSocketApi {
    pub fn new(bind_addr: impl Into<String>) -> Self {
        Self {
            bind_addr: bind_addr.into(),
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
        tracing::info!("WebSocket API listening on {}", self.bind_addr);

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        tracing::debug!("WS connection from {}", addr);
                        let tx = tx.clone();

                        tokio::spawn(async move {
                            match accept_async(stream).await {
                                Ok(ws_stream) => {
                                    let (mut write, mut read) = ws_stream.split();

                                    loop {
                                        match read.next().await {
                                            Some(Ok(msg)) => {
                                                if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
                                                    // Parse incoming message
                                                    let chat_id = addr.to_string();
                                                    let _ = tx.send(GatewayMessage {
                                                        surface: "ws-api".into(),
                                                        user_id: "ws-user".into(),
                                                        chat_id: chat_id.clone(),
                                                        text: text.to_string(),
                                                        message_id: None,
                                                    });

                                                    // Echo back as acknowledgment
                                                    let ack = format!("{{\"ack\": \"received\"}}");
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
        // For WS, responses are handled differently - we'd track connections
        // For M5, just log
        tracing::debug!(
            "WS send to {}: {}",
            response.chat_id,
            &response.text[..response.text.len().min(80)]
        );
        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        tracing::info!("WebSocket API stopped");
        Ok(())
    }
}
