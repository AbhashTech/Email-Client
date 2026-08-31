use crate::connection::connect_imap;
use crate::folder_sync::sync_single_folder;
use email_core::error::{EmailError, Result};
use email_core::events::SyncEvent;
use email_core::models::Account;
use email_keychain::CredentialStore;
use email_storage::Storage;
use log::{info, warn};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

pub struct IdleWorker;

impl IdleWorker {
    /// Starts background IMAP IDLE push listeners for all enabled accounts.
    pub fn start_for_all_accounts(
        storage: Storage,
        keyring: Arc<dyn CredentialStore>,
        event_tx: broadcast::Sender<SyncEvent>,
    ) {
        tokio::spawn(async move {
            let Ok(accounts) = storage.get_accounts() else {
                return;
            };

            for account in accounts {
                if !account.is_enabled {
                    continue;
                }

                let acc_storage = storage.clone();
                let acc_keyring = keyring.clone();
                let acc_event_tx = event_tx.clone();

                tokio::spawn(async move {
                    Self::run_account_idle_loop(account, acc_storage, acc_keyring, acc_event_tx).await;
                });
            }
        });
    }

    async fn run_account_idle_loop(
        account: Account,
        storage: Storage,
        keyring: Arc<dyn CredentialStore>,
        event_tx: broadcast::Sender<SyncEvent>,
    ) {
        let mut backoff_secs = 5;

        loop {
            info!("Starting IMAP IDLE push listener for {}", account.email);

            let run_res = Self::idle_account_session(&account, &storage, keyring.as_ref(), &event_tx).await;
            if let Err(e) = run_res {
                warn!("IMAP IDLE disconnected for {}: {}. Reconnecting in {}s...", account.email, e, backoff_secs);
                tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                backoff_secs = (backoff_secs * 2).min(60);
            } else {
                backoff_secs = 5;
            }
        }
    }

    async fn idle_account_session(
        account: &Account,
        storage: &Storage,
        keyring: &dyn CredentialStore,
        event_tx: &broadcast::Sender<SyncEvent>,
    ) -> Result<()> {
        let password = keyring.get_credential(&account.credential_key)?;
        let mut session = connect_imap(account, &password).await?;

        // Determine INBOX remote name
        let folders = storage.get_folders_for_account(&account.id)?;
        let inbox_folder = folders
            .iter()
            .find(|f| f.is_inbox() || f.remote_name.to_uppercase() == "INBOX")
            .cloned();

        let remote_name = inbox_folder
            .as_ref()
            .map(|f| f.remote_name.clone())
            .unwrap_or_else(|| "INBOX".to_string());

        session
            .select(&remote_name)
            .await
            .map_err(|e| EmailError::Imap(format!("Failed to select {}: {}", remote_name, e)))?;

        info!("IMAP IDLE: Connected and idling on '{}' for {}", remote_name, account.email);

        // Keep connection idling with periodic 14-min keepalive renewals
        loop {
            let mut idle = session.idle();
            idle.init().await.map_err(|e| EmailError::Imap(e.to_string()))?;
            let (idle_wait, interrupt) = idle.wait();

            let keepalive_duration = Duration::from_secs(14 * 60);

            tokio::select! {
                idle_res = idle_wait => {
                    match idle_res {
                        Ok(async_imap::extensions::idle::IdleResponse::NewData(_)) => {
                            info!("IMAP IDLE: Push event received from server for {}", account.email);
                            session = idle.done().await.map_err(|e| EmailError::Imap(e.to_string()))?;

                            // Sync new emails from INBOX
                            let mut current_folders = storage.get_folders_for_account(&account.id)?;
                            if let Some(folder) = current_folders.iter_mut().find(|f| f.is_inbox() || f.remote_name.to_uppercase() == "INBOX") {
                                if let Ok(count) = sync_single_folder(&mut session, account, folder, storage).await {
                                    let _ = event_tx.send(SyncEvent::FolderSynced {
                                        account_id: account.id.clone(),
                                        folder_id: folder.id.clone(),
                                        new_messages_count: count,
                                    });
                                    if count > 0 {
                                        let _ = event_tx.send(SyncEvent::NewMailNotification {
                                            account_id: account.id.clone(),
                                            from: account.name.clone(),
                                            subject: format!("{} new email(s) received", count),
                                        });
                                    }
                                }
                            }
                        }
                        Ok(async_imap::extensions::idle::IdleResponse::ManualInterrupt) => {
                            session = idle.done().await.map_err(|e| EmailError::Imap(e.to_string()))?;
                        }
                        Ok(async_imap::extensions::idle::IdleResponse::Timeout) => {
                            session = idle.done().await.map_err(|e| EmailError::Imap(e.to_string()))?;
                        }
                        Err(e) => {
                            return Err(EmailError::Imap(e.to_string()));
                        }
                    }
                }
                _ = tokio::time::sleep(keepalive_duration) => {
                    drop(interrupt);
                    session = idle.done().await.map_err(|e| EmailError::Imap(e.to_string()))?;
                    info!("IMAP IDLE: 14-min keepalive renewed for {}", account.email);
                }
            }
        }
    }
}
