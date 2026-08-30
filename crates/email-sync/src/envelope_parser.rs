use async_imap::types::{Fetch, Flag};
use chrono::Utc;
use email_core::models::{MessageHeader, Recipient};
use uuid::Uuid;

pub fn parse_fetch_envelope(
    fetch: &Fetch,
    account_id: &str,
    folder_id: &str,
) -> Option<MessageHeader> {
    let uid = fetch.uid?;
    let flags: Vec<Flag> = fetch.flags().collect();
    let is_read = flags.iter().any(|f| matches!(f, Flag::Seen));
    let is_flagged = flags.iter().any(|f| matches!(f, Flag::Flagged));
    let is_draft = flags.iter().any(|f| matches!(f, Flag::Draft));
    let is_deleted = flags.iter().any(|f| matches!(f, Flag::Deleted));

    let size_bytes = fetch.size.unwrap_or(0) as u64;

    let date_epoch = if let Some(internal_date) = fetch.internal_date() {
        internal_date.timestamp()
    } else {
        Utc::now().timestamp()
    };

    let mut subject = String::new();
    let mut from_name = None;
    let mut from_address = String::new();
    let mut message_id = None;
    let mut in_reply_to = None;
    let mut to_recipients = Vec::new();
    let mut cc_recipients = Vec::new();

    if let Some(ref env) = fetch.envelope() {
        if let Some(ref subj_bytes) = env.subject {
            subject = String::from_utf8_lossy(subj_bytes).to_string();
        }

        if let Some(ref msg_id_bytes) = env.message_id {
            message_id = Some(String::from_utf8_lossy(msg_id_bytes).to_string());
        }

        if let Some(ref reply_bytes) = env.in_reply_to {
            in_reply_to = Some(String::from_utf8_lossy(reply_bytes).to_string());
        }

        if let Some(ref from_list) = env.from {
            if let Some(first) = from_list.first() {
                if let Some(ref name_bytes) = first.name {
                    from_name = Some(String::from_utf8_lossy(name_bytes).to_string());
                }
                let host = first
                    .host
                    .as_ref()
                    .map(|h| String::from_utf8_lossy(h).to_string())
                    .unwrap_or_default();
                let mailbox = first
                    .mailbox
                    .as_ref()
                    .map(|m| String::from_utf8_lossy(m).to_string())
                    .unwrap_or_default();
                if !host.is_empty() && !mailbox.is_empty() {
                    from_address = format!("{}@{}", mailbox, host);
                } else if !mailbox.is_empty() {
                    from_address = mailbox;
                }
            }
        }

        if let Some(ref to_list) = env.to {
            for addr in to_list {
                let name = addr.name.as_ref().map(|n| String::from_utf8_lossy(n).to_string());
                let host = addr.host.as_ref().map(|h| String::from_utf8_lossy(h).to_string()).unwrap_or_default();
                let mailbox = addr.mailbox.as_ref().map(|m| String::from_utf8_lossy(m).to_string()).unwrap_or_default();
                let email = if !host.is_empty() && !mailbox.is_empty() {
                    format!("{}@{}", mailbox, host)
                } else {
                    mailbox
                };
                if !email.is_empty() {
                    to_recipients.push(Recipient { name, email });
                }
            }
        }

        if let Some(ref cc_list) = env.cc {
            for addr in cc_list {
                let name = addr.name.as_ref().map(|n| String::from_utf8_lossy(n).to_string());
                let host = addr.host.as_ref().map(|h| String::from_utf8_lossy(h).to_string()).unwrap_or_default();
                let mailbox = addr.mailbox.as_ref().map(|m| String::from_utf8_lossy(m).to_string()).unwrap_or_default();
                let email = if !host.is_empty() && !mailbox.is_empty() {
                    format!("{}@{}", mailbox, host)
                } else {
                    mailbox
                };
                if !email.is_empty() {
                    cc_recipients.push(Recipient { name, email });
                }
            }
        }
    }

    let snippet = if subject.len() > 100 {
        format!("{}...", &subject[..97])
    } else {
        subject.clone()
    };

    Some(MessageHeader {
        id: Uuid::new_v4().to_string(),
        account_id: account_id.to_string(),
        folder_id: folder_id.to_string(),
        uid,
        message_id,
        in_reply_to,
        subject,
        from_name,
        from_address,
        to_recipients,
        cc_recipients,
        date_epoch,
        snippet,
        is_read,
        is_flagged,
        is_draft,
        is_deleted,
        body_fetched: false,
        size_bytes,
    })
}
