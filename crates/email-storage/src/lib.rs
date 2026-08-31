pub mod schema;

use email_core::error::{EmailError, Result};
use email_core::models::*;
use log::info;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone)]
pub struct Storage {
    pool: Pool<SqliteConnectionManager>,
}

impl Storage {
    pub fn new_in_memory() -> Result<Self> {
        let manager = SqliteConnectionManager::memory();
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
        let manager = SqliteConnectionManager::file(path.as_ref());
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
        conn.execute_batch(schema::SCHEMA_V1)
            .map_err(|e| EmailError::Database(format!("Schema init error: {}", e)))?;
        info!("SQLite database initialized successfully with WAL mode.");
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
                    body_fetched, size_bytes
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
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
                    size_bytes=excluded.size_bytes
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
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;

        let mut query = String::from(
            "SELECT id, account_id, folder_id, uid, message_id, in_reply_to,
                    subject, from_name, from_address, to_recipients_json, cc_recipients_json,
                    date_epoch, snippet, is_read, is_flagged, is_draft, is_deleted,
                    body_fetched, size_bytes
             FROM messages WHERE is_deleted = 0",
        );

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(fid) = folder_id {
            query.push_str(" AND folder_id = ?");
            params_vec.push(Box::new(fid.to_string()));
        } else if let Some(aid) = account_id {
            query.push_str(" AND account_id = ?");
            params_vec.push(Box::new(aid.to_string()));
        }

        if let Some(search) = search_query {
            if !search.trim().is_empty() {
                query.push_str(" AND (subject LIKE ? OR from_address LIKE ? OR snippet LIKE ?)");
                let pattern = format!("%{}%", search.trim());
                params_vec.push(Box::new(pattern.clone()));
                params_vec.push(Box::new(pattern.clone()));
                params_vec.push(Box::new(pattern));
            }
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
                })
            })
            .map_err(|e| EmailError::Database(e.to_string()))?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row.map_err(|e| EmailError::Database(e.to_string()))?);
        }
        Ok(messages)
    }

    pub fn get_message_detail(&self, message_id: &str) -> Result<Option<MessageDetail>> {
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        let result = conn.query_row(
            "SELECT id, account_id, folder_id, uid, message_id, in_reply_to,
                    subject, from_name, from_address, to_recipients_json, cc_recipients_json,
                    date_epoch, snippet, is_read, is_flagged, is_draft, is_deleted,
                    body_plain, body_html, body_fetched, size_bytes
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
        let conn = self.pool.get().map_err(|e| EmailError::Database(e.to_string()))?;
        conn.execute(
            "UPDATE messages SET folder_id = ?1 WHERE id = ?2",
            params![target_folder_id, message_id],
        )
        .map_err(|e| EmailError::Database(e.to_string()))?;
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
    }
}

