use email_core::error::{EmailError, Result};
use email_core::models::{Attachment, MessageHeader, Recipient};
use futures::TryStreamExt;
use log::debug;
use mailparse::{parse_mail, ParsedMail};
use uuid::Uuid;

use crate::connection::ImapSession;

pub struct BodyFetchResult {
    pub plain_text: Option<String>,
    pub html_text: Option<String>,
    pub attachments: Vec<Attachment>,
}

pub async fn fetch_and_parse_body(
    session: &mut ImapSession,
    header: &MessageHeader,
) -> Result<BodyFetchResult> {
    debug!("Fetching full body for message UID {}", header.uid);

    let query = format!("{}", header.uid);
    let mut fetch_stream = session
        .uid_fetch(&query, "BODY.PEEK[]")
        .await
        .map_err(|e| EmailError::Imap(format!("Failed to fetch body for UID {}: {}", header.uid, e)))?;

    let mut raw_bytes: Option<Vec<u8>> = None;

    while let Some(fetch) = fetch_stream
        .try_next()
        .await
        .map_err(|e| EmailError::Imap(format!("Stream error fetching body: {}", e)))?
    {
        if let Some(body) = fetch.body() {
            raw_bytes = Some(body.to_vec());
            break;
        }
    }

    let bytes = raw_bytes.ok_or_else(|| {
        EmailError::Imap(format!("No body bytes returned for UID {}", header.uid))
    })?;

    parse_mime_message(&bytes, &header.id)
}

pub fn parse_full_mime_and_enrich_header(
    raw_bytes: &[u8],
    header: &mut MessageHeader,
) -> Result<BodyFetchResult> {
    let parsed = parse_mail(raw_bytes)
        .map_err(|e| EmailError::MimeParse(format!("Failed to parse MIME: {}", e)))?;

    // Extract and decode headers from MIME
    for h in &parsed.headers {
        let key = h.get_key().to_lowercase();
        let val = h.get_value();

        match key.as_str() {
            "subject" => {
                let trimmed = val.trim();
                if !trimmed.is_empty() {
                    header.subject = trimmed.to_string();
                }
            }
            "from" => {
                let addrs = extract_single_addrs(&val);
                if let Some((name_opt, addr_str)) = addrs.into_iter().next() {
                    if !addr_str.is_empty() {
                        header.from_address = addr_str;
                    }
                    if let Some(name) = name_opt {
                        if !name.trim().is_empty() {
                            header.from_name = Some(name.trim().to_string());
                        }
                    }
                }
                if header.from_address.is_empty() {
                    header.from_address = val.trim().to_string();
                }
            }
            "to" => {
                if header.to_recipients.is_empty() {
                    for (name, email) in extract_single_addrs(&val) {
                        if !email.is_empty() {
                            header.to_recipients.push(Recipient { name, email });
                        }
                    }
                }
            }
            "cc" => {
                if header.cc_recipients.is_empty() {
                    for (name, email) in extract_single_addrs(&val) {
                        if !email.is_empty() {
                            header.cc_recipients.push(Recipient { name, email });
                        }
                    }
                }
            }

            "date" => {
                if let Ok(epoch) = mailparse::dateparse(&val) {
                    if epoch > 0 {
                        header.date_epoch = epoch;
                    }
                }
            }

            "message-id" => {
                if header.message_id.is_none() {
                    header.message_id = Some(val.trim().to_string());
                }
            }
            "in-reply-to" => {
                if header.in_reply_to.is_none() {
                    header.in_reply_to = Some(val.trim().to_string());
                }
            }
            _ => {}
        }
    }

    let mut plain_parts = Vec::new();
    let mut html_parts = Vec::new();
    let mut attachments = Vec::new();

    walk_mime_parts(&parsed, &header.id, &mut plain_parts, &mut html_parts, &mut attachments)?;

    let plain_text = if !plain_parts.is_empty() {
        Some(plain_parts.join("\n\n"))
    } else {
        None
    };

    let html_text = if !html_parts.is_empty() {
        Some(html_parts.join("\n"))
    } else {
        None
    };

    // Populate snippet from plain text
    if let Some(ref plain) = plain_text {
        let snippet: String = plain
            .chars()
            .filter(|c| !c.is_control())
            .take(120)
            .collect();
        if !snippet.trim().is_empty() {
            header.snippet = snippet;
        }
    } else if header.snippet.is_empty() {
        header.snippet = header.subject.clone();
    }

    header.body_fetched = true;

    Ok(BodyFetchResult {
        plain_text,
        html_text,
        attachments,
    })
}

pub fn parse_mime_message(raw_bytes: &[u8], message_id: &str) -> Result<BodyFetchResult> {
    let parsed = parse_mail(raw_bytes)
        .map_err(|e| EmailError::MimeParse(format!("Failed to parse MIME: {}", e)))?;

    let mut plain_parts = Vec::new();
    let mut html_parts = Vec::new();
    let mut attachments = Vec::new();

    walk_mime_parts(&parsed, message_id, &mut plain_parts, &mut html_parts, &mut attachments)?;

    let plain_text = if !plain_parts.is_empty() {
        Some(plain_parts.join("\n\n"))
    } else {
        None
    };

    let html_text = if !html_parts.is_empty() {
        Some(html_parts.join("\n"))
    } else {
        None
    };

    Ok(BodyFetchResult {
        plain_text,
        html_text,
        attachments,
    })
}

fn walk_mime_parts(
    part: &ParsedMail,
    message_id: &str,
    plain_parts: &mut Vec<String>,
    html_parts: &mut Vec<String>,
    attachments: &mut Vec<Attachment>,
) -> Result<()> {
    let ctype = &part.ctype;
    let mimetype = ctype.mimetype.to_lowercase();
    let disposition = part.get_content_disposition();
    let is_attachment = disposition.disposition == mailparse::DispositionType::Attachment;

    if is_attachment || (part.subparts.is_empty() && !mimetype.starts_with("text/")) {
        let filename = disposition
            .params
            .get("filename")
            .cloned()
            .or_else(|| ctype.params.get("name").cloned())
            .unwrap_or_else(|| "attachment.bin".to_string());

        let size_bytes = part.get_body_raw().map(|b| b.len()).unwrap_or(0) as u64;
        let is_inline = disposition.disposition == mailparse::DispositionType::Inline;

        let att_id = Uuid::new_v4().to_string();
        let safe_name = filename.replace(|c: char| !c.is_alphanumeric() && c != '.' && c != '-' && c != '_', "_");
        let cache_file = get_attachment_cache_dir().join(format!("{}_{}", att_id, safe_name));
        let mut local_cache_path = None;

        if let Ok(raw_bytes) = part.get_body_raw() {
            if std::fs::write(&cache_file, &raw_bytes).is_ok() {
                local_cache_path = Some(cache_file.to_string_lossy().to_string());
            }
        }

        attachments.push(Attachment {
            id: att_id,
            message_id: message_id.to_string(),
            filename,
            mime_type: mimetype,
            size_bytes,
            content_id: part.headers.iter().find(|h| h.get_key().eq_ignore_ascii_case("content-id")).map(|h| h.get_value().trim_matches(|c| c == '<' || c == '>').to_string()),
            is_inline,
            local_cache_path,
        });
    } else if mimetype == "text/plain" && !is_attachment {

        if let Ok(body) = part.get_body() {
            let trimmed = body.trim().to_string();
            if !trimmed.is_empty() {
                plain_parts.push(trimmed);
            }
        }
    } else if mimetype == "text/html" && !is_attachment {
        if let Ok(body) = part.get_body() {
            let trimmed = body.trim().to_string();
            if !trimmed.is_empty() {
                html_parts.push(trimmed);
            }
        }
    }

    for subpart in &part.subparts {
        walk_mime_parts(subpart, message_id, plain_parts, html_parts, attachments)?;
    }

    Ok(())
}

fn get_attachment_cache_dir() -> std::path::PathBuf {
    let mut p = if let Ok(home) = std::env::var("HOME") {
        let mut p = std::path::PathBuf::from(home);
        p.push(".local");
        p.push("share");
        p.push("at-mail-rs");
        p
    } else {
        std::path::PathBuf::from(".")
    };
    p.push("attachments");
    let _ = std::fs::create_dir_all(&p);
    p
}


fn extract_single_addrs(val: &str) -> Vec<(Option<String>, String)> {

    let mut results = Vec::new();
    if let Ok(addr_list) = mailparse::addrparse(val) {
        for mail_addr in addr_list.iter() {
            match mail_addr {
                mailparse::MailAddr::Single(info) => {
                    results.push((info.display_name.clone(), info.addr.clone()));
                }
                mailparse::MailAddr::Group(group) => {
                    for info in &group.addrs {
                        results.push((info.display_name.clone(), info.addr.clone()));
                    }
                }
            }
        }
    }
    results
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multipart_alternative_mime_parse() {
        let raw_email = b"From: John Doe <sender@example.com>\r\nTo: recipient@example.com\r\nSubject: Test Email\r\nMIME-Version: 1.0\r\nContent-Type: multipart/alternative; boundary=\"boundary123\"\r\n\r\n--boundary123\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nHello Plain Text\r\n--boundary123\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p>Hello <b>HTML</b></p>\r\n--boundary123--";

        let mut header = MessageHeader {
            id: "msg-001".to_string(),
            account_id: "acc-1".to_string(),
            folder_id: "fol-1".to_string(),
            uid: 1,
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
            size_bytes: 0,
        };

        let res = parse_full_mime_and_enrich_header(raw_email, &mut header).unwrap();
        assert_eq!(header.subject, "Test Email");
        assert_eq!(header.from_address, "sender@example.com");
        assert_eq!(header.from_name.as_deref(), Some("John Doe"));
        assert_eq!(res.plain_text.as_deref(), Some("Hello Plain Text"));
        assert_eq!(res.html_text.as_deref(), Some("<p>Hello <b>HTML</b></p>"));
        assert_eq!(res.attachments.len(), 0);
    }

    #[test]
    fn test_attachment_extraction() {
        let raw_email = b"From: sender@example.com\r\nTo: recipient@example.com\r\nSubject: With Attachment\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"boundaryABC\"\r\n\r\n--boundaryABC\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nPlease find report attached.\r\n--boundaryABC\r\nContent-Type: application/pdf; name=\"report.pdf\"\r\nContent-Disposition: attachment; filename=\"report.pdf\"\r\nContent-Transfer-Encoding: base64\r\n\r\nSGVsbG8gUERG\r\n--boundaryABC--";

        let res = parse_mime_message(raw_email, "msg-002").unwrap();
        assert_eq!(res.plain_text.as_deref(), Some("Please find report attached."));
        assert_eq!(res.attachments.len(), 1);
        assert_eq!(res.attachments[0].filename, "report.pdf");
        assert_eq!(res.attachments[0].mime_type, "application/pdf");
    }
}
