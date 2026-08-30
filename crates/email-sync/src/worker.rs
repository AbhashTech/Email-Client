use crate::body_fetch::parse_full_mime_and_enrich_header;
use crate::connection::connect_imap;
use crate::folder_sync::{discover_remote_folders, sync_single_folder};
use email_core::error::{EmailError, Result};
use email_core::events::{SyncCommand, SyncEvent};
use email_core::models::MessageHeader;
use email_keychain::CredentialStore;
use email_smtp::SmtpClient;
use email_storage::Storage;
use futures::TryStreamExt;
use log::{error, info};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};


pub struct SyncWorker {
    storage: Storage,
    keyring: Arc<dyn CredentialStore>,
    cmd_rx: mpsc::UnboundedReceiver<SyncCommand>,
    event_tx: broadcast::Sender<SyncEvent>,
}

impl SyncWorker {
    pub fn new(
        storage: Storage,
        keyring: Arc<dyn CredentialStore>,
        cmd_rx: mpsc::UnboundedReceiver<SyncCommand>,
        event_tx: broadcast::Sender<SyncEvent>,
    ) -> Self {
        Self {
            storage,
            keyring,
            cmd_rx,
            event_tx,
        }
    }

    pub async fn run(mut self) {
        info!("SyncWorker actor started.");

        while let Some(cmd) = self.cmd_rx.recv().await {
            let storage = self.storage.clone();
            let keyring = self.keyring.clone();
            let event_tx = self.event_tx.clone();

            // Process commands sequentially to strictly respect server concurrent connection limits
            if let Err(e) = Self::handle_command(cmd, &storage, keyring.as_ref(), &event_tx).await {
                error!("Error handling sync command: {}", e);
                let _ = event_tx.send(SyncEvent::SyncError {
                    account_id: None,
                    error_message: e.to_string(),
                });
            }
        }

        info!("SyncWorker actor stopped.");
    }

    async fn handle_command(
        cmd: SyncCommand,
        storage: &Storage,
        keyring: &dyn CredentialStore,
        event_tx: &broadcast::Sender<SyncEvent>,
    ) -> Result<()> {
        match cmd {
            SyncCommand::TestConnection { account, password } => {
                let _ = event_tx.send(SyncEvent::SyncStatusChanged {
                    is_syncing: true,
                    status_text: format!("Testing connection for {}...", account.email),
                });

                let mut imap_res = connect_imap(&account, &password).await;
                let smtp_res = SmtpClient::test_connection(&account, &password).await;

                let imap_ok = imap_res.is_ok();
                let smtp_ok = smtp_res.is_ok();
                let success = imap_ok && smtp_ok;

                let message = if success {
                    "Both IMAP and SMTP connections verified successfully!".to_string()
                } else {
                    let mut errs = Vec::new();
                    if let Err(ref e) = imap_res {
                        errs.push(format!("IMAP: {}", e));
                    }
                    if let Err(ref e) = smtp_res {
                        errs.push(format!("SMTP: {}", e));
                    }
                    errs.join(" | ")
                };

                // Explicitly logout IMAP session so server immediately frees connection slot
                if let Ok(ref mut session) = imap_res {
                    let _ = session.logout().await;
                }

                let _ = event_tx.send(SyncEvent::ConnectionTestResult {
                    success,
                    imap_ok,
                    smtp_ok,
                    message,
                });

                let _ = event_tx.send(SyncEvent::SyncStatusChanged {
                    is_syncing: false,
                    status_text: "Connection test complete".to_string(),
                });
            }

            SyncCommand::DiscoverFolders { account, password } => {
                let _ = event_tx.send(SyncEvent::SyncStatusChanged {
                    is_syncing: true,
                    status_text: format!("Discovering folders for {}...", account.email),
                });

                let mut session = connect_imap(&account, &password).await?;
                let folders_res = discover_remote_folders(&mut session, &account).await;
                let _ = session.logout().await;

                let folders = folders_res?;
                storage.save_folders(&folders)?;

                let _ = event_tx.send(SyncEvent::FoldersDiscovered {
                    account_id: account.id.clone(),
                    folders,
                });

                let _ = event_tx.send(SyncEvent::SyncStatusChanged {
                    is_syncing: false,
                    status_text: "Folder discovery complete".to_string(),
                });
            }

            SyncCommand::SyncAll => {
                let accounts = storage.get_accounts()?;
                let _ = event_tx.send(SyncEvent::SyncStatusChanged {
                    is_syncing: true,
                    status_text: format!("Syncing {} account(s)...", accounts.len()),
                });

                for account in accounts {
                    if !account.is_enabled {
                        continue;
                    }
                    if let Ok(password) = keyring.get_credential(&account.credential_key) {
                        if let Ok(mut session) = connect_imap(&account, &password).await {
                            let mut folders = storage.get_folders_for_account(&account.id)?;
                            for folder in &mut folders {
                                if folder.is_synced {
                                    if let Ok(count) = sync_single_folder(&mut session, &account, folder, storage).await {
                                        let _ = event_tx.send(SyncEvent::FolderSynced {
                                            account_id: account.id.clone(),
                                            folder_id: folder.id.clone(),
                                            new_messages_count: count,
                                        });
                                    }
                                }
                            }
                            let _ = session.logout().await;
                        }
                    }
                }

                let _ = event_tx.send(SyncEvent::SyncStatusChanged {
                    is_syncing: false,
                    status_text: "Sync complete".to_string(),
                });
            }

            SyncCommand::SyncAccount { account_id } => {
                let account = storage.get_account(&account_id)?;
                let password = keyring.get_credential(&account.credential_key)?;

                let _ = event_tx.send(SyncEvent::SyncStatusChanged {
                    is_syncing: true,
                    status_text: format!("Syncing {}...", account.email),
                });

                let mut session = connect_imap(&account, &password).await?;
                let mut folders = storage.get_folders_for_account(&account.id)?;

                // If no folders exist yet, discover them first
                if folders.is_empty() {
                    if let Ok(discovered) = discover_remote_folders(&mut session, &account).await {
                        let _ = storage.save_folders(&discovered);
                        folders = discovered;
                    }
                }

                for folder in &mut folders {
                    if folder.is_synced {
                        if let Ok(count) = sync_single_folder(&mut session, &account, folder, storage).await {
                            let _ = event_tx.send(SyncEvent::FolderSynced {
                                account_id: account.id.clone(),
                                folder_id: folder.id.clone(),
                                new_messages_count: count,
                            });
                        }
                    }
                }

                let _ = session.logout().await;

                let _ = event_tx.send(SyncEvent::SyncStatusChanged {
                    is_syncing: false,
                    status_text: format!("Sync finished for {}", account.email),
                });
            }

            SyncCommand::SyncFolder { account_id, folder_id } => {
                let account = storage.get_account(&account_id)?;
                let password = keyring.get_credential(&account.credential_key)?;
                let mut folders = storage.get_folders_for_account(&account.id)?;
                if let Some(folder) = folders.iter_mut().find(|f| f.id == folder_id) {
                    let mut session = connect_imap(&account, &password).await?;
                    let count_res = sync_single_folder(&mut session, &account, folder, storage).await;
                    let _ = session.logout().await;

                    let count = count_res?;
                    let _ = event_tx.send(SyncEvent::FolderSynced {
                        account_id,
                        folder_id,
                        new_messages_count: count,
                    });
                }
            }

            SyncCommand::FetchBody {
                account_id,
                folder_id,
                uid,
                message_id,
            } => {
                // Check if already fetched in DB
                if let Ok(Some(detail)) = storage.get_message_detail(&message_id) {
                    if detail.header.body_fetched {
                        let _ = event_tx.send(SyncEvent::BodyFetched {
                            message_id,
                            detail: Box::new(detail),
                        });
                        return Ok(());
                    }
                }

                let account = storage.get_account(&account_id)?;
                let password = keyring.get_credential(&account.credential_key)?;
                let folders = storage.get_folders_for_account(&account.id)?;
                let folder = folders
                    .into_iter()
                    .find(|f| f.id == folder_id)
                    .ok_or_else(|| EmailError::FolderNotFound(folder_id.clone()))?;

                let mut session = connect_imap(&account, &password).await?;
                let select_res = session.select(&folder.remote_name).await;
                if let Err(e) = select_res {
                    let _ = session.logout().await;
                    return Err(EmailError::Imap(format!("Failed to select {}: {}", folder.remote_name, e)));
                }

                let mut header = if let Ok(Some(existing_detail)) = storage.get_message_detail(&message_id) {
                    existing_detail.header
                } else {
                    MessageHeader {
                        id: message_id.clone(),
                        account_id: account_id.clone(),
                        folder_id: folder_id.clone(),
                        uid,
                        message_id: None,
                        in_reply_to: None,
                        subject: String::new(),
                        from_name: None,
                        from_address: String::new(),
                        to_recipients: Vec::new(),
                        cc_recipients: Vec::new(),
                        date_epoch: 0,
                        snippet: String::new(),
                        is_read: true,
                        is_flagged: false,
                        is_draft: false,
                        is_deleted: false,
                        body_fetched: true,
                        size_bytes: 0,
                    }
                };

                let query = format!("{}", uid);
                let mut fetch_stream = session
                    .uid_fetch(&query, "BODY.PEEK[]")
                    .await
                    .map_err(|e| EmailError::Imap(format!("Failed to fetch body for UID {}: {}", uid, e)))?;

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
                drop(fetch_stream);
                let _ = session.logout().await;


                if let Some(bytes) = raw_bytes {
                    let parsed = parse_full_mime_and_enrich_header(&bytes, &mut header)?;
                    storage.save_message_headers(&[header])?;

                    storage.save_message_body(
                        &message_id,
                        parsed.plain_text.as_deref(),
                        parsed.html_text.as_deref(),
                    )?;

                    if !parsed.attachments.is_empty() {
                        storage.save_attachments(&parsed.attachments)?;
                    }
                }

                if let Ok(Some(updated_detail)) = storage.get_message_detail(&message_id) {
                    let _ = event_tx.send(SyncEvent::BodyFetched {
                        message_id,
                        detail: Box::new(updated_detail),
                    });
                }
            }


            SyncCommand::SendEmail { draft, password } => {
                let account = storage.get_account(&draft.account_id)?;
                SmtpClient::send_email(&account, &password, &draft).await?;
                let _ = event_tx.send(SyncEvent::EmailSent {
                    subject: draft.subject,
                });
            }

            SyncCommand::SetReadStatus { .. } => {
                // Handled via storage update
            }

            SyncCommand::SetFlaggedStatus { .. } => {
                // Handled via storage update
            }

            SyncCommand::DeleteMessage { .. } => {
                // Handled via storage update
            }
        }

        Ok(())
    }
}
