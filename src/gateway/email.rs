//! Email gateway transport (§3.16).
//!
//! v1 supports SMTP outbound only — useful for notifications from scheduled
//! jobs and approval prompts. Inbound (IMAP polling) is planned but requires
//! stateful protocol handling; for now, inbound flows should be wired via
//! webhook (Mailgun/Postmark/SES inbound parsing) which can POST into the
//! gateway WS API.
//!
//! Enabled via the `email` cargo feature.

use async_trait::async_trait;
use tokio::sync::mpsc;

use super::{GatewayMessage, GatewayResponse, GatewayTransport};

#[cfg(feature = "email")]
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
    message::{Mailbox, Message, header::ContentType},
    transport::smtp::authentication::Credentials,
};

/// SMTP credentials and recipients for outbound email.
pub struct EmailTransport {
    pub from: String,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub username: String,
    pub password: String,
    pub allowed_to: Vec<String>,
}

impl EmailTransport {
    pub fn new(
        from: String,
        smtp_host: String,
        smtp_port: u16,
        username: String,
        password: String,
        allowed_to: Vec<String>,
    ) -> Self {
        Self {
            from,
            smtp_host,
            smtp_port,
            username,
            password,
            allowed_to,
        }
    }
}

#[async_trait]
impl GatewayTransport for EmailTransport {
    fn name(&self) -> &str {
        "email"
    }

    async fn start(&self, _tx: mpsc::UnboundedSender<GatewayMessage>) -> anyhow::Result<()> {
        // No inbound polling in v1 (see file doc-comment). Inbound is wired via
        // webhook → WS API → MessageRouter for now.
        tracing::info!(
            "Email gateway: outbound only via SMTP {}:{} (from: {})",
            self.smtp_host,
            self.smtp_port,
            self.from
        );
        Ok(())
    }

    #[cfg(feature = "email")]
    async fn send(&self, response: GatewayResponse) -> anyhow::Result<()> {
        // Allowlist check
        if !self.allowed_to.iter().any(|a| a == &response.chat_id) {
            anyhow::bail!(
                "email recipient {} not in allowed_to list",
                response.chat_id
            );
        }

        let from: Mailbox = self.from.parse()?;
        let to: Mailbox = response.chat_id.parse()?;

        let subject = response
            .text
            .lines()
            .next()
            .unwrap_or("Sparrow update")
            .chars()
            .take(120)
            .collect::<String>();

        let email = Message::builder()
            .from(from)
            .to(to)
            .subject(subject)
            .header(ContentType::TEXT_PLAIN)
            .body(response.text)?;

        let creds = Credentials::new(self.username.clone(), self.password.clone());
        let mailer: AsyncSmtpTransport<Tokio1Executor> =
            AsyncSmtpTransport::<Tokio1Executor>::relay(&self.smtp_host)?
                .port(self.smtp_port)
                .credentials(creds)
                .build();

        mailer.send(email).await?;
        Ok(())
    }

    #[cfg(not(feature = "email"))]
    async fn send(&self, _response: GatewayResponse) -> anyhow::Result<()> {
        anyhow::bail!("email feature not enabled — rebuild with `cargo build --features email`")
    }

    async fn stop(&self) -> anyhow::Result<()> {
        Ok(())
    }
}
