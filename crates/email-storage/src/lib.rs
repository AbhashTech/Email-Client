pub mod schema;

use email_core::error::{EmailError, Result};
use email_core::models::*;
use log::info;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, OptionalExtension};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ParsedSearchQuery {
    pub from: Vec<String>,
    pub to: Vec<String>,
    pub subject: Vec<String>,
    pub is_unread: Option<bool>,
    pub is_flagged: Option<bool>,
    pub has_attachment: bool,
    pub free_text: Vec<String>,
}

pub fn parse_search_query(raw: &str) -> ParsedSearchQuery {
    let mut parsed = ParsedSearchQuery::default();
    let tokens = raw.split_whitespace();

    for token in tokens {
        if let Some(rest) = token.strip_prefix("from:") {
            if !rest.is_empty() {
                parsed.from.push(rest.to_lowercase());
            }
        } else if let Some(rest) = token.strip_prefix("to:") {
            if !rest.is_empty() {
                parsed.to.push(rest.to_lowercase());
            }
        } else if let Some(rest) = token.strip_prefix("subject:") {
            if !rest.is_empty() {
                parsed.subject.push(rest.to_lowercase());
            }
        } else if token.eq_ignore_ascii_case("is:unread") {
            parsed.is_unread = Some(true);
        } else if token.eq_ignore_ascii_case("is:read") {
            parsed.is_unread = Some(false);
        } else if token.eq_ignore_ascii_case("is:starred") || token.eq_ignore_ascii_case("is:flagged") {
            parsed.is_flagged = Some(true);
        } else if token.eq_ignore_ascii_case("has:attachment") || token.eq_ignore_ascii_case("has:attachments") {
            parsed.has_attachment = true;
        } else {
            parsed.free_text.push(token.to_string());
        }
    }

    parsed
}

pub fn sanitize_fts5_token(token: &str) -> String {
    let cleaned: String = token
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '@' || *c == '.')
        .collect();
    if cleaned.is_empty() {
        String::new()
    } else {
        format!("\"{}\"*", cleaned)
    }
}

#[derive(Clone)]
pub struct Storage {
    pool: Pool<SqliteConnectionManager>,
}

impl Storage {
    pub fn new_in_memory() -> Result<Self> {
        let manager = SqliteConnectionManager::memory().with_init(|c| {
            let _ = c.pragma_update(None, "journal_mode", "WAL");
            let _ = c.pragma_update(None, "synchronous", "NORMAL");
            let _ = c.pragma_update(None, "foreign_keys", "ON");
            let _ = c.pragma_update(None, "busy_timeout", "5000");
            Ok(())
        });
        let pool = Pool::new(manager)
            .map_err(|e| EmailError::Database(format!("Failed to create memory pool: {}", e)))?;
        let storage = Self { pool };
        storage.init_schema()?;
        Ok(storage)
    }

    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let manager = SqliteConnectionManager::file(path.as_ref()).with_init(|c| {
            let _ = c.pragma_update(None, "journal_mode", "WAL");
            let _ = c.pragma_update(None, "synchronous", "NORMAL");
            let _ = c.pragma_update(None, "foreign_keys", "ON");
            let _ = c.pragma_update(None, "busy_timeout", "5000");
            Ok(())
        });
        let pool = Pool::builder()
            .max_size(16)
            .build(manager)
            .map_err(|e| EmailError::Database(format!("Failed to create DB pool: {}", e)))?;
        let storage = Self { pool };
        storage.init_schema()?;
        Ok(storage)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| EmailError::Database(e.to_string()))?;
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        let _ = conn.pragma_update(None, "synchronous", "NORMAL");
        let _ = conn.pragma_update(None, "foreign_keys", "ON");
        let _ = conn.pragma_update(None, "busy_timeout", "5000");

        // 1. Create base tables
        conn.execute_batch(schema::SCHEMA_TABLES)
            .map_err(|e| EmailError::Database(format!("Base schema init error: {}", e)))?;

        // 2. Perform safe column migrations on existing tables before index creation
        Self::migrate_columns(&conn)?;

        // 3. Create indexes, triggers, and FTS5 virtual tables
        conn.execute_batch(schema::SCHEMA_INDEXES_AND_FTS)
            .map_err(|e| EmailError::Database(format!("Indexes & FTS init error: {}", e)))?;

        // 4. Populate FTS5 table if existing messages are not yet indexed
        let fts_count: i64 = conn
            .query_row("SELECT count(*) FROM messages_fts", [], |r| r.get(0))
            .unwrap_or(0);
        let msg_count: i64 = conn
            .query_row("SELECT count(*) FROM messages", [], |r| r.get(0))
            .unwrap_or(0);
        if fts_count == 0 && msg_count > 0 {
            let _ = conn.execute_batch(r#"
                INSERT INTO messages_fts(
                    rowid, message_id, account_id, folder_id, subject,
                    from_name, from_address, snippet, body_text, to_recipients
                )
                SELECT
                    rowid, id, account_id, folder_id, subject,
                    coalesce(from_name, ''), from_address, snippet,
                    coalesce(body_plain, ''), to_recipients_json
                FROM messages;
            "#);
        }

        info!("SQLite database initialized successfully with WAL mode & automated migrations.");
        Ok(())
    }

    fn migrate_columns(conn: &rusqlite::Connection) -> Result<()> {
        // Check messages table columns
        let mut stmt = conn
            .prepare("PRAGMA table_info(messages)")
            .map_err(|e| EmailError::Database(e.to_string()))?;
        let columns: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|e| EmailError::Database(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        if !columns.is_empty() && !columns.iter().any(|c| c == "snooze_until") {
            conn.execute("ALTER TABLE messages ADD COLUMN snooze_until INTEGER", [])
                .map_err(|e| EmailError::Database(format!("Failed to add snooze_until column: {}", e)))?;
        }

        // Check accounts table columns
        let mut stmt = conn
            .prepare("PRAGMA table_info(accounts)")
            .map_err(|e| EmailError::Database(e.to_string()))?;
        let acc_columns: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|e| EmailError::Database(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        if !acc_columns.is_empty() && !acc_columns.iter().any(|c| c == "sync_days_window") {
            let _ = conn.execute("ALTER TABLE accounts ADD COLUMN sync_days_window INTEGER NOT NULL DEFAULT 30", []);
        }

        Ok(())
    }

    // ==========================================
    // Accounts
    // ==========================================

    pub fn save_account(&self, account: &Account) -> Result<()> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        conn.execute(
            r#"
            INSERT INTO accounts (
                id, name, email, imap_host, imap_port, imap_security,
                smtp_host, smtp_port, smtp_security, auth_type, credential_key,
                sync_days_window, is_enabled, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            ON CONFLICT(id) DO UPDATE SET
                name=excluded.name,
                email=excluded.email,
                imap_host=excluded.imap_host,
                imap_port=excluded.imap_port,
                imap_security=excluded.imap_security,
                smtp_host=excluded.smtp_host,
                smtp_port=excluded.smtp_port,
                smtp_security=excluded.smtp_security,
                auth_type=excluded.auth_type,
                credential_key=excluded.credential_key,
                sync_days_window=excluded.sync_days_window,
                is_enabled=excluded.is_enabled,
                updated_at=excluded.updated_at
            "#,
            params![
                account.id,
                account.name,
                account.email,
                account.imap_host,
                account.imap_port,
                account.imap_security.as_str(),
                account.smtp_host,
                account.smtp_port,
                account.smtp_security.as_str(),
                account.auth_type.as_str(),
                account.credential_key,
                account.sync_days_window.days().unwrap_or(0),
                if account.is_enabled { 1 } else { 0 },
                account.created_at,
                account.updated_at,
            ],
        ).map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn get_accounts(&self) -> Result<Vec<Account>> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, email, imap_host, imap_port, imap_security,
                        smtp_host, smtp_port, smtp_security, auth_type, credential_key,
                        sync_days_window, is_enabled, created_at, updated_at
                 FROM accounts ORDER BY created_at ASC",
            )
            .map_err(|e| EmailError::Database(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                let imap_sec_str: String = row.get(5)?;
                let smtp_sec_str: String = row.get(8)?;
                let auth_str: String = row.get(9)?;
                let sync_days: i64 = row.get(11)?;
                let is_enabled: i32 = row.get(12)?;

                Ok(Account {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    email: row.get(2)?,
                    imap_host: row.get(3)?,
                    imap_port: row.get(4)?,
                    imap_security: SecurityType::from_str(&imap_sec_str),
                    smtp_host: row.get(6)?,
                    smtp_port: row.get(7)?,
                    smtp_security: SecurityType::from_str(&smtp_sec_str),
                    auth_type: AuthType::from_str(&auth_str),
                    credential_key: row.get(10)?,
                    sync_days_window: SyncWindow::from_days(sync_days),
                    is_enabled: is_enabled == 1,
                    created_at: row.get(13)?,
                    updated_at: row.get(14)?,
                })
            })
            .map_err(|e| EmailError::Database(e.to_string()))?;

        let mut accounts = Vec::new();
        for row in rows {
            accounts.push(row.map_err(|e| EmailError::Database(e.to_string()))?);
        }
        Ok(accounts)
    }

    pub fn get_account(&self, id: &str) -> Result<Account> {
        let accounts = self.get_accounts()?;
        accounts
            .into_iter()
            .find(|a| a.id == id)
            .ok_or_else(|| EmailError::AccountNotFound(id.to_string()))
    }

    pub fn delete_account(&self, id: &str) -> Result<()> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        conn.execute("DELETE FROM accounts WHERE id = ?1", params![id])
            .map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    // ==========================================
    // Folders
    // ==========================================

    pub fn save_folders(&self, folders: &[Folder]) -> Result<()> {
        let mut conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let tx = conn.transaction().map_err(|e| EmailError::Database(e.to_string()))?;
        {
            let mut stmt = tx.prepare(
                r#"
                INSERT INTO folders (
                    id, account_id, remote_name, display_name, delimiter, attributes,
                    is_synced, last_synced_uid, uid_validity, total_messages, unread_messages
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                ON CONFLICT(account_id, remote_name) DO UPDATE SET
                    display_name=excluded.display_name,
                    delimiter=excluded.delimiter,
                    attributes=excluded.attributes,
                    total_messages=excluded.total_messages,
                    unread_messages=excluded.unread_messages
                "#
            ).map_err(|e| EmailError::Database(e.to_string()))?;

            for f in folders {
                let attrs_json = serde_json::to_string(&f.attributes).unwrap_or_else(|_| "[]".to_string());
                stmt.execute(params![
                    f.id,
                    f.account_id,
                    f.remote_name,
                    f.display_name,
                    f.delimiter,
                    attrs_json,
                    if f.is_synced { 1 } else { 0 },
                    f.last_synced_uid,
                    f.uid_validity,
                    f.total_messages,
                    f.unread_messages,
                ]).map_err(|e| EmailError::Database(e.to_string()))?;
            }
        }
        tx.commit().map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn get_folders_for_account(&self, account_id: &str) -> Result<Vec<Folder>> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT f.id, f.account_id, f.remote_name, f.display_name, f.delimiter, f.attributes,
                        f.is_synced, f.last_synced_uid, f.uid_validity,
                        (SELECT COUNT(*) FROM messages m WHERE m.folder_id = f.id AND m.is_deleted = 0) as total_count,
                        (SELECT COUNT(*) FROM messages m WHERE m.folder_id = f.id AND m.is_read = 0 AND m.is_deleted = 0) as unread_count
                 FROM folders f WHERE f.account_id = ?1 ORDER BY f.display_name ASC",
            )
            .map_err(|e| EmailError::Database(e.to_string()))?;

        let rows = stmt
            .query_map(params![account_id], |row| {
                let attrs_json: String = row.get(5)?;
                let attrs: Vec<String> = serde_json::from_str(&attrs_json).unwrap_or_default();
                let is_synced: i32 = row.get(6)?;
                let total_messages: i64 = row.get(9)?;
                let unread_messages: i64 = row.get(10)?;

                Ok(Folder {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    remote_name: row.get(2)?,
                    display_name: row.get(3)?,
                    delimiter: row.get(4)?,
                    attributes: attrs,
                    is_synced: is_synced == 1,
                    last_synced_uid: row.get(7)?,
                    uid_validity: row.get(8)?,
                    total_messages: total_messages as u32,
                    unread_messages: unread_messages as u32,
                })
            })
            .map_err(|e| EmailError::Database(e.to_string()))?;

        let mut folders = Vec::new();
        for row in rows {
            folders.push(row.map_err(|e| EmailError::Database(e.to_string()))?);
        }
        Ok(folders)
    }

    pub fn set_folder_sync_enabled(&self, folder_id: &str, enabled: bool) -> Result<()> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        conn.execute(
            "UPDATE folders SET is_synced = ?1 WHERE id = ?2",
            params![if enabled { 1 } else { 0 }, folder_id],
        )
        .map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn update_folder_stats(&self, folder_id: &str, last_uid: u32, total: u32, unread: u32) -> Result<()> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        conn.execute(
            "UPDATE folders SET last_synced_uid = ?1, total_messages = ?2, unread_messages = ?3 WHERE id = ?4",
            params![last_uid, total, unread, folder_id],
        )
        .map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    // ==========================================
    // Messages
    // ==========================================

    pub fn save_full_messages(
        &self,
        messages: &[(MessageHeader, Option<String>, Option<String>, Vec<Attachment>)],
    ) -> Result<()> {
        let mut conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let tx = conn.transaction().map_err(|e| EmailError::Database(e.to_string()))?;
        {
            let mut msg_stmt = tx.prepare(
                r#"
                INSERT INTO messages (
                    id, account_id, folder_id, uid, message_id, in_reply_to,
                    subject, from_name, from_address, to_recipients_json, cc_recipients_json,
                    date_epoch, snippet, is_read, is_flagged, is_draft, is_deleted,
                    body_plain, body_html, body_fetched, size_bytes
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)
                ON CONFLICT(folder_id, uid) DO UPDATE SET
                    id=excluded.id,
                    subject=excluded.subject,
                    from_name=excluded.from_name,
                    from_address=excluded.from_address,
                    to_recipients_json=excluded.to_recipients_json,
                    cc_recipients_json=excluded.cc_recipients_json,
                    date_epoch=excluded.date_epoch,
                    snippet=excluded.snippet,
                    is_read=excluded.is_read,
                    is_flagged=excluded.is_flagged,
                    is_draft=excluded.is_draft,
                    is_deleted=excluded.is_deleted,
                    body_plain=COALESCE(excluded.body_plain, messages.body_plain),
                    body_html=COALESCE(excluded.body_html, messages.body_html),
                    body_fetched=MAX(excluded.body_fetched, messages.body_fetched),
                    size_bytes=excluded.size_bytes
                "#
            ).map_err(|e| EmailError::Database(e.to_string()))?;

            let mut att_stmt = tx.prepare(
                r#"
                INSERT INTO attachments (
                    id, message_id, filename, mime_type, size_bytes, content_id, is_inline, local_cache_path
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ON CONFLICT(id) DO NOTHING
                "#
            ).map_err(|e| EmailError::Database(e.to_string()))?;

            for (m, plain, html, atts) in messages {
                let to_json = serde_json::to_string(&m.to_recipients).unwrap_or_else(|_| "[]".to_string());
                let cc_json = serde_json::to_string(&m.cc_recipients).unwrap_or_else(|_| "[]".to_string());
                let body_fetched = if m.body_fetched || plain.is_some() || html.is_some() { 1 } else { 0 };

                msg_stmt.execute(params![
                    m.id,
                    m.account_id,
                    m.folder_id,
                    m.uid,
                    m.message_id,
                    m.in_reply_to,
                    m.subject,
                    m.from_name,
                    m.from_address,
                    to_json,
                    cc_json,
                    m.date_epoch,
                    m.snippet,
                    if m.is_read { 1 } else { 0 },
                    if m.is_flagged { 1 } else { 0 },
                    if m.is_draft { 1 } else { 0 },
                    if m.is_deleted { 1 } else { 0 },
                    plain,
                    html,
                    body_fetched,
                    m.size_bytes as i64,
                ]).map_err(|e| EmailError::Database(e.to_string()))?;

                for a in atts {
                    att_stmt.execute(params![
                        a.id,
                        a.message_id,
                        a.filename,
                        a.mime_type,
                        a.size_bytes as i64,
                        a.content_id,
                        if a.is_inline { 1 } else { 0 },
                        a.local_cache_path,
                    ]).map_err(|e| EmailError::Database(e.to_string()))?;
                }
            }
        }
        tx.commit().map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn get_folder_cached_uids(&self, folder_id: &str) -> Result<HashMap<u32, bool>> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT uid, body_fetched FROM messages WHERE folder_id = ?1 AND is_deleted = 0")
            .map_err(|e| EmailError::Database(e.to_string()))?;

        let rows = stmt
            .query_map(params![folder_id], |row| {
                let uid: u32 = row.get(0)?;
                let body_fetched: i32 = row.get(1)?;
                Ok((uid, body_fetched == 1))
            })
            .map_err(|e| EmailError::Database(e.to_string()))?;

        let mut map = HashMap::new();
        for r in rows {
            let (uid, fetched) = r.map_err(|e| EmailError::Database(e.to_string()))?;
            map.insert(uid, fetched);
        }
        Ok(map)
    }

    pub fn update_message_flags_batch(
        &self,
        folder_id: &str,
        updates: &[(u32, bool, bool, bool)],
    ) -> Result<()> {
        if updates.is_empty() {
            return Ok(());
        }
        let mut conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let tx = conn.transaction().map_err(|e| EmailError::Database(e.to_string()))?;
        {
            let mut stmt = tx.prepare(
                "UPDATE messages SET is_read = ?1, is_flagged = ?2, is_deleted = ?3 WHERE folder_id = ?4 AND uid = ?5"
            ).map_err(|e| EmailError::Database(e.to_string()))?;

            for (uid, is_read, is_flagged, is_deleted) in updates {
                stmt.execute(params![
                    if *is_read { 1 } else { 0 },
                    if *is_flagged { 1 } else { 0 },
                    if *is_deleted { 1 } else { 0 },
                    folder_id,
                    uid,
                ]).map_err(|e| EmailError::Database(e.to_string()))?;
            }
        }
        tx.commit().map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn save_message_headers(&self, headers: &[MessageHeader]) -> Result<()> {
        let mut conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let tx = conn.transaction().map_err(|e| EmailError::Database(e.to_string()))?;
        {
            let mut stmt = tx.prepare(
                r#"
                INSERT INTO messages (
                    id, account_id, folder_id, uid, message_id, in_reply_to,
                    subject, from_name, from_address, to_recipients_json, cc_recipients_json,
                    date_epoch, snippet, is_read, is_flagged, is_draft, is_deleted,
                    body_fetched, size_bytes, snooze_until
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
                ON CONFLICT(folder_id, uid) DO UPDATE SET
                    id=excluded.id,
                    subject=excluded.subject,
                    from_name=excluded.from_name,
                    from_address=excluded.from_address,
                    to_recipients_json=excluded.to_recipients_json,
                    cc_recipients_json=excluded.cc_recipients_json,
                    date_epoch=excluded.date_epoch,
                    snippet=excluded.snippet,
                    is_read=excluded.is_read,
                    is_flagged=excluded.is_flagged,
                    is_draft=excluded.is_draft,
                    is_deleted=excluded.is_deleted,
                    body_fetched=excluded.body_fetched,
                    size_bytes=excluded.size_bytes,
                    snooze_until=coalesce(messages.snooze_until, excluded.snooze_until)
                "#
            ).map_err(|e| EmailError::Database(e.to_string()))?;


            for m in headers {
                let to_json = serde_json::to_string(&m.to_recipients).unwrap_or_else(|_| "[]".to_string());
                let cc_json = serde_json::to_string(&m.cc_recipients).unwrap_or_else(|_| "[]".to_string());

                stmt.execute(params![
                    m.id,
                    m.account_id,
                    m.folder_id,
                    m.uid,
                    m.message_id,
                    m.in_reply_to,
                    m.subject,
                    m.from_name,
                    m.from_address,
                    to_json,
                    cc_json,
                    m.date_epoch,
                    m.snippet,
                    if m.is_read { 1 } else { 0 },
                    if m.is_flagged { 1 } else { 0 },
                    if m.is_draft { 1 } else { 0 },
                    if m.is_deleted { 1 } else { 0 },
                    if m.body_fetched { 1 } else { 0 },
                    m.size_bytes as i64,
                    m.snooze_until,
                ]).map_err(|e| EmailError::Database(e.to_string()))?;
            }
        }
        tx.commit().map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn get_messages(
        &self,
        account_id: Option<&str>,
        folder_id: Option<&str>,
        limit: usize,
        offset: usize,
        search_query: Option<&str>,
    ) -> Result<Vec<MessageHeader>> {
        if let Some(search) = search_query {
            if !search.trim().is_empty() {
                return self.search_messages_fts(account_id, folder_id, search, limit, offset);
            }
        }

        let now_ts = chrono::Utc::now().timestamp();
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;

        let mut query = String::from(
            "SELECT id, account_id, folder_id, uid, message_id, in_reply_to,
                    subject, from_name, from_address, to_recipients_json, cc_recipients_json,
                    date_epoch, snippet, is_read, is_flagged, is_draft, is_deleted,
                    body_fetched, size_bytes, snooze_until
             FROM messages WHERE is_deleted = 0 AND (snooze_until IS NULL OR snooze_until <= ?1)",
        );

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now_ts)];

        if let Some(fid) = folder_id {
            query.push_str(" AND folder_id = ?");
            params_vec.push(Box::new(fid.to_string()));
        } else if let Some(aid) = account_id {
            query.push_str(" AND account_id = ?");
            params_vec.push(Box::new(aid.to_string()));
        }

        query.push_str(" ORDER BY date_epoch DESC LIMIT ? OFFSET ?");
        params_vec.push(Box::new(limit as i64));
        params_vec.push(Box::new(offset as i64));

        let params_slice: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&query).map_err(|e| EmailError::Database(e.to_string()))?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(params_slice), |row| {
                let to_json: String = row.get(9)?;
                let cc_json: String = row.get(10)?;
                let to_recipients: Vec<Recipient> = serde_json::from_str(&to_json).unwrap_or_default();
                let cc_recipients: Vec<Recipient> = serde_json::from_str(&cc_json).unwrap_or_default();
                let is_read: i32 = row.get(13)?;
                let is_flagged: i32 = row.get(14)?;
                let is_draft: i32 = row.get(15)?;
                let is_deleted: i32 = row.get(16)?;
                let body_fetched: i32 = row.get(17)?;
                let size_bytes: i64 = row.get(18)?;

                Ok(MessageHeader {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    folder_id: row.get(2)?,
                    uid: row.get(3)?,
                    message_id: row.get(4)?,
                    in_reply_to: row.get(5)?,
                    subject: row.get(6)?,
                    from_name: row.get(7)?,
                    from_address: row.get(8)?,
                    to_recipients,
                    cc_recipients,
                    date_epoch: row.get(11)?,
                    snippet: row.get(12)?,
                    is_read: is_read == 1,
                    is_flagged: is_flagged == 1,
                    is_draft: is_draft == 1,
                    is_deleted: is_deleted == 1,
                    body_fetched: body_fetched == 1,
                    size_bytes: size_bytes as u64,
                    snooze_until: row.get(19)?,
                })
            })
            .map_err(|e| EmailError::Database(e.to_string()))?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row.map_err(|e| EmailError::Database(e.to_string()))?);
        }
        Ok(messages)
    }

    pub fn search_messages_fts(
        &self,
        account_id: Option<&str>,
        folder_id: Option<&str>,
        search_query: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<MessageHeader>> {
        let now_ts = chrono::Utc::now().timestamp();
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let parsed = parse_search_query(search_query);

        let mut fts_clauses: Vec<String> = Vec::new();

        for f in &parsed.from {
            let tok = sanitize_fts5_token(f);
            if !tok.is_empty() {
                fts_clauses.push(format!("(from_address: {tok} OR from_name: {tok})"));
            }
        }

        for t in &parsed.to {
            let tok = sanitize_fts5_token(t);
            if !tok.is_empty() {
                fts_clauses.push(format!("to_recipients: {tok}"));
            }
        }

        for s in &parsed.subject {
            let tok = sanitize_fts5_token(s);
            if !tok.is_empty() {
                fts_clauses.push(format!("subject: {tok}"));
            }
        }

        for text in &parsed.free_text {
            let tok = sanitize_fts5_token(text);
            if !tok.is_empty() {
                fts_clauses.push(format!(
                    "(subject: {tok} OR snippet: {tok} OR body_text: {tok} OR from_address: {tok} OR from_name: {tok})"
                ));
            }
        }

        let mut query = String::from(
            r#"
            SELECT m.id, m.account_id, m.folder_id, m.uid, m.message_id, m.in_reply_to,
                   m.subject, m.from_name, m.from_address, m.to_recipients_json, m.cc_recipients_json,
                   m.date_epoch, m.snippet, m.is_read, m.is_flagged, m.is_draft, m.is_deleted,
                   m.body_fetched, m.size_bytes, m.snooze_until
            FROM messages m
            "#,
        );

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if !fts_clauses.is_empty() {
            let match_expr = fts_clauses.join(" AND ");
            query.push_str(" JOIN messages_fts f ON f.message_id = m.id WHERE messages_fts MATCH ?");
            params_vec.push(Box::new(match_expr));
            query.push_str(" AND m.is_deleted = 0 AND (m.snooze_until IS NULL OR m.snooze_until <= ?)");
            params_vec.push(Box::new(now_ts));
        } else {
            query.push_str(" WHERE m.is_deleted = 0 AND (m.snooze_until IS NULL OR m.snooze_until <= ?)");
            params_vec.push(Box::new(now_ts));
        }

        if let Some(fid) = folder_id {
            query.push_str(" AND m.folder_id = ?");
            params_vec.push(Box::new(fid.to_string()));
        } else if let Some(aid) = account_id {
            query.push_str(" AND m.account_id = ?");
            params_vec.push(Box::new(aid.to_string()));
        }

        if let Some(unread) = parsed.is_unread {
            if unread {
                query.push_str(" AND m.is_read = 0");
            } else {
                query.push_str(" AND m.is_read = 1");
            }
        }

        if let Some(flagged) = parsed.is_flagged {
            if flagged {
                query.push_str(" AND m.is_flagged = 1");
            }
        }

        if parsed.has_attachment {
            query.push_str(" AND m.id IN (SELECT DISTINCT message_id FROM attachments WHERE is_inline = 0 OR size_bytes > 0)");
        }

        if !fts_clauses.is_empty() {
            query.push_str(" ORDER BY bm25(messages_fts) ASC, m.date_epoch DESC LIMIT ? OFFSET ?");
        } else {
            query.push_str(" ORDER BY m.date_epoch DESC LIMIT ? OFFSET ?");
        }

        params_vec.push(Box::new(limit as i64));
        params_vec.push(Box::new(offset as i64));

        let params_slice: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&query).map_err(|e| EmailError::Database(e.to_string()))?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(params_slice), |row| {
                let to_json: String = row.get(9)?;
                let cc_json: String = row.get(10)?;
                let to_recipients: Vec<Recipient> = serde_json::from_str(&to_json).unwrap_or_default();
                let cc_recipients: Vec<Recipient> = serde_json::from_str(&cc_json).unwrap_or_default();
                let is_read: i32 = row.get(13)?;
                let is_flagged: i32 = row.get(14)?;
                let is_draft: i32 = row.get(15)?;
                let is_deleted: i32 = row.get(16)?;
                let body_fetched: i32 = row.get(17)?;
                let size_bytes: i64 = row.get(18)?;

                Ok(MessageHeader {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    folder_id: row.get(2)?,
                    uid: row.get(3)?,
                    message_id: row.get(4)?,
                    in_reply_to: row.get(5)?,
                    subject: row.get(6)?,
                    from_name: row.get(7)?,
                    from_address: row.get(8)?,
                    to_recipients,
                    cc_recipients,
                    date_epoch: row.get(11)?,
                    snippet: row.get(12)?,
                    is_read: is_read == 1,
                    is_flagged: is_flagged == 1,
                    is_draft: is_draft == 1,
                    is_deleted: is_deleted == 1,
                    body_fetched: body_fetched == 1,
                    size_bytes: size_bytes as u64,
                    snooze_until: row.get(19)?,
                })
            })
            .map_err(|e| EmailError::Database(e.to_string()))?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row.map_err(|e| EmailError::Database(e.to_string()))?);
        }
        Ok(messages)
    }

    pub fn snooze_message(&self, message_id: &str, snooze_until: Option<i64>) -> Result<()> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        conn.execute(
            "UPDATE messages SET snooze_until = ?1 WHERE id = ?2",
            params![snooze_until, message_id],
        ).map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn unsnooze_message(&self, message_id: &str) -> Result<()> {
        self.snooze_message(message_id, None)
    }

    pub fn get_due_snoozed_messages(&self, now_ts: i64) -> Result<Vec<MessageHeader>> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let mut stmt = conn.prepare(
            r#"
            SELECT id, account_id, folder_id, uid, message_id, in_reply_to,
                   subject, from_name, from_address, to_recipients_json, cc_recipients_json,
                   date_epoch, snippet, is_read, is_flagged, is_draft, is_deleted,
                   body_fetched, size_bytes, snooze_until
            FROM messages
            WHERE snooze_until IS NOT NULL AND snooze_until <= ?1 AND is_deleted = 0
            "#,
        ).map_err(|e| EmailError::Database(e.to_string()))?;

        let rows = stmt.query_map(params![now_ts], |row| {
            let to_json: String = row.get(9)?;
            let cc_json: String = row.get(10)?;
            let to_recipients: Vec<Recipient> = serde_json::from_str(&to_json).unwrap_or_default();
            let cc_recipients: Vec<Recipient> = serde_json::from_str(&cc_json).unwrap_or_default();
            let is_read: i32 = row.get(13)?;
            let is_flagged: i32 = row.get(14)?;
            let is_draft: i32 = row.get(15)?;
            let is_deleted: i32 = row.get(16)?;
            let body_fetched: i32 = row.get(17)?;
            let size_bytes: i64 = row.get(18)?;

            Ok(MessageHeader {
                id: row.get(0)?,
                account_id: row.get(1)?,
                folder_id: row.get(2)?,
                uid: row.get(3)?,
                message_id: row.get(4)?,
                in_reply_to: row.get(5)?,
                subject: row.get(6)?,
                from_name: row.get(7)?,
                from_address: row.get(8)?,
                to_recipients,
                cc_recipients,
                date_epoch: row.get(11)?,
                snippet: row.get(12)?,
                is_read: is_read == 1,
                is_flagged: is_flagged == 1,
                is_draft: is_draft == 1,
                is_deleted: is_deleted == 1,
                body_fetched: body_fetched == 1,
                size_bytes: size_bytes as u64,
                snooze_until: row.get(19)?,
            })
        }).map_err(|e| EmailError::Database(e.to_string()))?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r.map_err(|e| EmailError::Database(e.to_string()))?);
        }
        Ok(list)
    }

    pub fn get_snoozed_messages(&self, account_id: Option<&str>) -> Result<Vec<MessageHeader>> {
        let now_ts = chrono::Utc::now().timestamp();
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let mut query = String::from(
            r#"
            SELECT id, account_id, folder_id, uid, message_id, in_reply_to,
                   subject, from_name, from_address, to_recipients_json, cc_recipients_json,
                   date_epoch, snippet, is_read, is_flagged, is_draft, is_deleted,
                   body_fetched, size_bytes, snooze_until
            FROM messages
            WHERE snooze_until IS NOT NULL AND snooze_until > ?1 AND is_deleted = 0
            "#,
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now_ts)];
        if let Some(aid) = account_id {
            query.push_str(" AND account_id = ?2");
            params_vec.push(Box::new(aid.to_string()));
        }
        query.push_str(" ORDER BY snooze_until ASC");

        let params_slice: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&query).map_err(|e| EmailError::Database(e.to_string()))?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params_slice), |row| {
            let to_json: String = row.get(9)?;
            let cc_json: String = row.get(10)?;
            let to_recipients: Vec<Recipient> = serde_json::from_str(&to_json).unwrap_or_default();
            let cc_recipients: Vec<Recipient> = serde_json::from_str(&cc_json).unwrap_or_default();
            let is_read: i32 = row.get(13)?;
            let is_flagged: i32 = row.get(14)?;
            let is_draft: i32 = row.get(15)?;
            let is_deleted: i32 = row.get(16)?;
            let body_fetched: i32 = row.get(17)?;
            let size_bytes: i64 = row.get(18)?;

            Ok(MessageHeader {
                id: row.get(0)?,
                account_id: row.get(1)?,
                folder_id: row.get(2)?,
                uid: row.get(3)?,
                message_id: row.get(4)?,
                in_reply_to: row.get(5)?,
                subject: row.get(6)?,
                from_name: row.get(7)?,
                from_address: row.get(8)?,
                to_recipients,
                cc_recipients,
                date_epoch: row.get(11)?,
                snippet: row.get(12)?,
                is_read: is_read == 1,
                is_flagged: is_flagged == 1,
                is_draft: is_draft == 1,
                is_deleted: is_deleted == 1,
                body_fetched: body_fetched == 1,
                size_bytes: size_bytes as u64,
                snooze_until: row.get(19)?,
            })
        }).map_err(|e| EmailError::Database(e.to_string()))?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r.map_err(|e| EmailError::Database(e.to_string()))?);
        }
        Ok(list)
    }

    pub fn get_message_detail(&self, message_id: &str) -> Result<Option<MessageDetail>> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let result = conn.query_row(
            "SELECT id, account_id, folder_id, uid, message_id, in_reply_to,
                    subject, from_name, from_address, to_recipients_json, cc_recipients_json,
                    date_epoch, snippet, is_read, is_flagged, is_draft, is_deleted,
                    body_plain, body_html, body_fetched, size_bytes, snooze_until
             FROM messages WHERE id = ?1",
            params![message_id],
            |row| {
                let to_json: String = row.get(9)?;
                let cc_json: String = row.get(10)?;
                let to_recipients: Vec<Recipient> = serde_json::from_str(&to_json).unwrap_or_default();
                let cc_recipients: Vec<Recipient> = serde_json::from_str(&cc_json).unwrap_or_default();
                let is_read: i32 = row.get(13)?;
                let is_flagged: i32 = row.get(14)?;
                let is_draft: i32 = row.get(15)?;
                let is_deleted: i32 = row.get(16)?;
                let body_plain: Option<String> = row.get(17)?;
                let body_html: Option<String> = row.get(18)?;
                let body_fetched: i32 = row.get(19)?;
                let size_bytes: i64 = row.get(20)?;

                let header = MessageHeader {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    folder_id: row.get(2)?,
                    uid: row.get(3)?,
                    message_id: row.get(4)?,
                    in_reply_to: row.get(5)?,
                    subject: row.get(6)?,
                    from_name: row.get(7)?,
                    from_address: row.get(8)?,
                    to_recipients,
                    cc_recipients,
                    date_epoch: row.get(11)?,
                    snippet: row.get(12)?,
                    is_read: is_read == 1,
                    is_flagged: is_flagged == 1,
                    is_draft: is_draft == 1,
                    is_deleted: is_deleted == 1,
                    body_fetched: body_fetched == 1,
                    size_bytes: size_bytes as u64,
                    snooze_until: row.get(21)?,
                };

                Ok((header, body_plain, body_html))
            },
        );

        match result {
            Ok((header, body_plain, body_html)) => {
                let attachments = self.get_attachments_for_message(message_id)?;
                Ok(Some(MessageDetail {
                    header,
                    body_plain,
                    body_html,
                    attachments,
                }))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(EmailError::Database(e.to_string())),
        }
    }

    pub fn get_conversation_thread(&self, message_id: &str) -> Result<Option<email_core::models::ConversationThread>> {
        let initial_detail = match self.get_message_detail(message_id)? {
            Some(d) => d,
            None => return Ok(None),
        };

        let root_subject = email_core::models::clean_subject_thread_root(&initial_detail.header.subject);
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;

        let mut stmt = conn.prepare(
            r#"
            SELECT id FROM messages
            WHERE account_id = ?1
              AND is_deleted = 0
              AND (
                (?2 IS NOT NULL AND (message_id = ?2 OR in_reply_to = ?2))
                OR (?3 IS NOT NULL AND in_reply_to = ?3)
                OR (subject != '' AND (
                    subject = ?4
                    OR subject = 'Re: ' || ?4
                    OR subject = 'RE: ' || ?4
                    OR subject = 're: ' || ?4
                    OR subject = 'Fwd: ' || ?4
                    OR subject = 'FWD: ' || ?4
                    OR subject = 'fwd: ' || ?4
                    OR subject = 'Fw: ' || ?4
                ))
              )
            ORDER BY date_epoch ASC
            "#,
        ).map_err(|e| EmailError::Database(e.to_string()))?;

        let rows = stmt.query_map(
            params![
                initial_detail.header.account_id,
                initial_detail.header.message_id,
                initial_detail.header.in_reply_to,
                root_subject,
            ],
            |row| row.get::<_, String>(0),
        ).map_err(|e| EmailError::Database(e.to_string()))?;

        let mut found_ids: Vec<String> = Vec::new();
        for r in rows {
            if let Ok(id) = r {
                found_ids.push(id);
            }
        }

        if !found_ids.contains(&initial_detail.header.id) {
            found_ids.push(initial_detail.header.id.clone());
        }

        let mut thread_messages = Vec::new();
        for mid in found_ids {
            if let Some(detail) = self.get_message_detail(&mid)? {
                thread_messages.push(detail);
            }
        }

        thread_messages.sort_by_key(|m| m.header.date_epoch);
        thread_messages.dedup_by(|a, b| a.header.id == b.header.id);

        let thread_id = initial_detail.header.message_id.unwrap_or_else(|| initial_detail.header.id.clone());

        Ok(Some(email_core::models::ConversationThread {
            thread_id,
            subject: root_subject,
            messages: thread_messages,
        }))
    }

    pub fn save_message_body(
        &self,
        message_id: &str,
        body_plain: Option<&str>,
        body_html: Option<&str>,
    ) -> Result<()> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        conn.execute(
            "UPDATE messages SET body_plain = ?1, body_html = ?2, body_fetched = 1 WHERE id = ?3",
            params![body_plain, body_html, message_id],
        )
        .map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn set_message_read(&self, message_id: &str, is_read: bool) -> Result<()> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        conn.execute(
            "UPDATE messages SET is_read = ?1 WHERE id = ?2",
            params![if is_read { 1 } else { 0 }, message_id],
        )
        .map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn set_message_flagged(&self, message_id: &str, is_flagged: bool) -> Result<()> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        conn.execute(
            "UPDATE messages SET is_flagged = ?1 WHERE id = ?2",
            params![if is_flagged { 1 } else { 0 }, message_id],
        )
        .map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn delete_message(&self, message_id: &str) -> Result<()> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        conn.execute(
            "UPDATE messages SET is_deleted = 1 WHERE id = ?1",
            params![message_id],
        )
        .map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn move_message_to_folder(&self, message_id: &str, target_folder_id: &str) -> Result<()> {
        let mut conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let tx = conn.transaction().map_err(|e| EmailError::Database(e.to_string()))?;
        {
            // 1. Get old folder_id and is_read status
            let info: Option<(String, bool)> = tx
                .query_row(
                    "SELECT folder_id, is_read FROM messages WHERE id = ?1",
                    params![message_id],
                    |row| Ok((row.get(0)?, row.get::<_, i32>(1)? != 0)),
                )
                .optional()
                .map_err(|e| EmailError::Database(e.to_string()))?;

            if let Some((old_folder_id, is_read)) = info {
                if old_folder_id != target_folder_id {
                    // 2. Safely update message folder_id and allocate a collision-free temporary UID
                    tx.execute(
                        r#"
                        UPDATE messages 
                        SET folder_id = ?1,
                            uid = (SELECT coalesce(max(uid), 0) + 1 FROM messages WHERE folder_id = ?1)
                        WHERE id = ?2
                        "#,
                        params![target_folder_id, message_id],
                    )
                    .map_err(|e| EmailError::Database(e.to_string()))?;

                    // 3. Update message counts on old folder
                    tx.execute(
                        r#"
                        UPDATE folders 
                        SET total_messages = max(0, total_messages - 1),
                            unread_messages = max(0, unread_messages - (CASE WHEN ?1 = 0 THEN 1 ELSE 0 END))
                        WHERE id = ?2
                        "#,
                        params![if is_read { 1 } else { 0 }, old_folder_id],
                    )
                    .map_err(|e| EmailError::Database(e.to_string()))?;

                    // 4. Update message counts on new folder
                    tx.execute(
                        r#"
                        UPDATE folders 
                        SET total_messages = total_messages + 1,
                            unread_messages = unread_messages + (CASE WHEN ?1 = 0 THEN 1 ELSE 0 END)
                        WHERE id = ?2
                        "#,
                        params![if is_read { 1 } else { 0 }, target_folder_id],
                    )
                    .map_err(|e| EmailError::Database(e.to_string()))?;
                }
            }
        }
        tx.commit().map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    // ==========================================
    // Attachments
    // ==========================================

    pub fn save_attachments(&self, attachments: &[Attachment]) -> Result<()> {
        let mut conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let tx = conn.transaction().map_err(|e| EmailError::Database(e.to_string()))?;
        {
            let mut stmt = tx.prepare(
                r#"
                INSERT INTO attachments (
                    id, message_id, filename, mime_type, size_bytes, content_id, is_inline, local_cache_path
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ON CONFLICT(id) DO NOTHING
                "#
            ).map_err(|e| EmailError::Database(e.to_string()))?;

            for a in attachments {
                stmt.execute(params![
                    a.id,
                    a.message_id,
                    a.filename,
                    a.mime_type,
                    a.size_bytes as i64,
                    a.content_id,
                    if a.is_inline { 1 } else { 0 },
                    a.local_cache_path,
                ]).map_err(|e| EmailError::Database(e.to_string()))?;
            }
        }
        tx.commit().map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn get_attachments_for_message(&self, message_id: &str) -> Result<Vec<Attachment>> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, message_id, filename, mime_type, size_bytes, content_id, is_inline, local_cache_path
                 FROM attachments WHERE message_id = ?1",
            )
            .map_err(|e| EmailError::Database(e.to_string()))?;

        let rows = stmt
            .query_map(params![message_id], |row| {
                let is_inline: i32 = row.get(6)?;
                let size_bytes: i64 = row.get(4)?;
                Ok(Attachment {
                    id: row.get(0)?,
                    message_id: row.get(1)?,
                    filename: row.get(2)?,
                    mime_type: row.get(3)?,
                    size_bytes: size_bytes as u64,
                    content_id: row.get(5)?,
                    is_inline: is_inline == 1,
                    local_cache_path: row.get(7)?,
                })
            })
            .map_err(|e| EmailError::Database(e.to_string()))?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r.map_err(|e| EmailError::Database(e.to_string()))?);
        }
        Ok(list)
    }

    // ==========================================
    // Templates & Signatures
    // ==========================================

    pub fn save_template(&self, template: &Template) -> Result<()> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        conn.execute(
            r#"
            INSERT INTO templates (id, name, subject_template, body_template, shortcut, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(id) DO UPDATE SET
                name=excluded.name,
                subject_template=excluded.subject_template,
                body_template=excluded.body_template,
                shortcut=excluded.shortcut
            "#,
            params![
                template.id,
                template.name,
                template.subject_template,
                template.body_template,
                template.shortcut,
                template.created_at,
            ],
        ).map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn get_templates(&self) -> Result<Vec<Template>> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT id, name, subject_template, body_template, shortcut, created_at FROM templates ORDER BY name ASC")
            .map_err(|e| EmailError::Database(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(Template {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    subject_template: row.get(2)?,
                    body_template: row.get(3)?,
                    shortcut: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })
            .map_err(|e| EmailError::Database(e.to_string()))?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r.map_err(|e| EmailError::Database(e.to_string()))?);
        }
        Ok(list)
    }

    pub fn delete_template(&self, id: &str) -> Result<()> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        conn.execute("DELETE FROM templates WHERE id = ?1", params![id])
            .map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn save_signature(&self, sig: &Signature) -> Result<()> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        conn.execute(
            r#"
            INSERT INTO signatures (id, account_id, name, content_html, is_default, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(id) DO UPDATE SET
                account_id=excluded.account_id,
                name=excluded.name,
                content_html=excluded.content_html,
                is_default=excluded.is_default
            "#,
            params![
                sig.id,
                sig.account_id,
                sig.name,
                sig.content_html,
                if sig.is_default { 1 } else { 0 },
                sig.created_at,
            ],
        ).map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn get_signatures(&self, account_id: Option<&str>) -> Result<Vec<Signature>> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT id, account_id, name, content_html, is_default, created_at FROM signatures ORDER BY name ASC")
            .map_err(|e| EmailError::Database(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                let is_default: i32 = row.get(4)?;
                Ok(Signature {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    name: row.get(2)?,
                    content_html: row.get(3)?,
                    is_default: is_default == 1,
                    created_at: row.get(5)?,
                })
            })
            .map_err(|e| EmailError::Database(e.to_string()))?;

        let mut list = Vec::new();
        for r in rows {
            let s = r.map_err(|e| EmailError::Database(e.to_string()))?;
            if account_id.is_none() || s.account_id.is_none() || s.account_id.as_deref() == account_id {
                list.push(s);
            }
        }
        Ok(list)
    }

    pub fn delete_signature(&self, id: &str) -> Result<()> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        conn.execute("DELETE FROM signatures WHERE id = ?1", params![id])
            .map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    // ==========================================
    // Local Drafts
    // ==========================================

    pub fn save_draft(&self, draft: &Draft) -> Result<()> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        conn.execute(
            r#"
            INSERT INTO drafts (
                id, account_id, to_input, cc_input, bcc_input, subject, body_plain,
                format, signature_id, in_reply_to, references_header, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            ON CONFLICT(id) DO UPDATE SET
                account_id=excluded.account_id,
                to_input=excluded.to_input,
                cc_input=excluded.cc_input,
                bcc_input=excluded.bcc_input,
                subject=excluded.subject,
                body_plain=excluded.body_plain,
                format=excluded.format,
                signature_id=excluded.signature_id,
                in_reply_to=excluded.in_reply_to,
                references_header=excluded.references_header,
                updated_at=excluded.updated_at
            "#,
            params![
                draft.id,
                draft.account_id,
                draft.to_input,
                draft.cc_input,
                draft.bcc_input,
                draft.subject,
                draft.body_plain,
                draft.format,
                draft.signature_id,
                draft.in_reply_to,
                draft.references,
                draft.updated_at,
            ],
        ).map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn get_draft(&self, id: &str) -> Result<Option<Draft>> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT id, account_id, to_input, cc_input, bcc_input, subject, body_plain, format, signature_id, in_reply_to, references_header, updated_at FROM drafts WHERE id = ?1")
            .map_err(|e| EmailError::Database(e.to_string()))?;

        let mut rows = stmt
            .query(params![id])
            .map_err(|e| EmailError::Database(e.to_string()))?;

        if let Some(row) = rows.next().map_err(|e| EmailError::Database(e.to_string()))? {
            Ok(Some(Draft {
                id: row.get(0).map_err(|e| EmailError::Database(e.to_string()))?,
                account_id: row.get(1).map_err(|e| EmailError::Database(e.to_string()))?,
                to_input: row.get(2).map_err(|e| EmailError::Database(e.to_string()))?,
                cc_input: row.get(3).map_err(|e| EmailError::Database(e.to_string()))?,
                bcc_input: row.get(4).map_err(|e| EmailError::Database(e.to_string()))?,
                subject: row.get(5).map_err(|e| EmailError::Database(e.to_string()))?,
                body_plain: row.get(6).map_err(|e| EmailError::Database(e.to_string()))?,
                format: row.get(7).map_err(|e| EmailError::Database(e.to_string()))?,
                signature_id: row.get(8).map_err(|e| EmailError::Database(e.to_string()))?,
                in_reply_to: row.get(9).map_err(|e| EmailError::Database(e.to_string()))?,
                references: row.get(10).map_err(|e| EmailError::Database(e.to_string()))?,
                updated_at: row.get(11).map_err(|e| EmailError::Database(e.to_string()))?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn list_drafts(&self, account_id: Option<&str>) -> Result<Vec<Draft>> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let sql = match account_id {
            Some(_) => "SELECT id, account_id, to_input, cc_input, bcc_input, subject, body_plain, format, signature_id, in_reply_to, references_header, updated_at FROM drafts WHERE account_id = ?1 ORDER BY updated_at DESC",
            None => "SELECT id, account_id, to_input, cc_input, bcc_input, subject, body_plain, format, signature_id, in_reply_to, references_header, updated_at FROM drafts ORDER BY updated_at DESC",
        };

        fn map_draft(row: &rusqlite::Row) -> rusqlite::Result<Draft> {
            Ok(Draft {
                id: row.get(0)?,
                account_id: row.get(1)?,
                to_input: row.get(2)?,
                cc_input: row.get(3)?,
                bcc_input: row.get(4)?,
                subject: row.get(5)?,
                body_plain: row.get(6)?,
                format: row.get(7)?,
                signature_id: row.get(8)?,
                in_reply_to: row.get(9)?,
                references: row.get(10)?,
                updated_at: row.get(11)?,
            })
        }

        let mut stmt = conn.prepare(sql).map_err(|e| EmailError::Database(e.to_string()))?;
        let rows = if let Some(aid) = account_id {
            stmt.query_map(params![aid], map_draft).map_err(|e| EmailError::Database(e.to_string()))?
        } else {
            stmt.query_map([], map_draft).map_err(|e| EmailError::Database(e.to_string()))?
        };

        let mut list = Vec::new();
        for r in rows {
            list.push(r.map_err(|e| EmailError::Database(e.to_string()))?);
        }
        Ok(list)
    }

    pub fn delete_draft(&self, id: &str) -> Result<()> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        conn.execute("DELETE FROM drafts WHERE id = ?1", params![id])
            .map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    // ==========================================
    // Scheduled Emails (Send Later)
    // ==========================================

    pub fn save_scheduled_email(&self, item: &ScheduledEmail) -> Result<()> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let draft_json = serde_json::to_string(&item.draft)
            .map_err(|e| EmailError::Database(format!("Serialization error: {}", e)))?;

        conn.execute(
            r#"
            INSERT INTO scheduled_emails (id, account_id, draft_json, send_at_timestamp, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(id) DO UPDATE SET
                account_id=excluded.account_id,
                draft_json=excluded.draft_json,
                send_at_timestamp=excluded.send_at_timestamp,
                created_at=excluded.created_at
            "#,
            params![
                item.id,
                item.account_id,
                draft_json,
                item.send_at_timestamp,
                item.created_at,
            ],
        ).map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn get_due_scheduled_emails(&self, now_timestamp: i64) -> Result<Vec<ScheduledEmail>> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT id, account_id, draft_json, send_at_timestamp, created_at FROM scheduled_emails WHERE send_at_timestamp <= ?1 ORDER BY send_at_timestamp ASC")
            .map_err(|e| EmailError::Database(e.to_string()))?;

        let rows = stmt
            .query_map(params![now_timestamp], |row| {
                let id: String = row.get(0)?;
                let account_id: String = row.get(1)?;
                let draft_json: String = row.get(2)?;
                let send_at_timestamp: i64 = row.get(3)?;
                let created_at: i64 = row.get(4)?;
                Ok((id, account_id, draft_json, send_at_timestamp, created_at))
            })
            .map_err(|e| EmailError::Database(e.to_string()))?;

        let mut list = Vec::new();
        for r in rows {
            let (id, account_id, draft_json, send_at_timestamp, created_at) =
                r.map_err(|e| EmailError::Database(e.to_string()))?;
            if let Ok(draft) = serde_json::from_str::<OutgoingDraft>(&draft_json) {
                list.push(ScheduledEmail {
                    id,
                    account_id,
                    draft,
                    send_at_timestamp,
                    created_at,
                });
            }
        }
        Ok(list)
    }

    pub fn list_all_scheduled(&self, account_id: Option<&str>) -> Result<Vec<ScheduledEmail>> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let sql = match account_id {
            Some(_) => "SELECT id, account_id, draft_json, send_at_timestamp, created_at FROM scheduled_emails WHERE account_id = ?1 ORDER BY send_at_timestamp ASC",
            None => "SELECT id, account_id, draft_json, send_at_timestamp, created_at FROM scheduled_emails ORDER BY send_at_timestamp ASC",
        };

        fn map_scheduled(row: &rusqlite::Row) -> rusqlite::Result<(String, String, String, i64, i64)> {
            let id: String = row.get(0)?;
            let account_id: String = row.get(1)?;
            let draft_json: String = row.get(2)?;
            let send_at_timestamp: i64 = row.get(3)?;
            let created_at: i64 = row.get(4)?;
            Ok((id, account_id, draft_json, send_at_timestamp, created_at))
        }

        let mut stmt = conn.prepare(sql).map_err(|e| EmailError::Database(e.to_string()))?;
        let rows = if let Some(aid) = account_id {
            stmt.query_map(params![aid], map_scheduled).map_err(|e| EmailError::Database(e.to_string()))?
        } else {
            stmt.query_map([], map_scheduled).map_err(|e| EmailError::Database(e.to_string()))?
        };

        let mut list = Vec::new();
        for r in rows {
            let (id, account_id, draft_json, send_at_timestamp, created_at) =
                r.map_err(|e| EmailError::Database(e.to_string()))?;
            if let Ok(draft) = serde_json::from_str::<OutgoingDraft>(&draft_json) {
                list.push(ScheduledEmail {
                    id,
                    account_id,
                    draft,
                    send_at_timestamp,
                    created_at,
                });
            }
        }
        Ok(list)
    }

    // --- PGP Key Management ---

    pub fn save_pgp_keypair(&self, keypair: &email_core::pgp::PgpKeypair) -> Result<()> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        conn.execute(
            r#"
            INSERT INTO pgp_keys (email, fingerprint, public_key_armored, private_key_armored, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(email) DO UPDATE SET
                fingerprint = excluded.fingerprint,
                public_key_armored = excluded.public_key_armored,
                private_key_armored = excluded.private_key_armored,
                created_at = excluded.created_at
            "#,
            params![
                keypair.email,
                keypair.fingerprint,
                keypair.public_key_armored,
                keypair.private_key_armored,
                keypair.created_at,
            ],
        )
        .map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn get_pgp_key(&self, email: &str) -> Result<Option<email_core::pgp::PgpKeypair>> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT email, fingerprint, public_key_armored, private_key_armored, created_at FROM pgp_keys WHERE email = ?1")
            .map_err(|e| EmailError::Database(e.to_string()))?;

        let result = stmt.query_row(params![email], |row| {
            Ok(email_core::pgp::PgpKeypair {
                email: row.get(0)?,
                fingerprint: row.get(1)?,
                public_key_armored: row.get(2)?,
                private_key_armored: row.get(3)?,
                created_at: row.get(4)?,
            })
        });

        match result {
            Ok(kp) => Ok(Some(kp)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(EmailError::Database(e.to_string())),
        }
    }

    pub fn get_all_pgp_keys(&self) -> Result<Vec<email_core::pgp::PgpKeypair>> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT email, fingerprint, public_key_armored, private_key_armored, created_at FROM pgp_keys ORDER BY email ASC")
            .map_err(|e| EmailError::Database(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(email_core::pgp::PgpKeypair {
                    email: row.get(0)?,
                    fingerprint: row.get(1)?,
                    public_key_armored: row.get(2)?,
                    private_key_armored: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })
            .map_err(|e| EmailError::Database(e.to_string()))?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r.map_err(|e| EmailError::Database(e.to_string()))?);
        }
        Ok(list)
    }

    pub fn delete_pgp_key(&self, email: &str) -> Result<()> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        conn.execute("DELETE FROM pgp_keys WHERE email = ?1", params![email])
            .map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn delete_scheduled_email(&self, id: &str) -> Result<()> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        conn.execute("DELETE FROM scheduled_emails WHERE id = ?1", params![id])
            .map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    // --- Outbox Auto-Retry Queue Management ---

    pub fn save_outbox_item(&self, item: &email_core::models::OutboxItem) -> Result<()> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let draft_json = serde_json::to_string(&item.draft)
            .map_err(|e| EmailError::Database(format!("Draft JSON serialization failed: {}", e)))?;

        conn.execute(
            r#"
            INSERT INTO outbox (id, account_id, draft_json, retry_count, max_retries, next_retry_timestamp, last_error, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(id) DO UPDATE SET
                retry_count = excluded.retry_count,
                next_retry_timestamp = excluded.next_retry_timestamp,
                last_error = excluded.last_error
            "#,
            params![
                item.id,
                item.account_id,
                draft_json,
                item.retry_count,
                item.max_retries,
                item.next_retry_timestamp,
                item.last_error,
                item.created_at,
            ],
        )
        .map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn get_due_outbox_items(&self) -> Result<Vec<email_core::models::OutboxItem>> {
        let now = chrono::Utc::now().timestamp();
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, account_id, draft_json, retry_count, max_retries, next_retry_timestamp, last_error, created_at
                 FROM outbox
                 WHERE next_retry_timestamp <= ?1 AND retry_count < max_retries
                 ORDER BY next_retry_timestamp ASC",
            )
            .map_err(|e| EmailError::Database(e.to_string()))?;

        let rows = stmt
            .query_map(params![now], |row| {
                let id: String = row.get(0)?;
                let account_id: String = row.get(1)?;
                let draft_json: String = row.get(2)?;
                let retry_count: u32 = row.get(3)?;
                let max_retries: u32 = row.get(4)?;
                let next_retry_timestamp: i64 = row.get(5)?;
                let last_error: Option<String> = row.get(6)?;
                let created_at: i64 = row.get(7)?;
                Ok((id, account_id, draft_json, retry_count, max_retries, next_retry_timestamp, last_error, created_at))
            })
            .map_err(|e| EmailError::Database(e.to_string()))?;

        let mut list = Vec::new();
        for r in rows {
            let (id, account_id, draft_json, retry_count, max_retries, next_retry_timestamp, last_error, created_at) =
                r.map_err(|e| EmailError::Database(e.to_string()))?;
            if let Ok(draft) = serde_json::from_str::<email_core::models::OutgoingDraft>(&draft_json) {
                list.push(email_core::models::OutboxItem {
                    id,
                    account_id,
                    draft,
                    retry_count,
                    max_retries,
                    next_retry_timestamp,
                    last_error,
                    created_at,
                });
            }
        }
        Ok(list)
    }

    pub fn get_all_outbox_items(&self, account_id: Option<&str>) -> Result<Vec<email_core::models::OutboxItem>> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let sql = match account_id {
            Some(_) => "SELECT id, account_id, draft_json, retry_count, max_retries, next_retry_timestamp, last_error, created_at FROM outbox WHERE account_id = ?1 ORDER BY created_at DESC",
            None => "SELECT id, account_id, draft_json, retry_count, max_retries, next_retry_timestamp, last_error, created_at FROM outbox ORDER BY created_at DESC",
        };

        fn map_outbox_row(row: &rusqlite::Row) -> rusqlite::Result<(String, String, String, u32, u32, i64, Option<String>, i64)> {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
            ))
        }

        let mut stmt = conn.prepare(sql).map_err(|e| EmailError::Database(e.to_string()))?;
        let rows = if let Some(aid) = account_id {
            stmt.query_map(params![aid], map_outbox_row).map_err(|e| EmailError::Database(e.to_string()))?
        } else {
            stmt.query_map([], map_outbox_row).map_err(|e| EmailError::Database(e.to_string()))?
        };

        let mut list = Vec::new();
        for r in rows {
            let (id, account_id, draft_json, retry_count, max_retries, next_retry_timestamp, last_error, created_at) =
                r.map_err(|e| EmailError::Database(e.to_string()))?;
            if let Ok(draft) = serde_json::from_str::<email_core::models::OutgoingDraft>(&draft_json) {
                list.push(email_core::models::OutboxItem {
                    id,
                    account_id,
                    draft,
                    retry_count,
                    max_retries,
                    next_retry_timestamp,
                    last_error,
                    created_at,
                });
            }
        }
        Ok(list)
    }

    pub fn delete_outbox_item(&self, id: &str) -> Result<()> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        conn.execute("DELETE FROM outbox WHERE id = ?1", params![id])
            .map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn record_outbox_failure(&self, id: &str, error_msg: &str) -> Result<Option<email_core::models::OutboxItem>> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT id, account_id, draft_json, retry_count, max_retries, next_retry_timestamp, last_error, created_at FROM outbox WHERE id = ?1")
            .map_err(|e| EmailError::Database(e.to_string()))?;

        let item = stmt.query_row(params![id], |row| {
            let id: String = row.get(0)?;
            let account_id: String = row.get(1)?;
            let draft_json: String = row.get(2)?;
            let retry_count: u32 = row.get(3)?;
            let max_retries: u32 = row.get(4)?;
            let next_retry_timestamp: i64 = row.get(5)?;
            let last_error: Option<String> = row.get(6)?;
            let created_at: i64 = row.get(7)?;
            Ok((id, account_id, draft_json, retry_count, max_retries, next_retry_timestamp, last_error, created_at))
        });

        match item {
            Ok((id, account_id, draft_json, retry_count, max_retries, _old_next_retry, _old_error, created_at)) => {
                let new_retry_count = retry_count + 1;
                let backoff_secs = email_core::models::OutboxItem::calculate_backoff_seconds(new_retry_count);
                let next_retry_timestamp = chrono::Utc::now().timestamp() + backoff_secs;

                conn.execute(
                    "UPDATE outbox SET retry_count = ?1, next_retry_timestamp = ?2, last_error = ?3 WHERE id = ?4",
                    params![new_retry_count, next_retry_timestamp, error_msg, id],
                )
                .map_err(|e| EmailError::Database(e.to_string()))?;

                if let Ok(draft) = serde_json::from_str::<email_core::models::OutgoingDraft>(&draft_json) {
                    Ok(Some(email_core::models::OutboxItem {
                        id,
                        account_id,
                        draft,
                        retry_count: new_retry_count,
                        max_retries,
                        next_retry_timestamp,
                        last_error: Some(error_msg.to_string()),
                        created_at,
                    }))
                } else {
                    Ok(None)
                }
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(EmailError::Database(e.to_string())),
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = ?2",
            params![key, value],
        )
        .map_err(|e| EmailError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT value FROM settings WHERE key = ?1")
            .map_err(|e| EmailError::Database(e.to_string()))?;
        let mut rows = stmt
            .query(params![key])
            .map_err(|e| EmailError::Database(e.to_string()))?;
        if let Some(row) = rows.next().map_err(|e| EmailError::Database(e.to_string()))? {
            let val: String = row.get(0).map_err(|e| EmailError::Database(e.to_string()))?;
            Ok(Some(val))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_crud_flow() {
        let storage = Storage::new_in_memory().expect("Failed to create in-memory storage");

        // 1. Account CRUD
        let account = Account::new(
            "Test User".to_string(),
            "test@example.com".to_string(),
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

        let accounts = storage.get_accounts().unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].email, "test@example.com");

        // 2. Folders CRUD & selective sync toggle
        let folder1 = Folder::new(
            account.id.clone(),
            "INBOX".to_string(),
            "Inbox".to_string(),
            "/".to_string(),
            vec!["\\Inbox".to_string()],
            true,
        );
        let folder2 = Folder::new(
            account.id.clone(),
            "Archive".to_string(),
            "Archive".to_string(),
            "/".to_string(),
            vec![],
            false,
        );

        storage.save_folders(&[folder1.clone(), folder2.clone()]).unwrap();
        let folders = storage.get_folders_for_account(&account.id).unwrap();
        assert_eq!(folders.len(), 2);

        storage.set_folder_sync_enabled(&folder2.id, true).unwrap();
        let updated_folders = storage.get_folders_for_account(&account.id).unwrap();
        let updated_f2 = updated_folders.iter().find(|f| f.id == folder2.id).unwrap();
        assert!(updated_f2.is_synced);

        // 3. Message headers batch insert & pagination
        let header = MessageHeader {
            id: "msg-123".to_string(),
            account_id: account.id.clone(),
            folder_id: folder1.id.clone(),
            uid: 101,
            message_id: Some("<msg123@example.com>".to_string()),
            in_reply_to: None,
            subject: "Quarterly Review Meeting".to_string(),
            from_name: Some("Boss".to_string()),
            from_address: "boss@example.com".to_string(),
            to_recipients: vec![Recipient::new(Some("Test User".to_string()), "test@example.com".to_string())],
            cc_recipients: vec![],
            date_epoch: 1725000000,
            snippet: "Let's review the Q3 targets...".to_string(),
            is_read: false,
            is_flagged: true,
            is_draft: false,
            is_deleted: false,
            body_fetched: false,
            size_bytes: 2048,
            snooze_until: None,
        };
        storage.save_message_headers(&[header.clone()]).unwrap();

        let msgs = storage.get_messages(Some(&account.id), Some(&folder1.id), 10, 0, None).unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].subject, "Quarterly Review Meeting");
        assert!(!msgs[0].is_read);
        assert!(msgs[0].is_flagged);

        // 4. Search query
        let search_res = storage.get_messages(None, None, 10, 0, Some("Quarterly")).unwrap();
        assert_eq!(search_res.len(), 1);

        // 5. Lazy body save & detail retrieval
        storage.save_message_body("msg-123", Some("Let's review the Q3 targets in room 4."), Some("<p>Let's review the <b>Q3 targets</b> in room 4.</p>")).unwrap();
        let detail = storage.get_message_detail("msg-123").unwrap().expect("Message detail not found");
        assert!(detail.header.body_fetched);
        assert_eq!(detail.body_plain.as_deref(), Some("Let's review the Q3 targets in room 4."));

        // 6. Templates and signatures
        let tpl = Template::new("Greeting".to_string(), "Hello".to_string(), "Hi there!".to_string(), Some("/hi".to_string()));
        storage.save_template(&tpl).unwrap();
        let tpls = storage.get_templates().unwrap();
        assert_eq!(tpls.len(), 1);
        assert_eq!(tpls[0].shortcut.as_deref(), Some("/hi"));

        let sig = Signature::new(Some(account.id.clone()), "Work Sig".to_string(), "<p>Best,</p>".to_string(), true);
        storage.save_signature(&sig).unwrap();
        let sigs = storage.get_signatures(Some(&account.id)).unwrap();
        assert_eq!(sigs.len(), 1);
        assert!(sigs[0].is_default);

        // 7. Test save_full_messages (Offline Ready full sync)
        let header2 = MessageHeader {
            id: "msg-456".to_string(),
            account_id: account.id.clone(),
            folder_id: folder1.id.clone(),
            uid: 102,
            message_id: Some("<msg456@example.com>".to_string()),
            in_reply_to: None,
            subject: "Offline Email Subject".to_string(),
            from_name: Some("Sender Name".to_string()),
            from_address: "sender@example.com".to_string(),
            to_recipients: vec![Recipient::new(Some("Test User".to_string()), "test@example.com".to_string())],
            cc_recipients: vec![],
            date_epoch: 1725000050,
            snippet: "Offline email body content...".to_string(),
            is_read: false,
            is_flagged: false,
            is_draft: false,
            is_deleted: false,
            body_fetched: true,
            size_bytes: 4096,
            snooze_until: None,
        };
        let att2 = Attachment {
            id: "att-1".to_string(),
            message_id: "msg-456".to_string(),
            filename: "invoice.pdf".to_string(),
            mime_type: "application/pdf".to_string(),
            size_bytes: 1024,
            content_id: None,
            is_inline: false,
            local_cache_path: Some("/tmp/invoice.pdf".to_string()),
        };

        storage.save_full_messages(&[(
            header2.clone(),
            Some("Offline email body content plain".to_string()),
            Some("<p>Offline email body content html</p>".to_string()),
            vec![att2],
        )]).unwrap();

        let cached_map = storage.get_folder_cached_uids(&folder1.id).unwrap();
        assert_eq!(cached_map.get(&102), Some(&true));

        let detail2 = storage.get_message_detail("msg-456").unwrap().expect("Detail 2 not found");
        assert!(detail2.header.body_fetched);
        assert_eq!(detail2.body_html.as_deref(), Some("<p>Offline email body content html</p>"));
        assert_eq!(detail2.attachments.len(), 1);
        assert_eq!(detail2.attachments[0].filename, "invoice.pdf");

        // 8. Test update_message_flags_batch
        storage.update_message_flags_batch(&folder1.id, &[(102, true, true, false)]).unwrap();
        let msgs_updated = storage.get_messages(Some(&account.id), Some(&folder1.id), 10, 0, None).unwrap();
        let updated_msg = msgs_updated.iter().find(|m| m.uid == 102).unwrap();
        assert!(updated_msg.is_read);
        assert!(updated_msg.is_flagged);

        // 9. Test move_message_to_folder
        storage.move_message_to_folder("msg-456", &folder2.id).unwrap();
        let msgs_folder1 = storage.get_messages(Some(&account.id), Some(&folder1.id), 10, 0, None).unwrap();
        assert!(!msgs_folder1.iter().any(|m| m.id == "msg-456"));
        let msgs_folder2 = storage.get_messages(Some(&account.id), Some(&folder2.id), 10, 0, None).unwrap();
        assert!(msgs_folder2.iter().any(|m| m.id == "msg-456"));

        // 10. Test settings storage
        assert_eq!(storage.get_setting("theme_preset").unwrap(), None);
        storage.set_setting("theme_preset", "catppuccin_mocha").unwrap();
        assert_eq!(storage.get_setting("theme_preset").unwrap().as_deref(), Some("catppuccin_mocha"));
        storage.set_setting("theme_preset", "system_auto").unwrap();
        assert_eq!(storage.get_setting("theme_preset").unwrap().as_deref(), Some("system_auto"));
    }

    #[test]
    fn test_parse_search_query_tokens() {
        let q = parse_search_query("from:alice@example.com to:team subject:launch is:unread is:starred has:attachment important update");
        assert_eq!(q.from, vec!["alice@example.com"]);
        assert_eq!(q.to, vec!["team"]);
        assert_eq!(q.subject, vec!["launch"]);
        assert_eq!(q.is_unread, Some(true));
        assert_eq!(q.is_flagged, Some(true));
        assert!(q.has_attachment);
        assert_eq!(q.free_text, vec!["important", "update"]);
    }

    #[test]
    fn test_drafts_crud_flow() {
        let storage = Storage::new_in_memory().unwrap();
        let account = Account::new(
            "Draft Tester".to_string(),
            "tester@draft.com".to_string(),
            "imap.draft.com".to_string(),
            993,
            SecurityType::Tls,
            "smtp.draft.com".to_string(),
            465,
            SecurityType::Tls,
            AuthType::Password,
            SyncWindow::Days30,
        );
        storage.save_account(&account).unwrap();

        let mut draft = Draft::new(
            account.id.clone(),
            "recipient@example.com".to_string(),
            "cc@example.com".to_string(),
            "".to_string(),
            "Work Draft Subject".to_string(),
            "Initial draft body...".to_string(),
            "markdown".to_string(),
            None,
            None,
            None,
        );

        storage.save_draft(&draft).unwrap();

        let list = storage.list_drafts(Some(&account.id)).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].subject, "Work Draft Subject");

        let fetched = storage.get_draft(&draft.id).unwrap().expect("Found draft");
        assert_eq!(fetched.body_plain, "Initial draft body...");

        draft.body_plain = "Updated draft body content".to_string();
        storage.save_draft(&draft).unwrap();

        let updated = storage.get_draft(&draft.id).unwrap().unwrap();
        assert_eq!(updated.body_plain, "Updated draft body content");

        storage.delete_draft(&draft.id).unwrap();
        assert!(storage.get_draft(&draft.id).unwrap().is_none());
    }

    #[test]
    fn test_scheduled_emails_flow() {
        let storage = Storage::new_in_memory().unwrap();
        let account = Account::new(
            "Scheduled Tester".to_string(),
            "tester@scheduled.com".to_string(),
            "imap.scheduled.com".to_string(),
            993,
            SecurityType::Tls,
            "smtp.scheduled.com".to_string(),
            465,
            SecurityType::Tls,
            AuthType::Password,
            SyncWindow::Days30,
        );
        storage.save_account(&account).unwrap();

        let now = chrono::Utc::now().timestamp();
        let outgoing = OutgoingDraft {
            account_id: account.id.clone(),
            to: vec![Recipient::new(None, "dest@example.com".to_string())],
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: "Scheduled Launch".to_string(),
            body_plain: "Rocket is fueled!".to_string(),
            body_html: None,
            in_reply_to: None,
            references: None,
            attachments: Vec::new(),
        };

        let scheduled_past = ScheduledEmail::new(account.id.clone(), outgoing.clone(), now - 30);
        let scheduled_future = ScheduledEmail::new(account.id.clone(), outgoing, now + 3600);

        storage.save_scheduled_email(&scheduled_past).unwrap();
        storage.save_scheduled_email(&scheduled_future).unwrap();

        let all = storage.list_all_scheduled(Some(&account.id)).unwrap();
        assert_eq!(all.len(), 2);

        let due = storage.get_due_scheduled_emails(now).unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].id, scheduled_past.id);

        storage.delete_scheduled_email(&scheduled_past.id).unwrap();
        let remaining = storage.list_all_scheduled(None).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, scheduled_future.id);
    }

    #[test]
    fn test_fts5_full_text_search_and_ranking() {
        let storage = Storage::new_in_memory().unwrap();
        let account = Account::new(
            "FTS Search Test".to_string(),
            "fts@example.com".to_string(),
            "imap.example.com".to_string(),
            993,
            SecurityType::Tls,
            "smtp.example.com".to_string(),
            587,
            SecurityType::StartTls,
            AuthType::Password,
            SyncWindow::Days30,
        );
        storage.save_account(&account).unwrap();

        let folder = Folder::new(
            account.id.clone(),
            "INBOX".to_string(),
            "Inbox".to_string(),
            "/".to_string(),
            vec!["\\Inbox".to_string()],
            true,
        );
        storage.save_folders(&[folder.clone()]).unwrap();

        let msg1 = MessageHeader {
            id: "msg_fts_1".to_string(),
            account_id: account.id.clone(),
            folder_id: folder.id.clone(),
            uid: 101,
            message_id: Some("msg1@fts.com".to_string()),
            in_reply_to: None,
            subject: "Quarterly Financial Analysis Report".to_string(),
            from_name: Some("Finance Lead".to_string()),
            from_address: "finance@company.com".to_string(),
            to_recipients: vec![Recipient::new(Some("Kunal".to_string()), "kunal@company.com".to_string())],
            cc_recipients: Vec::new(),
            date_epoch: 1700000000,
            snippet: "Here is the revenue metrics summary...".to_string(),
            is_read: true,
            is_flagged: false,
            is_draft: false,
            is_deleted: false,
            body_fetched: true,
            size_bytes: 4096,
            snooze_until: None,
        };

        let msg2 = MessageHeader {
            id: "msg_fts_2".to_string(),
            account_id: account.id.clone(),
            folder_id: folder.id.clone(),
            uid: 102,
            message_id: Some("msg2@fts.com".to_string()),
            in_reply_to: None,
            subject: "Team Lunch and Offsite Planning".to_string(),
            from_name: Some("Alice HR".to_string()),
            from_address: "alice@company.com".to_string(),
            to_recipients: vec![Recipient::new(Some("Kunal".to_string()), "kunal@company.com".to_string())],
            cc_recipients: Vec::new(),
            date_epoch: 1700000100,
            snippet: "Let us coordinate the upcoming offsite venue...".to_string(),
            is_read: false,
            is_flagged: true,
            is_draft: false,
            is_deleted: false,
            body_fetched: true,
            size_bytes: 2048,
            snooze_until: None,
        };

        storage.save_message_headers(&[msg1.clone(), msg2.clone()]).unwrap();
        storage.save_message_body(&msg1.id, Some("Comprehensive financial analysis breakdown with Q3 revenue"), None).unwrap();
        storage.save_message_body(&msg2.id, Some("Offsite event logistics and catering options"), None).unwrap();

        // 1. Search by subject keyword
        let res1 = storage.search_messages_fts(Some(&account.id), None, "Financial", 10, 0).unwrap();
        assert_eq!(res1.len(), 1);
        assert_eq!(res1[0].id, "msg_fts_1");

        // 2. Search by body text keyword (indexed in messages_fts)
        let res2 = storage.search_messages_fts(Some(&account.id), None, "logistics", 10, 0).unwrap();
        assert_eq!(res2.len(), 1);
        assert_eq!(res2[0].id, "msg_fts_2");

        // 3. Search with from: filter
        let res3 = storage.search_messages_fts(Some(&account.id), None, "from:alice", 10, 0).unwrap();
        assert_eq!(res3.len(), 1);
        assert_eq!(res3[0].id, "msg_fts_2");

        // 4. Search with is:unread
        let res4 = storage.search_messages_fts(Some(&account.id), None, "is:unread", 10, 0).unwrap();
        assert_eq!(res4.len(), 1);
        assert_eq!(res4[0].id, "msg_fts_2");
    }

    #[test]
    fn test_snooze_and_unsnooze_flow() {
        let storage = Storage::new_in_memory().unwrap();
        let account = Account::new(
            "Snooze Test".to_string(),
            "snooze@example.com".to_string(),
            "imap.example.com".to_string(),
            993,
            SecurityType::Tls,
            "smtp.example.com".to_string(),
            587,
            SecurityType::StartTls,
            AuthType::Password,
            SyncWindow::Days30,
        );
        storage.save_account(&account).unwrap();

        let folder = Folder::new(
            account.id.clone(),
            "INBOX".to_string(),
            "Inbox".to_string(),
            "/".to_string(),
            vec!["\\Inbox".to_string()],
            true,
        );
        storage.save_folders(&[folder.clone()]).unwrap();

        let msg = MessageHeader {
            id: "msg_snooze_1".to_string(),
            account_id: account.id.clone(),
            folder_id: folder.id.clone(),
            uid: 201,
            message_id: Some("snooze1@example.com".to_string()),
            in_reply_to: None,
            subject: "Reminder: Review Security Architecture".to_string(),
            from_name: Some("Security Officer".to_string()),
            from_address: "sec@example.com".to_string(),
            to_recipients: vec![Recipient::new(Some("Kunal".to_string()), "kunal@example.com".to_string())],
            cc_recipients: Vec::new(),
            date_epoch: 1700000000,
            snippet: "Please audit the firewall configuration.".to_string(),
            is_read: false,
            is_flagged: false,
            is_draft: false,
            is_deleted: false,
            body_fetched: true,
            size_bytes: 1024,
            snooze_until: None,
        };

        storage.save_message_headers(&[msg.clone()]).unwrap();

        // Initially in standard get_messages
        let initial = storage.get_messages(Some(&account.id), None, 10, 0, None).unwrap();
        assert_eq!(initial.len(), 1);

        // Snooze until 1 hour in the future
        let future_time = chrono::Utc::now().timestamp() + 3600;
        storage.snooze_message(&msg.id, Some(future_time)).unwrap();

        // Should now be hidden from standard inbox query
        let active = storage.get_messages(Some(&account.id), None, 10, 0, None).unwrap();
        assert_eq!(active.len(), 0);

        // Should appear in get_snoozed_messages
        let snoozed = storage.get_snoozed_messages(Some(&account.id)).unwrap();
        assert_eq!(snoozed.len(), 1);
        assert_eq!(snoozed[0].id, "msg_snooze_1");

        // Unsnooze
        storage.unsnooze_message(&msg.id).unwrap();
        let restored = storage.get_messages(Some(&account.id), None, 10, 0, None).unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].id, "msg_snooze_1");
    }

    #[test]
    fn test_conversation_thread_resolution() {
        let storage = Storage::new_in_memory().unwrap();
        let account = Account::new(
            "Thread Test".to_string(),
            "thread@example.com".to_string(),
            "imap.example.com".to_string(),
            993,
            SecurityType::Tls,
            "smtp.example.com".to_string(),
            587,
            SecurityType::StartTls,
            AuthType::Password,
            SyncWindow::Days30,
        );
        storage.save_account(&account).unwrap();

        let folder = Folder::new(
            account.id.clone(),
            "INBOX".to_string(),
            "Inbox".to_string(),
            "/".to_string(),
            vec!["\\Inbox".to_string()],
            true,
        );
        storage.save_folders(&[folder.clone()]).unwrap();

        let m1 = MessageHeader {
            id: "thread_msg_1".to_string(),
            account_id: account.id.clone(),
            folder_id: folder.id.clone(),
            uid: 301,
            message_id: Some("<root_thread_001@example.com>".to_string()),
            in_reply_to: None,
            subject: "API Design Proposal for 2.0".to_string(),
            from_name: Some("Lead Architect".to_string()),
            from_address: "arch@example.com".to_string(),
            to_recipients: vec![Recipient::new(Some("Team".to_string()), "team@example.com".to_string())],
            cc_recipients: Vec::new(),
            date_epoch: 1700000000,
            snippet: "Here is the RFC draft for 2.0...".to_string(),
            is_read: true,
            is_flagged: false,
            is_draft: false,
            is_deleted: false,
            body_fetched: true,
            size_bytes: 1024,
            snooze_until: None,
        };

        let m2 = MessageHeader {
            id: "thread_msg_2".to_string(),
            account_id: account.id.clone(),
            folder_id: folder.id.clone(),
            uid: 302,
            message_id: Some("<reply_thread_002@example.com>".to_string()),
            in_reply_to: Some("<root_thread_001@example.com>".to_string()),
            subject: "Re: API Design Proposal for 2.0".to_string(),
            from_name: Some("Kunal".to_string()),
            from_address: "kunal@example.com".to_string(),
            to_recipients: vec![Recipient::new(Some("Lead Architect".to_string()), "arch@example.com".to_string())],
            cc_recipients: Vec::new(),
            date_epoch: 1700000200,
            snippet: "Looks solid, let's verify error status codes.".to_string(),
            is_read: true,
            is_flagged: false,
            is_draft: false,
            is_deleted: false,
            body_fetched: true,
            size_bytes: 512,
            snooze_until: None,
        };

        storage.save_message_headers(&[m1.clone(), m2.clone()]).unwrap();
        storage.save_message_body(&m1.id, Some("RFC content details for proposed endpoints."), None).unwrap();
        storage.save_message_body(&m2.id, Some("I agree with endpoint design."), None).unwrap();

        // Resolving from child reply
        let thread = storage.get_conversation_thread(&m2.id).unwrap().expect("Thread resolved");
        assert_eq!(thread.subject, "API Design Proposal for 2.0");
        assert_eq!(thread.messages.len(), 2);
        assert_eq!(thread.messages[0].header.id, "thread_msg_1");
        assert_eq!(thread.messages[1].header.id, "thread_msg_2");
    }

    #[test]
    fn test_pgp_key_storage_crud() {
        let storage = Storage::new_in_memory().unwrap();
        let kp = email_core::pgp::generate_pgp_keypair("security@abhashtech.com").unwrap();

        // 1. Save Keypair
        storage.save_pgp_keypair(&kp).unwrap();

        // 2. Query Key
        let fetched = storage.get_pgp_key("security@abhashtech.com").unwrap().expect("Key found");
        assert_eq!(fetched.email, "security@abhashtech.com");
        assert_eq!(fetched.fingerprint, kp.fingerprint);
        assert_eq!(fetched.public_key_armored, kp.public_key_armored);

        // 3. List All Keys
        let all_keys = storage.get_all_pgp_keys().unwrap();
        assert_eq!(all_keys.len(), 1);

        // 4. Delete Key
        storage.delete_pgp_key("security@abhashtech.com").unwrap();
        let deleted = storage.get_pgp_key("security@abhashtech.com").unwrap();
        assert!(deleted.is_none());
    }

    #[test]
    fn test_outbox_queue_crud_and_retry_flow() {
        let storage = Storage::new_in_memory().unwrap();

        let account = Account::new(
            "outbox_user".to_string(),
            "outbox@example.com".to_string(),
            "imap.example.com".to_string(),
            993,
            SecurityType::Tls,
            "smtp.example.com".to_string(),
            587,
            SecurityType::StartTls,
            AuthType::Password,
            SyncWindow::Days30,
        );
        storage.save_account(&account).unwrap();

        let draft = OutgoingDraft {
            account_id: account.id.clone(),
            to: vec![Recipient::new(Some("Boss".to_string()), "boss@example.com".to_string())],
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: "Urgent status report".to_string(),
            body_plain: "Network is temporarily down, sending via retry queue.".to_string(),
            body_html: None,
            in_reply_to: None,
            references: None,
            attachments: Vec::new(),
        };

        let mut item = OutboxItem::new(account.id.clone(), draft);
        // Force timestamp in past so it's due immediately
        item.next_retry_timestamp = chrono::Utc::now().timestamp() - 10;

        // 1. Save outbox item
        storage.save_outbox_item(&item).unwrap();

        // 2. Query due items
        let due = storage.get_due_outbox_items().unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].id, item.id);
        assert_eq!(due[0].retry_count, 0);

        // 3. Record failure with exponential backoff
        let updated = storage.record_outbox_failure(&item.id, "SMTP connection timed out").unwrap().expect("Item exists");
        assert_eq!(updated.retry_count, 1);
        assert_eq!(updated.last_error.as_deref(), Some("SMTP connection timed out"));
        assert!(updated.next_retry_timestamp > chrono::Utc::now().timestamp());

        // 4. List all outbox items
        let all = storage.get_all_outbox_items(None).unwrap();
        assert_eq!(all.len(), 1);

        // 5. Delete outbox item on successful delivery
        storage.delete_outbox_item(&item.id).unwrap();
        let all_after = storage.get_all_outbox_items(None).unwrap();
        assert!(all_after.is_empty());
    }

    #[test]
    fn test_database_auto_migration_from_legacy_schema() {
        let db_path = std::env::temp_dir().join(format!("legacy_test_{}.db", uuid::Uuid::new_v4()));

        // 1. Create a legacy database manually without snooze_until column
        {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            conn.execute_batch(r#"
                CREATE TABLE accounts (
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
                    is_enabled INTEGER NOT NULL DEFAULT 1,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );
                CREATE TABLE folders (
                    id TEXT PRIMARY KEY,
                    account_id TEXT NOT NULL,
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
                CREATE TABLE messages (
                    id TEXT PRIMARY KEY,
                    account_id TEXT NOT NULL,
                    folder_id TEXT NOT NULL,
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
                    UNIQUE(folder_id, uid)
                );
            "#).unwrap();
        }

        // 2. Open with Storage::new — should run automatic migration without error!
        let storage = Storage::new(&db_path).expect("Automatic migration succeeded");
        let accounts = storage.get_accounts().unwrap();
        assert!(accounts.is_empty());
    }
}


