pub const SCHEMA_TABLES: &str = r#"
-- Accounts
CREATE TABLE IF NOT EXISTS accounts (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT NOT NULL,
    imap_host TEXT NOT NULL,
    imap_port INTEGER NOT NULL,
    imap_security TEXT NOT NULL,
    smtp_host TEXT NOT NULL,
    smtp_port INTEGER NOT NULL,
    smtp_security TEXT NOT NULL,
    auth_type TEXT NOT NULL,
    credential_key TEXT NOT NULL,
    sync_days_window INTEGER NOT NULL DEFAULT 30,
    is_enabled INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Folders
CREATE TABLE IF NOT EXISTS folders (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    remote_name TEXT NOT NULL,
    display_name TEXT NOT NULL,
    delimiter TEXT NOT NULL DEFAULT '/',
    attributes TEXT NOT NULL DEFAULT '[]',
    is_synced INTEGER NOT NULL DEFAULT 1,
    last_synced_uid INTEGER NOT NULL DEFAULT 0,
    uid_validity INTEGER NOT NULL DEFAULT 0,
    total_messages INTEGER NOT NULL DEFAULT 0,
    unread_messages INTEGER NOT NULL DEFAULT 0,
    UNIQUE(account_id, remote_name)
);

-- Messages (Metadata indexed, lazy body)
CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    folder_id TEXT NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
    uid INTEGER NOT NULL,
    message_id TEXT,
    in_reply_to TEXT,
    subject TEXT NOT NULL DEFAULT '',
    from_name TEXT,
    from_address TEXT NOT NULL,
    to_recipients_json TEXT NOT NULL DEFAULT '[]',
    cc_recipients_json TEXT NOT NULL DEFAULT '[]',
    date_epoch INTEGER NOT NULL,
    snippet TEXT NOT NULL DEFAULT '',
    is_read INTEGER NOT NULL DEFAULT 0,
    is_flagged INTEGER NOT NULL DEFAULT 0,
    is_draft INTEGER NOT NULL DEFAULT 0,
    is_deleted INTEGER NOT NULL DEFAULT 0,
    body_plain TEXT,
    body_html TEXT,
    body_fetched INTEGER NOT NULL DEFAULT 0,
    size_bytes INTEGER NOT NULL DEFAULT 0,
    snooze_until INTEGER,
    UNIQUE(folder_id, uid)
);

-- Attachments
CREATE TABLE IF NOT EXISTS attachments (
    id TEXT PRIMARY KEY,
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    filename TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    content_id TEXT,
    is_inline INTEGER NOT NULL DEFAULT 0,
    local_cache_path TEXT
);

-- Templates
CREATE TABLE IF NOT EXISTS templates (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    subject_template TEXT NOT NULL DEFAULT '',
    body_template TEXT NOT NULL,
    shortcut TEXT,
    created_at INTEGER NOT NULL
);

-- Signatures
CREATE TABLE IF NOT EXISTS signatures (
    id TEXT PRIMARY KEY,
    account_id TEXT REFERENCES accounts(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    content_html TEXT NOT NULL,
    is_default INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);

-- Local Drafts
CREATE TABLE IF NOT EXISTS drafts (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    to_input TEXT NOT NULL DEFAULT '',
    cc_input TEXT NOT NULL DEFAULT '',
    bcc_input TEXT NOT NULL DEFAULT '',
    subject TEXT NOT NULL DEFAULT '',
    body_plain TEXT NOT NULL DEFAULT '',
    format TEXT NOT NULL DEFAULT 'markdown',
    signature_id TEXT,
    in_reply_to TEXT,
    references_header TEXT,
    updated_at INTEGER NOT NULL
);

-- Scheduled Emails (Send Later)
CREATE TABLE IF NOT EXISTS scheduled_emails (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    draft_json TEXT NOT NULL,
    send_at_timestamp INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);

-- PGP End-to-End Encryption Keys
CREATE TABLE IF NOT EXISTS pgp_keys (
    email TEXT PRIMARY KEY,
    fingerprint TEXT NOT NULL,
    public_key_armored TEXT NOT NULL,
    private_key_armored TEXT,
    created_at INTEGER NOT NULL
);

-- Offline Outbox Auto-Retry Queue
CREATE TABLE IF NOT EXISTS outbox (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    draft_json TEXT NOT NULL,
    retry_count INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 5,
    next_retry_timestamp INTEGER NOT NULL,
    last_error TEXT,
    created_at INTEGER NOT NULL
);

-- Application Settings & Persistent Preferences
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

pub const SCHEMA_INDEXES_AND_FTS: &str = r#"
-- Indexes for lightning fast lookups
CREATE INDEX IF NOT EXISTS idx_messages_folder_date ON messages(folder_id, date_epoch DESC);
CREATE INDEX IF NOT EXISTS idx_messages_account_unread ON messages(account_id, is_read);
CREATE INDEX IF NOT EXISTS idx_messages_search ON messages(subject, from_address, snippet);
CREATE INDEX IF NOT EXISTS idx_folders_account ON folders(account_id);
CREATE INDEX IF NOT EXISTS idx_drafts_account ON drafts(account_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_scheduled_due ON scheduled_emails(send_at_timestamp ASC);
CREATE INDEX IF NOT EXISTS idx_messages_snooze ON messages(snooze_until);
CREATE INDEX IF NOT EXISTS idx_outbox_retry ON outbox(next_retry_timestamp ASC);

-- FTS5 Full-Text Search Virtual Table
CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
    message_id UNINDEXED,
    account_id UNINDEXED,
    folder_id UNINDEXED,
    subject,
    from_name,
    from_address,
    snippet,
    body_text,
    to_recipients,
    tokenize = 'unicode61 remove_diacritics 2'
);

-- Synchronization Triggers for FTS5
CREATE TRIGGER IF NOT EXISTS messages_fts_ai AFTER INSERT ON messages BEGIN
    INSERT INTO messages_fts(
        rowid, message_id, account_id, folder_id, subject,
        from_name, from_address, snippet, body_text, to_recipients
    ) VALUES (
        new.rowid, new.id, new.account_id, new.folder_id, new.subject,
        coalesce(new.from_name, ''), new.from_address, new.snippet,
        coalesce(new.body_plain, ''), new.to_recipients_json
    );
END;

CREATE TRIGGER IF NOT EXISTS messages_fts_ad AFTER DELETE ON messages BEGIN
    DELETE FROM messages_fts WHERE message_id = old.id;
END;

CREATE TRIGGER IF NOT EXISTS messages_fts_au AFTER UPDATE ON messages BEGIN
    DELETE FROM messages_fts WHERE message_id = old.id;
    INSERT INTO messages_fts(
        rowid, message_id, account_id, folder_id, subject,
        from_name, from_address, snippet, body_text, to_recipients
    ) VALUES (
        new.rowid, new.id, new.account_id, new.folder_id, new.subject,
        coalesce(new.from_name, ''), new.from_address, new.snippet,
        coalesce(new.body_plain, ''), new.to_recipients_json
    );
END;
"#;

pub const SCHEMA_V1: &str = SCHEMA_TABLES;

