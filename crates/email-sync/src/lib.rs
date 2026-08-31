pub mod body_fetch;
pub mod connection;
pub mod date_window;
pub mod envelope_parser;
pub mod folder_sync;
pub mod idle;
pub mod worker;

pub use body_fetch::*;
pub use connection::*;
pub use date_window::*;
pub use envelope_parser::*;
pub use folder_sync::*;
pub use idle::*;
pub use worker::*;

#[cfg(test)]
mod tests {
    use super::*;
    use email_core::models::*;
    use email_storage::Storage;

    #[test]
    fn test_full_email_sync_and_offline_storage() {
        let storage = Storage::new_in_memory().unwrap();

        let account = Account::new(
            "Kunal".to_string(),
            "kunal@abhashtech.com".to_string(),
            "imap.example.com".to_string(),
            993,
            SecurityType::Tls,
            "smtp.example.com".to_string(),
            465,
            SecurityType::Tls,
            AuthType::Password,
            SyncWindow::Days30,
        );
        storage.save_account(&account).unwrap();

        let folder = Folder::new(
            account.id.clone(),
            "INBOX".to_string(),
            "Inbox".to_string(),
            "/".to_string(),
            vec![],
            true,
        );
        storage.save_folders(&[folder.clone()]).unwrap();

        // Simulate incoming RFC822 email payload
        let raw_email = b"From: ICICI Bank <services@custcomm.icici.bank.in>\r\n\
To: kunal@abhashtech.com\r\n\
Subject: =?UTF-8?B?U2VjdXJpbmcgeW91ciBzZW5pb3IgeWVhcnMh?=\r\n\
Date: Fri, 28 Aug 2026 15:33:00 +0530\r\n\
Message-ID: <msg-icici-123@icici.bank.in>\r\n\
MIME-Version: 1.0\r\n\
Content-Type: multipart/alternative; boundary=\"BOUNDARY\"\r\n\
\r\n\
--BOUNDARY\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
\r\n\
The Independent Generation. This August, The Orange Book explores how senior citizens are living life.\r\n\
--BOUNDARY\r\n\
Content-Type: text/html; charset=utf-8\r\n\
\r\n\
<h2>The Independent Generation</h2><p>This August, <b>The Orange Book</b> explores how senior citizens are living life on their own terms.</p>\r\n\
--BOUNDARY--";

        let mut header = MessageHeader {
            id: "msg-icici-001".to_string(),
            account_id: account.id.clone(),
            folder_id: folder.id.clone(),
            uid: 501,
            message_id: None,
            in_reply_to: None,
            subject: String::new(),
            from_name: None,
            from_address: String::new(),
            to_recipients: Vec::new(),
            cc_recipients: Vec::new(),
            date_epoch: 0,
            snippet: String::new(),
            is_read: false,
            is_flagged: false,
            is_draft: false,
            is_deleted: false,
            body_fetched: false,
            size_bytes: raw_email.len() as u64,
        };

        let parsed = parse_full_mime_and_enrich_header(raw_email, &mut header).unwrap();

        assert_eq!(header.subject, "Securing your senior years!");
        assert_eq!(header.from_address, "services@custcomm.icici.bank.in");
        assert_eq!(header.from_name.as_deref(), Some("ICICI Bank"));
        assert_eq!(header.sender_display(), "ICICI Bank");
        assert!(header.body_fetched);

        // Store full message in database as done during sync
        storage.save_full_messages(&[(
            header.clone(),
            parsed.plain_text,
            parsed.html_text,
            parsed.attachments,
        )]).unwrap();

        // Verify offline retrieval
        let cached = storage.get_folder_cached_uids(&folder.id).unwrap();
        assert_eq!(cached.get(&501), Some(&true));

        let detail = storage.get_message_detail("msg-icici-001").unwrap().expect("Detail not found");
        assert_eq!(detail.header.subject, "Securing your senior years!");
        assert_eq!(detail.header.sender_display(), "ICICI Bank");
        assert!(detail.body_html.unwrap().contains("The Orange Book"));
    }
}
