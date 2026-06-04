//! Email gateway transport (§3.16).
//!
//! Outbound: SMTP via `lettre`. Inbound: IMAP polling of UNSEEN messages from
//! allowed senders, surfaced as `GatewayMessage`s. Both are behind the `email`
//! cargo feature. No fake success — misconfiguration returns real errors.

use async_trait::async_trait;
use tokio::sync::mpsc;

use super::{GatewayMessage, GatewayResponse, GatewayTransport};

#[cfg(feature = "email")]
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
    message::{Mailbox, Message, header::ContentType},
    transport::smtp::authentication::Credentials,
};

/// SMTP (outbound) + optional IMAP (inbound) email transport.
pub struct EmailTransport {
    pub from: String,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub username: String,
    pub password: String,
    pub allowed_to: Vec<String>,
    /// IMAP server for inbound polling (None = outbound only).
    pub imap_host: Option<String>,
    pub imap_port: u16,
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
            imap_host: None,
            imap_port: 993,
        }
    }

    /// Enable inbound IMAP polling.
    pub fn with_imap(mut self, host: String, port: u16) -> Self {
        self.imap_host = Some(host);
        self.imap_port = port;
        self
    }
}

#[cfg(feature = "email")]
fn parse_email(raw: &str) -> (String, String, String) {
    // Minimal RFC822 split: headers vs body, pull From/Subject.
    let split = raw.find("\r\n\r\n").or_else(|| raw.find("\n\n"));
    let (headers, body) = match split {
        Some(i) => (&raw[..i], raw[i..].trim_start()),
        None => (raw, ""),
    };
    let mut from = String::new();
    let mut subject = String::new();
    for line in headers.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("from:") {
            from = line[5..].trim().to_string();
        } else if lower.starts_with("subject:") {
            subject = line[8..].trim().to_string();
        }
    }
    // Extract bare address from "Name <addr>"
    if let (Some(a), Some(b)) = (from.find('<'), from.find('>')) {
        if a < b {
            from = from[a + 1..b].to_string();
        }
    }
    (from, subject, body.to_string())
}

#[async_trait]
impl GatewayTransport for EmailTransport {
    fn name(&self) -> &str {
        "email"
    }

    #[cfg(feature = "email")]
    async fn start(&self, tx: mpsc::UnboundedSender<GatewayMessage>) -> anyhow::Result<()> {
        let Some(imap_host) = self.imap_host.clone() else {
            tracing::info!(
                "Email gateway: outbound only via SMTP {}:{}",
                self.smtp_host,
                self.smtp_port
            );
            return Ok(());
        };

        // IMAP polling runs on a dedicated blocking thread (imap crate is sync).
        let port = self.imap_port;
        let user = self.username.clone();
        let pass = self.password.clone();
        let allowed = self.allowed_to.clone();
        tracing::info!("Email gateway: IMAP inbound polling {}:{}", imap_host, port);

        std::thread::spawn(move || {
            loop {
                if let Err(e) = poll_imap_once(&imap_host, port, &user, &pass, &allowed, &tx) {
                    tracing::warn!("IMAP poll error: {}", e);
                }
                std::thread::sleep(std::time::Duration::from_secs(30));
            }
        });
        Ok(())
    }

    #[cfg(not(feature = "email"))]
    async fn start(&self, _tx: mpsc::UnboundedSender<GatewayMessage>) -> anyhow::Result<()> {
        anyhow::bail!("email feature not enabled — rebuild with `cargo build --features email`")
    }

    #[cfg(feature = "email")]
    async fn send(&self, response: GatewayResponse) -> anyhow::Result<()> {
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

/// One IMAP poll: fetch UNSEEN from allowed senders, emit GatewayMessages, mark seen.
#[cfg(feature = "email")]
fn poll_imap_once(
    host: &str,
    port: u16,
    user: &str,
    pass: &str,
    allowed: &[String],
    tx: &mpsc::UnboundedSender<GatewayMessage>,
) -> anyhow::Result<()> {
    let tls = native_tls::TlsConnector::builder().build()?;
    let client = imap::connect((host, port), host, &tls)?;
    let mut session = client
        .login(user, pass)
        .map_err(|(e, _)| anyhow::anyhow!("IMAP login failed: {}", e))?;
    session.select("INBOX")?;

    let unseen = session.search("UNSEEN")?;
    for uid in unseen {
        let messages = session.fetch(uid.to_string(), "RFC822")?;
        for msg in messages.iter() {
            if let Some(body) = msg.body() {
                let raw = String::from_utf8_lossy(body);
                let (from, subject, text) = parse_email(&raw);
                let allowed_ok =
                    allowed.is_empty() || allowed.iter().any(|a| from.contains(a.as_str()));
                if !allowed_ok {
                    continue;
                }
                let combined = if subject.is_empty() {
                    text
                } else {
                    format!("{}\n\n{}", subject, text)
                };
                let _ = tx.send(GatewayMessage {
                    surface: "email".into(),
                    user_id: from.clone(),
                    chat_id: from,
                    text: combined.trim().to_string(),
                    message_id: Some(uid.to_string()),
                });
            }
        }
        // Mark as seen so we don't reprocess.
        let _ = session.store(uid.to_string(), "+FLAGS (\\Seen)");
    }

    let _ = session.logout();
    Ok(())
}
