use crate::models::{Account, Folder, MessageDetail, OutgoingDraft};

#[derive(Debug, Clone)]
pub enum SyncCommand {
    /// Discover folders for an account
    DiscoverFolders { account: Account, password: String },
    /// Sync all enabled accounts
    SyncAll,
    /// Sync a single account with selective folder filtering & date window
    SyncAccount { account_id: String },
    /// Sync a specific folder
    SyncFolder { account_id: String, folder_id: String },
    /// Fetch message body on demand (lazy loading)
    FetchBody {
        account_id: String,
        folder_id: String,
        uid: u32,
        message_id: String,
    },
    /// Test IMAP & SMTP credentials
    TestConnection {
        account: Account,
        password: String,
    },
    /// Send email draft via SMTP
    SendEmail {
        draft: OutgoingDraft,
        password: String,
    },
    /// Mark message read/unread
    SetReadStatus {
        account_id: String,
        folder_id: String,
        uid: u32,
        is_read: bool,
    },
    /// Star / Flag message
    SetFlaggedStatus {
        account_id: String,
        folder_id: String,
        uid: u32,
        is_flagged: bool,
    },
    /// Delete message
    DeleteMessage {
        account_id: String,
        folder_id: String,
        uid: u32,
    },
    /// Move message to target folder on server
    MoveMessage {
        account_id: String,
        source_folder_id: String,
        target_folder_id: String,
        uid: u32,
        message_id: String,
    },
}

#[derive(Debug, Clone)]
pub enum SyncEvent {
    /// Overall sync status changed
    SyncStatusChanged {
        is_syncing: bool,
        status_text: String,
    },
    /// Discovered folder list from IMAP
    FoldersDiscovered {
        account_id: String,
        folders: Vec<Folder>,
    },
    /// Folder sync completed
    FolderSynced {
        account_id: String,
        folder_id: String,
        new_messages_count: usize,
    },
    /// Message body lazy-loaded
    BodyFetched {
        message_id: String,
        detail: Box<MessageDetail>,
    },
    /// Connection test result
    ConnectionTestResult {
        success: bool,
        imap_ok: bool,
        smtp_ok: bool,
        message: String,
    },
    /// Email sent successfully
    EmailSent {
        subject: String,
    },
    /// Background error occurred
    SyncError {
        account_id: Option<String>,
        error_message: String,
    },
    /// New mail received via IDLE or sync
    NewMailNotification {
        account_id: String,
        from: String,
        subject: String,
    },
}
