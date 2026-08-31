use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityType {
    Tls,
    StartTls,
    Plain,
}

impl Default for SecurityType {
    fn default() -> Self {
        SecurityType::Tls
    }
}

impl SecurityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SecurityType::Tls => "Tls",
            SecurityType::StartTls => "StartTls",
            SecurityType::Plain => "Plain",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "StartTls" => SecurityType::StartTls,
            "Plain" => SecurityType::Plain,
            _ => SecurityType::Tls,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthType {
    Password,
    OAuth2,
}

impl Default for AuthType {
    fn default() -> Self {
        AuthType::Password
    }
}

impl AuthType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuthType::Password => "Password",
            AuthType::OAuth2 => "OAuth2",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "OAuth2" => AuthType::OAuth2,
            _ => AuthType::Password,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncWindow {
    Days7,
    Days14,
    Days30,
    Days45,
    Days60,
    Days90,
    Days365,
    Custom(i64),
    All,
}

impl Default for SyncWindow {
    fn default() -> Self {
        SyncWindow::Days30
    }
}

impl SyncWindow {
    pub fn days(&self) -> Option<i64> {
        match self {
            SyncWindow::Days7 => Some(7),
            SyncWindow::Days14 => Some(14),
            SyncWindow::Days30 => Some(30),
            SyncWindow::Days45 => Some(45),
            SyncWindow::Days60 => Some(60),
            SyncWindow::Days90 => Some(90),
            SyncWindow::Days365 => Some(365),
            SyncWindow::Custom(d) => Some((*d).max(1)),
            SyncWindow::All => None,
        }
    }

    pub fn from_days(days: i64) -> Self {
        match days {
            0 => SyncWindow::All,
            7 => SyncWindow::Days7,
            14 => SyncWindow::Days14,
            30 => SyncWindow::Days30,
            45 => SyncWindow::Days45,
            60 => SyncWindow::Days60,
            90 => SyncWindow::Days90,
            365 => SyncWindow::Days365,
            d if d > 0 => SyncWindow::Custom(d),
            _ => SyncWindow::All,
        }
    }

    pub fn label(&self) -> String {
        match self {
            SyncWindow::Days7 => "Last 7 Days (1 Week)".to_string(),
            SyncWindow::Days14 => "Last 14 Days (2 Weeks)".to_string(),
            SyncWindow::Days30 => "Last 30 Days (1 Month)".to_string(),
            SyncWindow::Days45 => "Last 45 Days (1.5 Months)".to_string(),
            SyncWindow::Days60 => "Last 60 Days (2 Months)".to_string(),
            SyncWindow::Days90 => "Last 90 Days (3 Months / Quarter)".to_string(),
            SyncWindow::Days365 => "Last 365 Days (1 Year)".to_string(),
            SyncWindow::Custom(d) => format!("Custom ({} Days)", d),
            SyncWindow::All => "All History (Everything)".to_string(),
        }
    }

    pub fn calculate_since_date(&self) -> Option<String> {
        let days = self.days()?;
        let cutoff = Utc::now() - chrono::Duration::days(days);
        // Format as RFC 3501 IMAP date: "01-Jan-2024"
        Some(cutoff.format("%d-%b-%Y").to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub email: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_security: SecurityType,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_security: SecurityType,
    pub auth_type: AuthType,
    pub credential_key: String, // Lookup key in OS native keyring (zero plaintext in DB)
    pub sync_days_window: SyncWindow,
    pub is_enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Account {
    pub fn new(
        name: String,
        email: String,
        imap_host: String,
        imap_port: u16,
        imap_security: SecurityType,
        smtp_host: String,
        smtp_port: u16,
        smtp_security: SecurityType,
        auth_type: AuthType,
        sync_days_window: SyncWindow,
    ) -> Self {
        let id = Uuid::new_v4().to_string();
        let credential_key = format!("mail_acc_{}_secret", id);
        let now = Utc::now().timestamp();
        Self {
            id,
            name,
            email,
            imap_host,
            imap_port,
            imap_security,
            smtp_host,
            smtp_port,
            smtp_security,
            auth_type,
            credential_key,
            sync_days_window,
            is_enabled: true,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub account_id: String,
    pub remote_name: String,
    pub display_name: String,
    pub delimiter: String,
    pub attributes: Vec<String>,
    pub is_synced: bool,
    pub last_synced_uid: u32,
    pub uid_validity: u32,
    pub total_messages: u32,
    pub unread_messages: u32,
}

impl Folder {
    pub fn new(
        account_id: String,
        remote_name: String,
        display_name: String,
        delimiter: String,
        attributes: Vec<String>,
        is_synced: bool,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            account_id,
            remote_name,
            display_name,
            delimiter,
            attributes,
            is_synced,
            last_synced_uid: 0,
            uid_validity: 0,
            total_messages: 0,
            unread_messages: 0,
        }
    }

    pub fn is_inbox(&self) -> bool {
        self.remote_name.eq_ignore_ascii_case("INBOX")
            || self.attributes.iter().any(|a| a.eq_ignore_ascii_case("\\Inbox"))
    }

    pub fn is_sent(&self) -> bool {
        self.remote_name.to_lowercase().contains("sent")
            || self.attributes.iter().any(|a| a.eq_ignore_ascii_case("\\Sent"))
    }

    pub fn is_drafts(&self) -> bool {
        self.remote_name.to_lowercase().contains("draft")
            || self.attributes.iter().any(|a| a.eq_ignore_ascii_case("\\Drafts"))
    }

    pub fn is_trash(&self) -> bool {
        self.remote_name.to_lowercase().contains("trash")
            || self.remote_name.to_lowercase().contains("bin")
            || self.attributes.iter().any(|a| a.eq_ignore_ascii_case("\\Trash"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Recipient {
    pub name: Option<String>,
    pub email: String,
}

impl Recipient {
    pub fn new(name: Option<String>, email: String) -> Self {
        Self { name, email }
    }

    pub fn display(&self) -> String {
        if let Some(ref name) = self.name {
            if !name.trim().is_empty() {
                return format!("{} <{}>", name, self.email);
            }
        }
        self.email.clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHeader {
    pub id: String,
    pub account_id: String,
    pub folder_id: String,
    pub uid: u32,
    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub subject: String,
    pub from_name: Option<String>,
    pub from_address: String,
    pub to_recipients: Vec<Recipient>,
    pub cc_recipients: Vec<Recipient>,
    pub date_epoch: i64,
    pub snippet: String,
    pub is_read: bool,
    pub is_flagged: bool,
    pub is_draft: bool,
    pub is_deleted: bool,
    pub body_fetched: bool,
    pub size_bytes: u64,
}

impl MessageHeader {
    pub fn formatted_date(&self) -> String {
        if self.date_epoch == 0 {
            return "".to_string();
        }
        let dt = DateTime::from_timestamp(self.date_epoch, 0).unwrap_or_default();
        let now = Utc::now();
        if dt.date_naive() == now.date_naive() {
            dt.format("%H:%M").to_string()
        } else if dt.year() == now.year() {
            dt.format("%b %d").to_string()
        } else {
            dt.format("%b %d, %Y").to_string()
        }
    }

    pub fn formatted_full_date(&self) -> String {
        if self.date_epoch == 0 {
            return "".to_string();
        }
        let dt = DateTime::from_timestamp(self.date_epoch, 0).unwrap_or_default();
        dt.format("%a, %b %e, %Y at %I:%M %p").to_string()
    }

    pub fn sender_display(&self) -> &str {

        if let Some(ref name) = self.from_name {
            if !name.trim().is_empty() {
                return name.as_str();
            }
        }
        if !self.from_address.trim().is_empty() {
            self.from_address.as_str()
        } else {
            "Unknown Sender"
        }
    }
}


use chrono::Datelike;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: String,
    pub message_id: String,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub content_id: Option<String>,
    pub is_inline: bool,
    pub local_cache_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDetail {
    pub header: MessageHeader,
    pub body_plain: Option<String>,
    pub body_html: Option<String>,
    pub attachments: Vec<Attachment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub id: String,
    pub name: String,
    pub subject_template: String,
    pub body_template: String,
    pub shortcut: Option<String>,
    pub created_at: i64,
}

impl Template {
    pub fn new(name: String, subject_template: String, body_template: String, shortcut: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            subject_template,
            body_template,
            shortcut,
            created_at: Utc::now().timestamp(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signature {
    pub id: String,
    pub account_id: Option<String>, // None = global default
    pub name: String,
    pub content_html: String,
    pub is_default: bool,
    pub created_at: i64,
}

impl Signature {
    pub fn new(account_id: Option<String>, name: String, content_html: String, is_default: bool) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            account_id,
            name,
            content_html,
            is_default,
            created_at: Utc::now().timestamp(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_window_all_options() {
        assert_eq!(SyncWindow::from_days(7), SyncWindow::Days7);
        assert_eq!(SyncWindow::from_days(14), SyncWindow::Days14);
        assert_eq!(SyncWindow::from_days(30), SyncWindow::Days30);
        assert_eq!(SyncWindow::from_days(45), SyncWindow::Days45);
        assert_eq!(SyncWindow::from_days(60), SyncWindow::Days60);
        assert_eq!(SyncWindow::from_days(90), SyncWindow::Days90);
        assert_eq!(SyncWindow::from_days(365), SyncWindow::Days365);
        assert_eq!(SyncWindow::from_days(120), SyncWindow::Custom(120));
        assert_eq!(SyncWindow::from_days(0), SyncWindow::All);

        assert_eq!(SyncWindow::Days7.days(), Some(7));
        assert_eq!(SyncWindow::Days14.days(), Some(14));
        assert_eq!(SyncWindow::Days30.days(), Some(30));
        assert_eq!(SyncWindow::Days45.days(), Some(45));
        assert_eq!(SyncWindow::Days60.days(), Some(60));
        assert_eq!(SyncWindow::Days90.days(), Some(90));
        assert_eq!(SyncWindow::Days365.days(), Some(365));
        assert_eq!(SyncWindow::Custom(50).days(), Some(50));
        assert_eq!(SyncWindow::All.days(), None);

        assert!(SyncWindow::Days45.calculate_since_date().is_some());
        assert!(SyncWindow::Custom(60).calculate_since_date().is_some());
        assert!(SyncWindow::All.calculate_since_date().is_none());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutgoingDraft {
    pub account_id: String,
    pub to: Vec<Recipient>,
    pub cc: Vec<Recipient>,
    pub bcc: Vec<Recipient>,
    pub subject: String,
    pub body_plain: String,
    pub body_html: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Option<String>,
}
