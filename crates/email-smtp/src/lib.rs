use email_core::error::{EmailError, Result};
use email_core::models::{Account, OutgoingDraft, SecurityType};
use lettre::message::header::ContentType;
use lettre::message::{Mailbox, Message, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::Tls;
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
use log::info;
use std::time::Duration;

pub struct SmtpClient;

impl SmtpClient {
    pub async fn test_connection(account: &Account, password: &str) -> Result<()> {
        let creds = Credentials::new(account.email.clone(), password.to_string());

        let transport_builder = match account.smtp_security {
            SecurityType::Tls => {
                AsyncSmtpTransport::<Tokio1Executor>::relay(&account.smtp_host)
                    .map_err(|e| EmailError::Smtp(format!("Invalid SMTP host: {}", e)))?
                    .port(account.smtp_port)
            }
            SecurityType::StartTls => {
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&account.smtp_host)
                    .map_err(|e| EmailError::Smtp(format!("Invalid SMTP host: {}", e)))?
                    .port(account.smtp_port)
            }
            SecurityType::Plain => {
                AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&account.smtp_host)
                    .port(account.smtp_port)
                    .tls(Tls::None)
            }
        };

        let transport: AsyncSmtpTransport<Tokio1Executor> = transport_builder
            .credentials(creds)
            .timeout(Some(Duration::from_secs(10)))
            .build();

        transport
            .test_connection()
            .await
            .map_err(|e| EmailError::Smtp(format!("SMTP connection test failed: {}", e)))?;

        info!("SMTP connection test succeeded for {}", account.email);
        Ok(())
    }

    pub async fn send_email(
        account: &Account,
        password: &str,
        draft: &OutgoingDraft,
    ) -> Result<()> {
        let from_mailbox: Mailbox = format!("{} <{}>", account.name, account.email)
            .parse()
            .map_err(|e| EmailError::Smtp(format!("Invalid from address: {}", e)))?;

        let mut builder = Message::builder()
            .from(from_mailbox)
            .subject(&draft.subject);

        for recipient in &draft.to {
            let mbox: Mailbox = recipient
                .display()
                .parse()
                .map_err(|e| EmailError::Smtp(format!("Invalid recipient {}: {}", recipient.email, e)))?;
            builder = builder.to(mbox);
        }

        for recipient in &draft.cc {
            let mbox: Mailbox = recipient
                .display()
                .parse()
                .map_err(|e| EmailError::Smtp(format!("Invalid cc recipient {}: {}", recipient.email, e)))?;
            builder = builder.cc(mbox);
        }

        for recipient in &draft.bcc {
            let mbox: Mailbox = recipient
                .display()
                .parse()
                .map_err(|e| EmailError::Smtp(format!("Invalid bcc recipient {}: {}", recipient.email, e)))?;
            builder = builder.bcc(mbox);
        }

        if let Some(ref reply_to) = draft.in_reply_to {
            builder = builder.in_reply_to(reply_to.clone());
        }

        if let Some(ref refs) = draft.references {
            builder = builder.references(refs.clone());
        }

        let email = if let Some(ref html) = draft.body_html {
            // Send alternative multipart: plain text + HTML
            let plain_part = SinglePart::builder()
                .header(ContentType::TEXT_PLAIN)
                .body(draft.body_plain.clone());
            let html_part = SinglePart::builder()
                .header(ContentType::TEXT_HTML)
                .body(html.clone());

            let multipart = MultiPart::alternative()
                .singlepart(plain_part)
                .singlepart(html_part);

            builder
                .multipart(multipart)
                .map_err(|e| EmailError::Smtp(format!("Failed to build multipart email: {}", e)))?
        } else {
            builder
                .header(ContentType::TEXT_PLAIN)
                .body(draft.body_plain.clone())
                .map_err(|e| EmailError::Smtp(format!("Failed to build plain email: {}", e)))?
        };

        let creds = Credentials::new(account.email.clone(), password.to_string());

        let transport_builder = match account.smtp_security {
            SecurityType::Tls => {
                AsyncSmtpTransport::<Tokio1Executor>::relay(&account.smtp_host)
                    .map_err(|e| EmailError::Smtp(format!("Invalid SMTP host: {}", e)))?
                    .port(account.smtp_port)
            }
            SecurityType::StartTls => {
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&account.smtp_host)
                    .map_err(|e| EmailError::Smtp(format!("Invalid SMTP host: {}", e)))?
                    .port(account.smtp_port)
            }
            SecurityType::Plain => {
                AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&account.smtp_host)
                    .port(account.smtp_port)
                    .tls(Tls::None)
            }
        };

        let transport: AsyncSmtpTransport<Tokio1Executor> = transport_builder
            .credentials(creds)
            .timeout(Some(Duration::from_secs(15)))
            .build();

        transport
            .send(email)
            .await
            .map_err(|e| EmailError::Smtp(format!("Failed to send email: {}", e)))?;

        info!("Successfully sent email: '{}'", draft.subject);
        Ok(())
    }
}
