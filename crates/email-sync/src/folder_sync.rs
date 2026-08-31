use crate::body_fetch::parse_full_mime_and_enrich_header;
use crate::connection::ImapSession;
use crate::date_window::DateWindowQuery;
use crate::envelope_parser::parse_fetch_envelope;

use email_core::error::{EmailError, Result};
use email_core::models::{Account, Folder};
use email_storage::Storage;
use futures::TryStreamExt;
use log::{debug, info};
use std::collections::HashSet;

pub async fn discover_remote_folders(
    session: &mut ImapSession,
    account: &Account,
) -> Result<Vec<Folder>> {
    info!("Discovering folders for account {}", account.email);

    let mut list_stream = session
        .list(Some(""), Some("*"))
        .await
        .map_err(|e| EmailError::Imap(format!("IMAP LIST error: {}", e)))?;

    let mut folders = Vec::new();

    while let Some(item) = list_stream
        .try_next()
        .await
        .map_err(|e| EmailError::Imap(format!("Stream error listing folders: {}", e)))?
    {
        let remote_name = item.name().to_string();
        let delimiter = item.delimiter().unwrap_or("/").to_string();
        let attributes: Vec<String> = item
            .attributes()
            .iter()
            .map(|a| format!("{:?}", a))
            .collect();

        // Infer display name from path
        let display_name = if let Some(last) = remote_name.rsplit(&delimiter).next() {
            if last.is_empty() {
                remote_name.clone()
            } else {
                last.to_string()
            }
        } else {
            remote_name.clone()
        };

        let is_inbox = remote_name.eq_ignore_ascii_case("INBOX");
        let is_synced_default = is_inbox
            || display_name.eq_ignore_ascii_case("Sent")
            || display_name.eq_ignore_ascii_case("Drafts");

        folders.push(Folder::new(
            account.id.clone(),
            remote_name,
            display_name,
            delimiter,
            attributes,
            is_synced_default,
        ));
    }

    info!("Discovered {} folders for {}", folders.len(), account.email);
    Ok(folders)
}

pub async fn sync_single_folder(
    session: &mut ImapSession,
    account: &Account,
    folder: &mut Folder,
    storage: &Storage,
) -> Result<usize> {
    if !folder.is_synced {
        debug!("Skipping unsynced folder: {}", folder.remote_name);
        return Ok(0);
    }

    info!(
        "Syncing folder '{}' for account '{}' (Window: {:?})",
        folder.remote_name, account.email, account.sync_days_window
    );

    let mailbox = session
        .select(&folder.remote_name)
        .await
        .map_err(|e| EmailError::Imap(format!("Failed to select {}: {}", folder.remote_name, e)))?;

    let total_messages = mailbox.exists;
    let uid_validity = mailbox.uid_validity.unwrap_or(0);

    // Build date-window search query
    let search_query =
        DateWindowQuery::build_search_query(account.sync_days_window, 0);


    debug!("Running IMAP UID SEARCH with query: {}", search_query);

    let uid_set: HashSet<u32> = session
        .uid_search(&search_query)
        .await
        .map_err(|e| EmailError::Imap(format!("UID SEARCH failed for {}: {}", folder.remote_name, e)))?;

    if uid_set.is_empty() {
        debug!(
            "No new/updated messages in date window for {}",
            folder.remote_name
        );
        let _ = storage.update_folder_stats(
            &folder.id,
            folder.last_synced_uid,
            total_messages,
            0,
        );
        return Ok(0);
    }

    let mut sorted_uids: Vec<u32> = uid_set.into_iter().collect();
    sorted_uids.sort_unstable();

    let cached_map = storage.get_folder_cached_uids(&folder.id).unwrap_or_default();
    let mut unfetched_uids = Vec::new();
    let mut cached_uids = Vec::new();

    for uid in &sorted_uids {
        if cached_map.get(uid) == Some(&true) {
            cached_uids.push(*uid);
        } else {
            unfetched_uids.push(*uid);
        }
    }

    let mut max_uid = folder.last_synced_uid;
    let mut synced_new_or_updated = 0;

    // 1. Download full emails (Full MIME + Attachments + Headers) for new or un-cached messages
    for chunk in unfetched_uids.chunks(25) {
        let uid_range = if chunk.len() == 1 {
            format!("{}", chunk[0])
        } else {
            let uids_str: Vec<String> = chunk.iter().map(|u| u.to_string()).collect();
            uids_str.join(",")
        };

        let mut fetch_stream = session
            .uid_fetch(&uid_range, "(UID FLAGS INTERNALDATE RFC822.SIZE BODY.PEEK[])")
            .await
            .map_err(|e| EmailError::Imap(format!("UID FETCH failed for {}: {}", uid_range, e)))?;

        let mut batch_records = Vec::new();

        while let Some(fetch) = fetch_stream
            .try_next()
            .await
            .map_err(|e| EmailError::Imap(format!("Stream error in UID FETCH: {}", e)))?
        {
            if let Some(mut header) = parse_fetch_envelope(&fetch, &account.id, &folder.id) {
                if header.uid > max_uid {
                    max_uid = header.uid;
                }

                let mut plain_text = None;
                let mut html_text = None;
                let mut attachments = Vec::new();

                // If full body bytes were returned, parse and store body and attachments immediately
                if let Some(raw_body) = fetch.body() {
                    if let Ok(parsed) = parse_full_mime_and_enrich_header(raw_body, &mut header) {
                        plain_text = parsed.plain_text;
                        html_text = parsed.html_text;
                        attachments = parsed.attachments;
                    }
                }

                batch_records.push((header, plain_text, html_text, attachments));
            }
        }

        if !batch_records.is_empty() {
            synced_new_or_updated += batch_records.len();
            storage.save_full_messages(&batch_records)?;
        }
    }

    // 2. Fast flag sync (Read / Flagged status) for already-cached offline messages
    for chunk in cached_uids.chunks(100) {
        let uid_range = if chunk.len() == 1 {
            format!("{}", chunk[0])
        } else {
            let uids_str: Vec<String> = chunk.iter().map(|u| u.to_string()).collect();
            uids_str.join(",")
        };

        if let Ok(mut fetch_stream) = session.uid_fetch(&uid_range, "(UID FLAGS)").await {
            let mut flag_updates = Vec::new();
            while let Ok(Some(fetch)) = fetch_stream.try_next().await {
                if let Some(uid) = fetch.uid {
                    if uid > max_uid {
                        max_uid = uid;
                    }
                    let flags: Vec<async_imap::types::Flag> = fetch.flags().collect();
                    let is_read = flags.iter().any(|f| matches!(f, async_imap::types::Flag::Seen));
                    let is_flagged = flags.iter().any(|f| matches!(f, async_imap::types::Flag::Flagged));
                    let is_deleted = flags.iter().any(|f| matches!(f, async_imap::types::Flag::Deleted));
                    flag_updates.push((uid, is_read, is_flagged, is_deleted));
                }
            }
            if !flag_updates.is_empty() {
                let _ = storage.update_message_flags_batch(&folder.id, &flag_updates);
            }
        }
    }

    folder.last_synced_uid = max_uid;
    folder.total_messages = total_messages;
    folder.uid_validity = uid_validity;

    let _ = storage.update_folder_stats(&folder.id, max_uid, total_messages, 0);

    info!(
        "Synced and offline-cached {} messages for folder '{}'",
        synced_new_or_updated, folder.remote_name
    );

    Ok(synced_new_or_updated)
}
