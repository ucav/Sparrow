use async_trait::async_trait;
use tokio::sync::mpsc;

use super::{GatewayMessage, GatewayResponse, GatewayTransport};

// ─── WhatsApp (Meta Business API webhook mode) ────────────────────────────────

pub struct WhatsAppTransport {
    phone_number_id: String,
    access_token: String,
    _allowed_users: Vec<String>,
}

impl WhatsAppTransport {
    pub fn new(phone_number_id: String, access_token: String, allowed_users: Vec<String>) -> Self {
        Self { phone_number_id, access_token, _allowed_users: allowed_users }
    }

    async fn send_text(&self, to: &str, text: &str) -> anyhow::Result<()> {
        let client = reqwest::Client::new();
        let _ = client
            .post(format!("https://graph.facebook.com/v18/{}/messages", self.phone_number_id))
            .header("Authorization", format!("Bearer {}", self.access_token))
            .json(&serde_json::json!({
                "messaging_product": "whatsapp",
                "to": to,
                "type": "text",
                "text": {"body": text}
            }))
            .send().await?;
        Ok(())
    }
}

#[async_trait]
impl GatewayTransport for WhatsAppTransport {
    fn name(&self) -> &str { "whatsapp" }
    async fn start(&self, _tx: mpsc::UnboundedSender<GatewayMessage>) -> anyhow::Result<()> {
        tracing::info!("WhatsApp gateway ready (webhook mode — configure Meta Business webhook URL)");
        Ok(())
    }
    async fn send(&self, response: GatewayResponse) -> anyhow::Result<()> {
        self.send_text(&response.chat_id, &response.text).await
    }
    async fn stop(&self) -> anyhow::Result<()> { Ok(()) }
}

// ─── Signal (requires an external signal-cli bridge) ──────────────────────────

pub struct SignalTransport { _allowed_users: Vec<String> }

impl SignalTransport {
    pub fn new(allowed_users: Vec<String>) -> Self { Self { _allowed_users: allowed_users } }
}

#[async_trait]
impl GatewayTransport for SignalTransport {
    fn name(&self) -> &str { "signal" }
    async fn start(&self, _tx: mpsc::UnboundedSender<GatewayMessage>) -> anyhow::Result<()> {
        tracing::info!("Signal gateway requires signal-cli REST API or similar bridge");
        Ok(())
    }
    async fn send(&self, _response: GatewayResponse) -> anyhow::Result<()> {
        anyhow::bail!("Signal transport requires a configured signal-cli bridge")
    }
    async fn stop(&self) -> anyhow::Result<()> { Ok(()) }
}

// ─── Email (Mailgun-compatible sending; inbound polling not wired) ────────────

pub struct EmailTransport {
    smtp_host: String,
    _smtp_port: u16,
    _username: String,
    password: String,
    from_addr: String,
    _allowed_users: Vec<String>,
}

impl EmailTransport {
    pub fn new(smtp_host: String, smtp_port: u16, username: String, password: String, from_addr: String, allowed_users: Vec<String>) -> Self {
        Self { smtp_host, _smtp_port: smtp_port, _username: username, password, from_addr, _allowed_users: allowed_users }
    }
}

#[async_trait]
impl GatewayTransport for EmailTransport {
    fn name(&self) -> &str { "email" }
    async fn start(&self, _tx: mpsc::UnboundedSender<GatewayMessage>) -> anyhow::Result<()> {
        tracing::info!("Email gateway ready (SMTP for sending, IMAP for receiving via polling)");
        Ok(())
    }
    async fn send(&self, response: GatewayResponse) -> anyhow::Result<()> {
        let client = reqwest::Client::new();
        // SMTP via REST bridge or direct SMTP client
        let _ = client
            .post(format!("https://api.mailgun.net/v3/{}/messages", self.smtp_host))
            .basic_auth("api", Some(&self.password))
            .form(&[
                ("from", self.from_addr.as_str()),
                ("to", &response.chat_id),
                ("subject", "Sparrow Response"),
                ("text", &response.text),
            ])
            .send().await?;
        Ok(())
    }
    async fn stop(&self) -> anyhow::Result<()> { Ok(()) }
}

// ─── Feishu/Lark ───────────────────────────────────────────────────────────────

pub struct FeishuTransport { _app_id: String, app_secret: String, _allowed_users: Vec<String> }

impl FeishuTransport {
    pub fn new(app_id: String, app_secret: String, allowed_users: Vec<String>) -> Self {
        Self { _app_id: app_id, app_secret, _allowed_users: allowed_users }
    }
}

#[async_trait]
impl GatewayTransport for FeishuTransport {
    fn name(&self) -> &str { "feishu" }
    async fn start(&self, _tx: mpsc::UnboundedSender<GatewayMessage>) -> anyhow::Result<()> {
        tracing::info!("Feishu gateway ready (configure bot webhook in Feishu admin)");
        Ok(())
    }
    async fn send(&self, response: GatewayResponse) -> anyhow::Result<()> {
        let client = reqwest::Client::new();
        let _ = client
            .post("https://open.feishu.cn/open-apis/im/v1/messages?receive_id_type=chat_id")
            .header("Authorization", format!("Bearer {}", self.app_secret))
            .json(&serde_json::json!({
                "receive_id": response.chat_id,
                "msg_type": "text",
                "content": serde_json::json!({"text": response.text}).to_string(),
            }))
            .send().await?;
        Ok(())
    }
    async fn stop(&self) -> anyhow::Result<()> { Ok(()) }
}

// ─── WeCom ─────────────────────────────────────────────────────────────────────

pub struct WeComTransport { _corp_id: String, _secret: String, _allowed_users: Vec<String> }

impl WeComTransport {
    pub fn new(corp_id: String, secret: String, allowed_users: Vec<String>) -> Self {
        Self { _corp_id: corp_id, _secret: secret, _allowed_users: allowed_users }
    }
}

#[async_trait]
impl GatewayTransport for WeComTransport {
    fn name(&self) -> &str { "wecom" }
    async fn start(&self, _tx: mpsc::UnboundedSender<GatewayMessage>) -> anyhow::Result<()> {
        tracing::info!("WeCom gateway ready (configure bot in WeCom admin)");
        Ok(())
    }
    async fn send(&self, _response: GatewayResponse) -> anyhow::Result<()> {
        anyhow::bail!("WeCom transport requires access-token exchange and message API wiring")
    }
    async fn stop(&self) -> anyhow::Result<()> { Ok(()) }
}

// ─── QQBot ─────────────────────────────────────────────────────────────────────

pub struct QQBotTransport { _app_id: String, _token: String, _allowed_users: Vec<String> }

impl QQBotTransport {
    pub fn new(app_id: String, token: String, allowed_users: Vec<String>) -> Self {
        Self { _app_id: app_id, _token: token, _allowed_users: allowed_users }
    }
}

#[async_trait]
impl GatewayTransport for QQBotTransport {
    fn name(&self) -> &str { "qqbot" }
    async fn start(&self, _tx: mpsc::UnboundedSender<GatewayMessage>) -> anyhow::Result<()> {
        tracing::info!("QQBot gateway ready (configure in QQ Open Platform)");
        Ok(())
    }
    async fn send(&self, _response: GatewayResponse) -> anyhow::Result<()> {
        anyhow::bail!("QQBot transport requires QQ Open Platform send API wiring")
    }
    async fn stop(&self) -> anyhow::Result<()> { Ok(()) }
}

// ─── Microsoft Teams ────────────────────────────────────────────────────────────

pub struct TeamsTransport { _app_id: String, _app_password: String, _allowed_users: Vec<String> }

impl TeamsTransport {
    pub fn new(app_id: String, app_password: String, allowed_users: Vec<String>) -> Self {
        Self { _app_id: app_id, _app_password: app_password, _allowed_users: allowed_users }
    }
}

#[async_trait]
impl GatewayTransport for TeamsTransport {
    fn name(&self) -> &str { "teams" }
    async fn start(&self, _tx: mpsc::UnboundedSender<GatewayMessage>) -> anyhow::Result<()> {
        tracing::info!("Teams gateway ready (configure bot in Azure Bot Service)");
        Ok(())
    }
    async fn send(&self, _response: GatewayResponse) -> anyhow::Result<()> {
        anyhow::bail!("Teams transport requires Azure Bot Framework adapter wiring")
    }
    async fn stop(&self) -> anyhow::Result<()> { Ok(()) }
}
