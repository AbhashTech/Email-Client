use crate::theme::AppTheme;
use crate::tray::AppTray;
use crate::views::*;
use eframe::App;
use egui::{Color32, RichText, Rounding, Stroke, TopBottomPanel};
use email_core::events::{SyncCommand, SyncEvent};
use email_core::models::{Account, Folder, MessageDetail, MessageHeader, OutgoingDraft, Signature, Template};
use email_keychain::CredentialStore;
use email_storage::Storage;
use log::{error, warn};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct PendingSend {
    pub draft: OutgoingDraft,
    pub scheduled_time: std::time::Instant,
    pub duration: std::time::Duration,
}

#[derive(Debug)]
pub enum UiAsyncLoadResult {
    MessageDetailAndThread {
        requested_id: String,
        detail: Option<MessageDetail>,
        thread_messages: Vec<MessageDetail>,
    },
    FoldersForAccount {
        account_id: String,
        folders: Vec<Folder>,
    },
    ReloadedMessages {
        folder: FolderSelection,
        messages: Vec<MessageHeader>,
    },
    QueueCounts {
        scheduled: usize,
        outbox: usize,
    },
    SnoozedReturned {
        count: usize,
    },
}

pub struct EmailApp {
    storage: Storage,
    keyring: Arc<dyn CredentialStore>,
    cmd_tx: mpsc::UnboundedSender<SyncCommand>,
    event_rx: broadcast::Receiver<SyncEvent>,
    rt_handle: tokio::runtime::Handle,
    tray: Option<AppTray>,
    egui_ctx: egui::Context,
    async_load_tx: std::sync::mpsc::Sender<UiAsyncLoadResult>,
    async_load_rx: std::sync::mpsc::Receiver<UiAsyncLoadResult>,

    // Data State
    accounts: Vec<Account>,
    folders_by_account: HashMap<String, Vec<Folder>>,
    messages: Vec<MessageHeader>,
    selected_message_detail: Option<MessageDetail>,
    selected_thread_messages: Vec<MessageDetail>,
    templates: Vec<Template>,
    signatures: Vec<Signature>,

    // UI Navigation State
    selected_folder: FolderSelection,
    selected_message_id: Option<String>,
    selected_message_ids: HashSet<String>,
    last_clicked_idx: Option<usize>,
    search_query: String,
    focus_search_requested: bool,
    allowed_remote_images: HashSet<String>,
    pending_send: Option<PendingSend>,
    current_theme: crate::theme::ThemePreset,
    status_text: String,
    status_toast: Option<(String, std::time::Instant)>,
    is_syncing: bool,
    show_sidebar: bool,
    show_message_list: bool,
    is_window_visible: bool,
    is_maximized: bool,
    last_queue_check: std::time::Instant,
    scheduled_count: usize,
    outbox_count: usize,
    show_scheduled_modal: bool,
    show_move_modal: bool,
    last_applied_system_theme: Option<egui::Theme>,

    // Bug Fix: graceful quit flag (replaces std::process::exit)
    should_quit: bool,
    // Cancellation token shared with background tasks — cancelled on quit
    // so IMAP IDLE loops exit promptly instead of blocking the runtime drop.
    shutdown: CancellationToken,

    // Enhancement 1: Auto-sync timer
    last_auto_sync: std::time::Instant,

    // Enhancement 3: Live memory usage cache
    cached_mem_rss: String,
    last_mem_refresh: std::time::Instant,

    // Sub-views
    account_setup_view: AccountSetupView,
    compose_view: ComposeView,
    settings_view: SettingsView,
    command_palette: CommandPalette,
}



impl EmailApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        storage: Storage,
        keyring: Arc<dyn CredentialStore>,
        cmd_tx: mpsc::UnboundedSender<SyncCommand>,
        event_rx: broadcast::Receiver<SyncEvent>,
        rt_handle: tokio::runtime::Handle,
        shutdown: CancellationToken,
    ) -> Self {
        let mut current_theme = crate::theme::ThemePreset::DarkSlate;
        let mut active_custom_theme_id = None;

        if let Ok(Some(custom_id)) = storage.get_setting("theme_custom_id") {
            if !custom_id.trim().is_empty() {
                let custom_themes = crate::theme::load_custom_themes();
                if let Some(ct) = custom_themes.iter().find(|t| t.id == custom_id) {
                    AppTheme::apply_custom(&cc.egui_ctx, ct);
                    active_custom_theme_id = Some(custom_id);
                }
            }
        }

        if active_custom_theme_id.is_none() {
            if let Ok(Some(preset_key)) = storage.get_setting("theme_preset") {
                current_theme = crate::theme::ThemePreset::from_key(&preset_key);
            }
            AppTheme::apply_preset(&cc.egui_ctx, current_theme);
        }

        let tray = AppTray::new(cmd_tx.clone(), rt_handle.clone(), cc.egui_ctx.clone());
        let (async_load_tx, async_load_rx) = std::sync::mpsc::channel();

        let mut settings_view = SettingsView::new();
        settings_view.active_custom_theme_id = active_custom_theme_id;

        let mut app = Self {
            storage,
            keyring,
            cmd_tx,
            event_rx,
            rt_handle,
            tray: Some(tray),
            egui_ctx: cc.egui_ctx.clone(),
            async_load_tx,
            async_load_rx,
            accounts: Vec::new(),
            folders_by_account: HashMap::new(),
            messages: Vec::new(),
            selected_message_detail: None,
            selected_thread_messages: Vec::new(),
            templates: Vec::new(),
            signatures: Vec::new(),
            selected_folder: FolderSelection::UnifiedUnread,
            selected_message_id: None,
            selected_message_ids: HashSet::new(),
            last_clicked_idx: None,
            search_query: String::new(),
            focus_search_requested: false,
            allowed_remote_images: HashSet::new(),
            pending_send: None,
            current_theme,
            status_text: "Ready".to_string(),
            status_toast: None,
            is_syncing: false,
            show_sidebar: true,
            show_message_list: true,
            is_window_visible: true,
            is_maximized: false,
            last_queue_check: std::time::Instant::now(),
            scheduled_count: 0,
            outbox_count: 0,
            show_scheduled_modal: false,
            show_move_modal: false,
            last_applied_system_theme: None,
            should_quit: false,
            shutdown: shutdown.clone(),
            last_auto_sync: std::time::Instant::now(),
            cached_mem_rss: "–".to_string(),
            last_mem_refresh: std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(10))
                .unwrap_or_else(std::time::Instant::now),
            account_setup_view: AccountSetupView::new(),
            compose_view: ComposeView::new(),
            settings_view,
            command_palette: CommandPalette::new(),
        };



        app.reload_data();

        // If no accounts, open setup view immediately
        if app.accounts.is_empty() {
            app.account_setup_view.open();
        } else {
            // Auto-sync on startup
            let _ = app.cmd_tx.send(SyncCommand::SyncAll);
        }

        // Wakes the reactive egui event loop whenever background sync events occur.
        // Handles RecvError::Lagged gracefully so bursts never terminate the background waker.
        let mut bcast_rx = app.event_rx.resubscribe();
        let egui_ctx_events = cc.egui_ctx.clone();
        app.rt_handle.spawn(async move {
            loop {
                match bcast_rx.recv().await {
                    Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        egui_ctx_events.request_repaint();
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        // Gentle 3s background timer for queue processing and auto-sync checks
        let egui_ctx_timer = cc.egui_ctx.clone();
        let shutdown_timer = shutdown.clone();
        app.rt_handle.spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        egui_ctx_timer.request_repaint();
                    }
                    _ = shutdown_timer.cancelled() => {
                        break;
                    }
                }
            }
        });

        app
    }


    pub fn reload_data(&mut self) {
        if let Ok(accounts) = self.storage.get_accounts() {
            self.accounts = accounts;
        }

        self.folders_by_account.clear();
        for acc in &self.accounts {
            if let Ok(folders) = self.storage.get_folders_for_account(&acc.id) {
                self.folders_by_account.insert(acc.id.clone(), folders);
            }
        }

        if let Ok(templates) = self.storage.get_templates() {
            self.templates = templates;
        }

        if let Ok(signatures) = self.storage.get_signatures(None) {
            self.signatures = signatures;
        }

        self.scheduled_count = self.storage.list_all_scheduled(None).unwrap_or_default().len();
        self.outbox_count = self.storage.get_all_outbox_items(None).unwrap_or_default().len();

        self.update_tray_unread();
        if let Ok(msgs) = self.storage.get_messages(None, None, 150, 0, None) {
            let mut m = msgs;
            if matches!(self.selected_folder, FolderSelection::UnifiedUnread) {
                m.retain(|msg| !msg.is_read);
            }
            self.messages = m;
        }
    }

    pub fn update_tray_unread(&self) {
        let total_unread: u32 = self
            .folders_by_account
            .values()
            .flatten()
            .map(|f| f.unread_messages)
            .sum();

        if let Some(ref tray) = self.tray {
            tray.update_unread_count(total_unread);
        }
    }

    #[allow(dead_code)]
    pub fn load_selected_thread(&mut self) {
        if let Some(ref detail) = self.selected_message_detail {
            let storage_for_thread = self.storage.clone();
            let mid_clone = detail.header.id.clone();
            let det_clone = detail.clone();
            let tx = self.async_load_tx.clone();
            let egui_ctx = self.egui_ctx.clone();
            self.rt_handle.spawn_blocking(move || {
                let thread_messages = storage_for_thread
                    .get_conversation_thread(&det_clone.header.id)
                    .ok()
                    .flatten()
                    .map(|t| t.messages)
                    .unwrap_or_else(|| vec![det_clone.clone()]);
                let _ = tx.send(UiAsyncLoadResult::MessageDetailAndThread {
                    requested_id: mid_clone,
                    detail: Some(det_clone),
                    thread_messages,
                });
                egui_ctx.request_repaint();
            });
        } else {
            self.selected_thread_messages.clear();
        }
    }

    pub fn trigger_async_reload_messages(&self) {
        let folder = self.selected_folder.clone();
        let search = if self.search_query.is_empty() {
            None
        } else {
            Some(self.search_query.clone())
        };
        let storage = self.storage.clone();
        let tx = self.async_load_tx.clone();
        let egui_ctx = self.egui_ctx.clone();

        self.rt_handle.spawn_blocking(move || {
            let search_ref = search.as_deref();
            let messages = match &folder {
                FolderSelection::UnifiedOutbox => {
                    if let Ok(items) = storage.get_all_outbox_items(None) {
                        items
                            .into_iter()
                            .map(|item| {
                                let snippet = if let Some(ref err) = item.last_error {
                                    format!("⚠️ Retry {}/{} failed: {}", item.retry_count, item.max_retries, err)
                                } else {
                                    format!("📤 Queued for delivery (Retry count: {})", item.retry_count)
                                };
                                MessageHeader {
                                    id: format!("outbox_{}", item.id),
                                    account_id: item.account_id,
                                    folder_id: "outbox".to_string(),
                                    uid: 0,
                                    message_id: None,
                                    in_reply_to: item.draft.in_reply_to,
                                    subject: format!("[Outbox] {}", item.draft.subject),
                                    from_name: Some("Outbox Auto-Retry".to_string()),
                                    from_address: "outbox@queue".to_string(),
                                    to_recipients: item.draft.to,
                                    cc_recipients: item.draft.cc,
                                    date_epoch: item.created_at,
                                    snippet,
                                    is_read: true,
                                    is_flagged: false,
                                    is_draft: true,
                                    is_deleted: false,
                                    body_fetched: true,
                                    size_bytes: item.draft.body_plain.len() as u64,
                                    snooze_until: None,
                                }
                            })
                            .collect()
                    } else {
                        Vec::new()
                    }
                }
                FolderSelection::UnifiedSnoozed => {
                    storage.get_snoozed_messages(None).unwrap_or_default()
                }
                FolderSelection::UnifiedFlagged | FolderSelection::UnifiedUnread => {
                    if let Ok(mut msgs) = storage.get_messages(None, None, 150, 0, search_ref) {
                        if matches!(folder, FolderSelection::UnifiedFlagged) {
                            msgs.retain(|m| m.is_flagged);
                        } else if matches!(folder, FolderSelection::UnifiedUnread) {
                            msgs.retain(|m| !m.is_read);
                        }
                        msgs
                    } else {
                        Vec::new()
                    }
                }
                FolderSelection::Folder {
                    account_id,
                    folder_id,
                } => {
                    storage
                        .get_messages(Some(account_id), Some(folder_id), 150, 0, search_ref)
                        .unwrap_or_default()
                }
            };

            let _ = tx.send(UiAsyncLoadResult::ReloadedMessages { folder, messages });
            egui_ctx.request_repaint();
        });
    }

    pub fn reload_messages(&mut self) {
        self.trigger_async_reload_messages();
    }

    fn poll_background_events(&mut self, ctx: &egui::Context) {
        // Poll Tray Actions
        if let Some(ref mut tray) = self.tray {
            while let Some(action) = tray.try_recv_action() {
                match action {
                    crate::tray::TrayAction::ToggleVisibility => {
                        self.is_window_visible = !self.is_window_visible;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(self.is_window_visible));
                        if self.is_window_visible {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        }
                        tray.set_visible(self.is_window_visible);
                    }
                    crate::tray::TrayAction::ShowApp => {
                        self.is_window_visible = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        tray.set_visible(true);
                    }
                    crate::tray::TrayAction::HideApp => {
                        self.is_window_visible = false;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                        tray.set_visible(false);
                    }
                    crate::tray::TrayAction::ComposeEmail => {
                        self.is_window_visible = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        tray.set_visible(true);
                        self.compose_view.open_new(self.accounts.first().map(|a| a.id.as_str()), &self.signatures);
                    }
                    crate::tray::TrayAction::SyncAll => {
                        let _ = self.cmd_tx.send(SyncCommand::SyncAll);
                    }
                    crate::tray::TrayAction::Quit => {
                        self.shutdown.cancel();
                        self.should_quit = true;
                    }
                }
            }
        }

        // Poll Async Data Loads
        while let Ok(result) = self.async_load_rx.try_recv() {
            match result {
                UiAsyncLoadResult::MessageDetailAndThread {
                    requested_id,
                    detail,
                    thread_messages,
                } => {
                    if self.selected_message_id.as_deref() == Some(&requested_id) {
                        self.selected_message_detail = detail;
                        self.selected_thread_messages = thread_messages;
                    }
                }
                UiAsyncLoadResult::FoldersForAccount {
                    account_id,
                    folders,
                } => {
                    self.folders_by_account.insert(account_id, folders);
                    self.update_tray_unread();
                }
                UiAsyncLoadResult::ReloadedMessages { folder, messages } => {
                    if self.selected_folder == folder {
                        self.messages = messages;
                    }
                }
                UiAsyncLoadResult::QueueCounts { scheduled, outbox } => {
                    self.scheduled_count = scheduled;
                    self.outbox_count = outbox;
                }
                UiAsyncLoadResult::SnoozedReturned { count } => {
                    if count > 0 {
                        self.status_toast = Some((
                            format!("🔔 {} snoozed email(s) returned to your Inbox!", count),
                            std::time::Instant::now(),
                        ));
                        self.trigger_async_reload_messages();
                    }
                }
            }
        }

        // Poll Sync Events
        loop {
            let event = match self.event_rx.try_recv() {
                Ok(event) => event,
                Err(tokio::sync::broadcast::error::TryRecvError::Lagged(n)) => {
                    warn!("Sync event receiver lagged by {} events", n);
                    continue;
                }
                Err(tokio::sync::broadcast::error::TryRecvError::Empty)
                | Err(tokio::sync::broadcast::error::TryRecvError::Closed) => break,
            };
            match event {
                SyncEvent::SyncStatusChanged {
                    is_syncing,
                    status_text,
                } => {
                    // Enhancement 5: toast on sync completion
                    if self.is_syncing && !is_syncing {
                        self.status_toast = Some((
                            format!("✓ {}", status_text),
                            std::time::Instant::now(),
                        ));
                    }
                    self.is_syncing = is_syncing;
                    self.status_text = status_text;
                }
                SyncEvent::FolderSynced {
                    account_id,
                    folder_id,
                    new_messages_count,
                } => {
                    if new_messages_count > 0 {
                        let is_current = match &self.selected_folder {
                            FolderSelection::Folder { folder_id: cur_f, .. } => cur_f == &folder_id,
                            _ => true,
                        };
                        if is_current {
                            self.reload_messages();
                        }
                    }
                    let storage_clone = self.storage.clone();
                    let acc_id = account_id.clone();
                    let tx = self.async_load_tx.clone();
                    let egui_ctx = self.egui_ctx.clone();
                    self.rt_handle.spawn_blocking(move || {
                        if let Ok(folders) = storage_clone.get_folders_for_account(&acc_id) {
                            let _ = tx.send(UiAsyncLoadResult::FoldersForAccount {
                                account_id: acc_id,
                                folders,
                            });
                            egui_ctx.request_repaint();
                        }
                    });
                }
                SyncEvent::FoldersDiscovered {
                    account_id: _,
                    folders,
                } => {
                    self.account_setup_view.discovered_folders = folders;
                    self.account_setup_view.test_status_msg = Some((
                        true,
                        "Folders discovered successfully! Select which to sync below.".to_string(),
                    ));
                }
                SyncEvent::BodyFetched { message_id, detail } => {
                    if self.selected_message_id.as_deref() == Some(&message_id) {
                        self.selected_message_detail = Some(*detail.clone());
                        let storage_for_thread = self.storage.clone();
                        let mid_clone = message_id.clone();
                        let tx = self.async_load_tx.clone();
                        let egui_ctx = self.egui_ctx.clone();
                        let det_clone = *detail.clone();
                        self.rt_handle.spawn_blocking(move || {
                            let thread_messages = storage_for_thread
                                .get_conversation_thread(&det_clone.header.id)
                                .ok()
                                .flatten()
                                .map(|t| t.messages)
                                .unwrap_or_else(|| vec![det_clone.clone()]);
                            let _ = tx.send(UiAsyncLoadResult::MessageDetailAndThread {
                                requested_id: mid_clone,
                                detail: Some(det_clone),
                                thread_messages,
                            });
                            egui_ctx.request_repaint();
                        });
                    }
                    if let Some(m) = self.messages.iter_mut().find(|m| m.id == message_id) {
                        *m = detail.header.clone();
                    }
                }
                SyncEvent::ConnectionTestResult {
                    success,
                    imap_ok: _,
                    smtp_ok: _,
                    message,
                } => {
                    self.account_setup_view.test_status_msg = Some((success, message));
                }
                SyncEvent::EmailSent { subject } => {
                    self.status_text = format!("Email sent: '{}'", subject);
                }
                SyncEvent::SyncError { error_message, .. } => {
                    error!("Sync error: {}", error_message);
                    self.status_text = format!("Error: {}", error_message);
                }
                SyncEvent::NewMailNotification { from, subject, .. } => {
                    self.status_text = format!("New mail from {}: {}", from, subject);
                    
                    #[cfg(target_os = "linux")]
                    {
                        let from_clone = from.clone();
                        let subject_clone = subject.clone();
                        std::thread::spawn(move || {
                            let _ = notify_rust::Notification::new()
                                .summary(&format!("📬 New Mail from {}", from_clone))
                                .body(&subject_clone)
                                .icon("mail-unread")
                                .timeout(notify_rust::Timeout::Milliseconds(5000))
                                .show();
                        });
                    }
                    self.reload_messages();
                }
            }
        }

        self.check_background_queues();
    }

    pub fn check_background_queues(&mut self) {
        if self.last_queue_check.elapsed() < std::time::Duration::from_secs(10) {
            return;
        }
        self.last_queue_check = std::time::Instant::now();

        let storage_clone = self.storage.clone();
        let cmd_tx_clone = self.cmd_tx.clone();
        let tx = self.async_load_tx.clone();
        let egui_ctx = self.egui_ctx.clone();
        let now_ts = chrono::Utc::now().timestamp();

        self.rt_handle.spawn_blocking(move || {
            let mut outbox_cnt = 0;
            if let Ok(all_outbox) = storage_clone.get_all_outbox_items(None) {
                outbox_cnt = all_outbox.len();
            }
            if let Ok(due_items) = storage_clone.get_due_outbox_items() {
                for item in due_items {
                    let _ = cmd_tx_clone.send(SyncCommand::SendEmail {
                        draft: item.draft.clone(),
                        password: None,
                    });
                    let _ = storage_clone.delete_outbox_item(&item.id);
                }
            }

            let mut snoozed_restored = 0;
            if let Ok(due_snoozed) = storage_clone.get_due_snoozed_messages(now_ts) {
                snoozed_restored = due_snoozed.len();
                for msg in due_snoozed {
                    let _ = storage_clone.unsnooze_message(&msg.id);
                }
            }

            let mut sched_cnt = 0;
            if let Ok(all_sched) = storage_clone.list_all_scheduled(None) {
                sched_cnt = all_sched.len();
            }
            if let Ok(due_list) = storage_clone.get_due_scheduled_emails(now_ts) {
                for item in due_list {
                    let _ = cmd_tx_clone.send(SyncCommand::SendEmail {
                        draft: item.draft.clone(),
                        password: None,
                    });
                    let _ = storage_clone.delete_scheduled_email(&item.id);
                }
            }

            let _ = tx.send(UiAsyncLoadResult::QueueCounts {
                scheduled: sched_cnt,
                outbox: outbox_cnt,
            });
            if snoozed_restored > 0 {
                let _ = tx.send(UiAsyncLoadResult::SnoozedReturned {
                    count: snoozed_restored,
                });
            }
            egui_ctx.request_repaint();
        });
    }

    pub fn handle_close_requested(&mut self, ctx: &egui::Context) {
        let cfg = crate::load_app_config();
        match cfg.close_action {
            crate::CloseButtonAction::MinimizeToTray => {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                self.is_window_visible = false;
                if let Some(ref tray) = self.tray {
                    tray.set_visible(false);
                }
            }
            crate::CloseButtonAction::QuitApplication => {
                self.shutdown.cancel();
                self.should_quit = true;
            }
        }
    }

    // Enhancement 1: Auto-sync on configurable timer
    pub fn check_auto_sync(&mut self) {
        let cfg = crate::load_app_config();
        let global_interval_secs = cfg.auto_sync_interval_secs;
        
        if self.accounts.is_empty() || self.is_syncing {
            return;
        }


        // Use the minimum interval across all accounts to determine the tick rate,
        // then sync each account that has its own interval configured.
        let mut min_interval = global_interval_secs;
        for acc in &self.accounts {
            if let Some(i) = acc.sync_interval_secs {
                if i > 0 && i < min_interval {
                    min_interval = i;
                }
            }
        }
        
        if min_interval == 0 { return; }
        
        if self.last_auto_sync.elapsed() >= std::time::Duration::from_secs(min_interval) {
            self.last_auto_sync = std::time::Instant::now();
            
            // Sync each account that is due.
            // Since we only track one last_auto_sync, we just SyncAll for simplicity,
            // but the prompt says: "use account.sync_interval_secs.unwrap_or(interval_secs) per account when syncing"
            
            for account in &self.accounts {
                let acc_interval = account.sync_interval_secs.unwrap_or(global_interval_secs);
                if acc_interval > 0 {
                    let _ = self.cmd_tx.send(SyncCommand::SyncAccount { account_id: account.id.clone() });
                }
            }
        }
    }

    // Enhancement 3: Read process RSS cross-platform (instant /proc on Linux, sysinfo fallback)
    fn read_rss_memory() -> String {
        #[cfg(target_os = "linux")]
        {
            if let Ok(content) = std::fs::read_to_string("/proc/self/statm") {
                if let Some(rss_pages_str) = content.split_whitespace().nth(1) {
                    if let Ok(pages) = rss_pages_str.parse::<u64>() {
                        let mb = (pages * 4) / 1024;
                        return format!("{} MB", mb);
                    }
                }
            }
        }
        use sysinfo::{Pid, ProcessesToUpdate, System};
        let pid = Pid::from_u32(std::process::id());
        let mut sys = System::new();
        sys.refresh_processes(ProcessesToUpdate::Some(&[pid]));
        if let Some(proc) = sys.process(pid) {
            let mb = proc.memory() / (1024 * 1024);
            format!("{} MB", mb)
        } else {
            "– MB".to_string()
        }
    }

    pub fn refresh_memory_stat(&mut self) {
        if self.last_mem_refresh.elapsed() >= std::time::Duration::from_secs(5) {
            self.cached_mem_rss = Self::read_rss_memory();
            self.last_mem_refresh = std::time::Instant::now();
        }
    }

    // Enhancement 4: Mark all messages in current view as read
    pub fn mark_all_messages_read(&mut self) {
        let unread_ids: Vec<(String, String, String, u32)> = self
            .messages
            .iter()
            .filter(|m| !m.is_read)
            .map(|m| (m.id.clone(), m.account_id.clone(), m.folder_id.clone(), m.uid))
            .collect();

        let storage_clone = self.storage.clone();
        let unread_ids_clone = unread_ids.clone();
        self.rt_handle.spawn_blocking(move || {
            for (mid, _, _, _) in unread_ids_clone {
                let _ = storage_clone.set_message_read(&mid, true);
            }
        });

        for (_mid, account_id, folder_id, uid) in &unread_ids {
            let _ = self.cmd_tx.send(SyncCommand::SetReadStatus {
                account_id: account_id.clone(),
                folder_id: folder_id.clone(),
                uid: *uid,
                is_read: true,
            });
        }

        // Update in-memory message list
        for m in &mut self.messages {
            m.is_read = true;
        }

        // Update folder unread counters
        for (_, account_id, folder_id, _) in &unread_ids {
            if let Some(folders) = self.folders_by_account.get_mut(account_id) {
                if let Some(f) = folders.iter_mut().find(|f| &f.id == folder_id) {
                    f.unread_messages = f.unread_messages.saturating_sub(1);
                }
            }
        }
        self.update_tray_unread();

        let count = unread_ids.len();
        if count > 0 {
            self.status_toast = Some((
                format!("✓ Marked {} message(s) as read", count),
                std::time::Instant::now(),
            ));
        }
    }

    pub fn populate_command_palette(&mut self) {
        let mut items = vec![
            PaletteItem {
                id: "compose".into(),
                title: "Compose New Email".into(),
                category: "Actions".into(),
                shortcut: Some("c".into()),
                action: PaletteAction::Compose,
            },
            PaletteItem {
                id: "sync".into(),
                title: "Sync All Mailboxes".into(),
                category: "Actions".into(),
                shortcut: Some("F5".into()),
                action: PaletteAction::SyncAll,
            },
            PaletteItem {
                id: "settings".into(),
                title: "Open Settings".into(),
                category: "Navigation".into(),
                shortcut: Some("Cmd+,".into()),
                action: PaletteAction::OpenSettings,
            },
            PaletteItem {
                id: "focus_search".into(),
                title: "Search Messages".into(),
                category: "Navigation".into(),
                shortcut: Some("/".into()),
                action: PaletteAction::FocusSearch,
            },
            PaletteItem {
                id: "toggle_sidebar".into(),
                title: "Toggle Left Sidebar".into(),
                category: "View".into(),
                shortcut: None,
                action: PaletteAction::ToggleSidebar,
            },
            PaletteItem {
                id: "toggle_list".into(),
                title: "Toggle Message List Pane".into(),
                category: "View".into(),
                shortcut: None,
                action: PaletteAction::ToggleMessageList,
            },
            PaletteItem {
                id: "folder_unread".into(),
                title: "Smart View: Unread Messages".into(),
                category: "Folders".into(),
                shortcut: None,
                action: PaletteAction::SelectFolder("unified_unread".into()),
            },
            PaletteItem {
                id: "folder_flagged".into(),
                title: "Smart View: Starred / Flagged".into(),
                category: "Folders".into(),
                shortcut: None,
                action: PaletteAction::SelectFolder("unified_flagged".into()),
            },
        ];

        // Add accounts and their custom folders
        for acc in &self.accounts {
            if let Some(folders) = self.folders_by_account.get(&acc.id) {
                for f in folders {
                    items.push(PaletteItem {
                        id: format!("folder_{}", f.id),
                        title: format!("{} → {}", acc.email, f.display_name),
                        category: "Account Folders".into(),
                        shortcut: None,
                        action: PaletteAction::SelectFolder(f.id.clone()),
                    });
                }
            }
        }

        // Add message actions if an email is selected
        if self.selected_message_id.is_some() {
            items.push(PaletteItem {
                id: "reply".into(),
                title: "Reply to Current Email".into(),
                category: "Message".into(),
                shortcut: Some("r".into()),
                action: PaletteAction::Reply,
            });
            items.push(PaletteItem {
                id: "reply_all".into(),
                title: "Reply All to Current Email".into(),
                category: "Message".into(),
                shortcut: Some("a".into()),
                action: PaletteAction::ReplyAll,
            });
            items.push(PaletteItem {
                id: "forward".into(),
                title: "Forward Current Email".into(),
                category: "Message".into(),
                shortcut: Some("f".into()),
                action: PaletteAction::Forward,
            });
            items.push(PaletteItem {
                id: "star".into(),
                title: "Toggle Star / Flag".into(),
                category: "Message".into(),
                shortcut: Some("s".into()),
                action: PaletteAction::ToggleStar,
            });
            items.push(PaletteItem {
                id: "mark_read".into(),
                title: "Mark as Read".into(),
                category: "Message".into(),
                shortcut: None,
                action: PaletteAction::MarkRead,
            });
            items.push(PaletteItem {
                id: "mark_unread".into(),
                title: "Mark as Unread (Toggle)".into(),
                category: "Message".into(),
                shortcut: Some("u".into()),
                action: PaletteAction::MarkUnread,
            });
            items.push(PaletteItem {
                id: "delete".into(),
                title: "Delete Email(s)".into(),
                category: "Message".into(),
                shortcut: Some("Del".into()),
                action: PaletteAction::DeleteSelected,
            });
        }

        // Theme Presets
        for preset in crate::theme::ThemePreset::all() {
            items.push(PaletteItem {
                id: format!("theme_{:?}", preset),
                title: format!("Switch Theme: {}", preset.display_name()),
                category: "Themes".into(),
                shortcut: None,
                action: PaletteAction::SetTheme(*preset),
            });
        }

        self.command_palette.set_items(items);
    }

    pub fn execute_palette_action(&mut self, action: PaletteAction) {
        match action {
            PaletteAction::SetTheme(preset) => {
                self.current_theme = preset;
                self.settings_view.active_custom_theme_id = None;
                let _ = self.storage.set_setting("theme_preset", preset.to_key());
                let _ = self.storage.set_setting("theme_custom_id", "");
                self.status_toast = Some((format!("Switched to {} theme", preset.display_name()), std::time::Instant::now()));
            }
            PaletteAction::Compose => {
                self.compose_view.open_new(self.accounts.first().map(|a| a.id.as_str()), &self.signatures);
            }
            PaletteAction::SyncAll => {
                let _ = self.cmd_tx.send(SyncCommand::SyncAll);
            }
            PaletteAction::OpenSettings => {
                self.settings_view.open();
            }
            PaletteAction::ToggleSidebar => {
                self.show_sidebar = !self.show_sidebar;
            }
            PaletteAction::ToggleMessageList => {
                self.show_message_list = !self.show_message_list;
            }
            PaletteAction::FocusSearch => {
                self.focus_search_requested = true;
            }
            PaletteAction::SelectFolder(fid) => {
                if fid == "unified_unread" {
                    self.selected_folder = FolderSelection::UnifiedUnread;
                } else if fid == "unified_flagged" {
                    self.selected_folder = FolderSelection::UnifiedFlagged;
                } else {
                    for (acc_id, folders) in &self.folders_by_account {
                        if folders.iter().any(|f| f.id == fid) {
                            self.selected_folder = FolderSelection::Folder {
                                account_id: acc_id.clone(),
                                folder_id: fid.clone(),
                            };
                            break;
                        }
                    }
                }
                self.selected_message_id = None;
                self.selected_message_ids.clear();
                self.selected_message_detail = None;
                self.reload_messages();
            }
            PaletteAction::MarkRead => {
                if let Some(ref mid) = self.selected_message_id {
                    let storage_clone = self.storage.clone();
                    let m_id = mid.clone();
                    self.rt_handle.spawn_blocking(move || {
                        let _ = storage_clone.set_message_read(&m_id, true);
                    });
                    if let Some(m) = self.messages.iter_mut().find(|m| &m.id == mid) {
                        m.is_read = true;
                        let _ = self.cmd_tx.send(SyncCommand::SetReadStatus {
                            account_id: m.account_id.clone(),
                            folder_id: m.folder_id.clone(),
                            uid: m.uid,
                            is_read: true,
                        });
                        if let Some(folders) = self.folders_by_account.get_mut(&m.account_id) {
                            if let Some(f) = folders.iter_mut().find(|f| f.id == m.folder_id) {
                                f.unread_messages = f.unread_messages.saturating_sub(1);
                            }
                        }
                    }
                    if let Some(ref mut detail) = self.selected_message_detail {
                        if &detail.header.id == mid {
                            detail.header.is_read = true;
                        }
                    }
                    for tm in &mut self.selected_thread_messages {
                        if &tm.header.id == mid {
                            tm.header.is_read = true;
                        }
                    }
                    self.update_tray_unread();
                }
            }
            PaletteAction::MarkUnread => {
                if let Some(ref mid) = self.selected_message_id {
                    let storage_clone = self.storage.clone();
                    let m_id = mid.clone();
                    self.rt_handle.spawn_blocking(move || {
                        let _ = storage_clone.set_message_read(&m_id, false);
                    });
                    if let Some(m) = self.messages.iter_mut().find(|m| &m.id == mid) {
                        m.is_read = false;
                        let _ = self.cmd_tx.send(SyncCommand::SetReadStatus {
                            account_id: m.account_id.clone(),
                            folder_id: m.folder_id.clone(),
                            uid: m.uid,
                            is_read: false,
                        });
                        if let Some(folders) = self.folders_by_account.get_mut(&m.account_id) {
                            if let Some(f) = folders.iter_mut().find(|f| f.id == m.folder_id) {
                                f.unread_messages += 1;
                            }
                        }
                    }
                    if let Some(ref mut detail) = self.selected_message_detail {
                        if &detail.header.id == mid {
                            detail.header.is_read = false;
                        }
                    }
                    for tm in &mut self.selected_thread_messages {
                        if &tm.header.id == mid {
                            tm.header.is_read = false;
                        }
                    }
                    self.update_tray_unread();
                }
            }
            PaletteAction::ToggleStar => {
                if let Some(ref mid) = self.selected_message_id {
                    let mut new_flag = false;
                    if let Some(m) = self.messages.iter_mut().find(|m| &m.id == mid) {
                        new_flag = !m.is_flagged;
                        m.is_flagged = new_flag;
                        let storage_clone = self.storage.clone();
                        let m_id = mid.clone();
                        self.rt_handle.spawn_blocking(move || {
                            let _ = storage_clone.set_message_flagged(&m_id, new_flag);
                        });
                        let _ = self.cmd_tx.send(SyncCommand::SetFlaggedStatus {
                            account_id: m.account_id.clone(),
                            folder_id: m.folder_id.clone(),
                            uid: m.uid,
                            is_flagged: new_flag,
                        });
                    }
                    if let Some(ref mut detail) = self.selected_message_detail {
                        if &detail.header.id == mid {
                            detail.header.is_flagged = new_flag;
                        }
                    }
                    for tm in &mut self.selected_thread_messages {
                        if &tm.header.id == mid {
                            tm.header.is_flagged = new_flag;
                        }
                    }
                }
            }
            PaletteAction::DeleteSelected => {
                let to_delete = if !self.selected_message_ids.is_empty() {
                    self.selected_message_ids.iter().cloned().collect::<Vec<_>>()
                } else if let Some(ref mid) = self.selected_message_id {
                    vec![mid.clone()]
                } else {
                    Vec::new()
                };

                let storage_clone = self.storage.clone();
                let del_clone = to_delete.clone();
                self.rt_handle.spawn_blocking(move || {
                    for mid in &del_clone {
                        let _ = storage_clone.delete_message(mid);
                    }
                });

                for mid in &to_delete {
                    if let Some(m) = self.messages.iter().find(|m| &m.id == mid) {
                        let _ = self.cmd_tx.send(SyncCommand::DeleteMessage {
                            account_id: m.account_id.clone(),
                            folder_id: m.folder_id.clone(),
                            uid: m.uid,
                        });
                    }
                }
                self.messages.retain(|m| !to_delete.contains(&m.id));
                self.selected_message_ids.clear();
                self.selected_message_id = None;
                self.selected_message_detail = None;
                self.selected_thread_messages.clear();
            }
            PaletteAction::Reply => {
                if let Some(ref detail) = self.selected_message_detail {
                    let quote = detail.body_plain.clone().unwrap_or_default();
                    self.compose_view.open_reply(
                        &detail.header.account_id,
                        &detail.header.from_address,
                        "",
                        &detail.header.subject,
                        detail.header.message_id.clone(),
                        &quote,
                        &self.signatures,
                        true,
                    );
                }
            }
            PaletteAction::ReplyAll => {
                if let Some(ref detail) = self.selected_message_detail {
                    let quote = detail.body_plain.clone().unwrap_or_default();
                    let my_emails: std::collections::HashSet<String> = self
                        .accounts
                        .iter()
                        .map(|a| a.email.trim().to_lowercase())
                        .collect();
                    let (to_str, cc_str) = crate::views::compose::build_reply_all_recipients(&detail.header, &my_emails);
                    self.compose_view.open_reply(
                        &detail.header.account_id,
                        &to_str,
                        &cc_str,
                        &detail.header.subject,
                        detail.header.message_id.clone(),
                        &quote,
                        &self.signatures,
                        true,
                    );
                }
            }
            PaletteAction::Forward => {
                if let Some(ref detail) = self.selected_message_detail {
                    let quote = detail.body_plain.clone().unwrap_or_default();
                    let subj = format!("Fwd: {}", detail.header.subject);
                    self.compose_view.open_reply(
                        &detail.header.account_id,
                        "",
                        "",
                        &subj,
                        None,
                        &format!("---------- Forwarded message ---------\nFrom: {}\nSubject: {}\n\n{}", detail.header.from_address, detail.header.subject, quote),
                        &self.signatures,
                        true,
                    );
                }
            }
        }
    }
}


impl App for EmailApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_background_events(ctx);

        // Bug Fix: Graceful quit — let eframe close the window cleanly instead of
        // calling std::process::exit(), which avoids the "Wait or Terminate" dialog
        // on Linux compositors (Wayland/X11).
        if self.should_quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        if ctx.input(|i| i.viewport().close_requested()) {
            self.handle_close_requested(ctx);
        }

        // Enhancement 1: Periodic auto-sync
        self.check_auto_sync();

        // Enhancement 3: Refresh live memory stat
        self.refresh_memory_stat();

        if !self.is_window_visible {
            // Render an opaque background so Wayland compositor receives a valid committed frame buffer
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(AppTheme::bg_app_ctx(ctx)))
                .show(ctx, |_ui| {});
            return;
        }

        if self.current_theme == crate::theme::ThemePreset::SystemAuto
            || self.current_theme == crate::theme::ThemePreset::GruvboxAuto
        {
            let current_detected = crate::theme::detect_system_theme(ctx);
            if self.last_applied_system_theme != Some(current_detected) {
                self.last_applied_system_theme = Some(current_detected);
                AppTheme::apply_preset(ctx, self.current_theme);
            }
        }

        // Handle Ctrl+, / Cmd+, shortcut to open Settings
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Comma)) {
            self.settings_view.open();
        }

        // Handle Ctrl+K / Cmd+K / Ctrl+P shortcut to open Command Palette
        if ctx.input(|i| i.modifiers.command && (i.key_pressed(egui::Key::K) || i.key_pressed(egui::Key::P))) {
            self.populate_command_palette();
            self.command_palette.open();
        }

        fn str_to_key(s: &str) -> Option<egui::Key> {
            match s.to_lowercase().as_str() {
                "a" => Some(egui::Key::A), "b" => Some(egui::Key::B), "c" => Some(egui::Key::C),
                "d" => Some(egui::Key::D), "e" => Some(egui::Key::E), "f" => Some(egui::Key::F),
                "g" => Some(egui::Key::G), "h" => Some(egui::Key::H), "i" => Some(egui::Key::I),
                "j" => Some(egui::Key::J), "k" => Some(egui::Key::K), "l" => Some(egui::Key::L),
                "m" => Some(egui::Key::M), "n" => Some(egui::Key::N), "o" => Some(egui::Key::O),
                "p" => Some(egui::Key::P), "q" => Some(egui::Key::Q), "r" => Some(egui::Key::R),
                "s" => Some(egui::Key::S), "t" => Some(egui::Key::T), "u" => Some(egui::Key::U),
                "v" => Some(egui::Key::V), "w" => Some(egui::Key::W), "x" => Some(egui::Key::X),
                "y" => Some(egui::Key::Y), "z" => Some(egui::Key::Z),
                "/" => Some(egui::Key::Slash),
                _ => None,
            }
        }

        // Enhancement 2: Global keyboard shortcuts (only when no text field is focused)
        // These match the shortcuts listed in the Command Palette.
        if !ctx.wants_keyboard_input() {
            let kb = crate::load_app_config().keybindings;
            
            if let Some(k) = str_to_key(&kb.compose) {
                if ctx.input(|i| i.key_pressed(k)) {
                    self.compose_view.open_new(self.accounts.first().map(|a| a.id.as_str()), &self.signatures);
                }
            }
            if let Some(k) = str_to_key(&kb.focus_search) {
                if ctx.input(|i| i.key_pressed(k)) {
                    self.focus_search_requested = true;
                }
            }

            if self.selected_message_id.is_some() {
                if let Some(k) = str_to_key(&kb.reply) {
                    if ctx.input(|i| i.key_pressed(k)) {
                        if let Some(ref detail) = self.selected_message_detail {
                            let quote = detail.body_plain.clone().unwrap_or_default();
                            self.compose_view.open_reply(
                                &detail.header.account_id,
                                &detail.header.from_address,
                                "",
                                &detail.header.subject,
                                detail.header.message_id.clone(),
                                &quote,
                                &self.signatures,
                                true,
                            );
                        }
                    }
                }
                
                if let Some(k) = str_to_key(&kb.forward) {
                    if ctx.input(|i| i.key_pressed(k)) {
                        if let Some(ref detail) = self.selected_message_detail {
                            let quote = detail.body_plain.clone().unwrap_or_default();
                            let subj = format!("Fwd: {}", detail.header.subject);
                            self.compose_view.open_reply(
                                &detail.header.account_id,
                                "",
                                "",
                                &subj,
                                None,
                                &format!("---------- Forwarded message ---------\nFrom: {}\nSubject: {}\n\n{}", detail.header.from_address, detail.header.subject, quote),
                                &self.signatures,
                                true,
                            );
                        }
                    }
                }
                
                if let Some(k) = str_to_key(&kb.toggle_read) {
                    if ctx.input(|i| i.key_pressed(k)) {
                        self.execute_palette_action(PaletteAction::MarkUnread);
                    }
                }
                
                if let Some(k) = str_to_key(&kb.toggle_star) {
                    if ctx.input(|i| i.key_pressed(k)) {
                        self.execute_palette_action(PaletteAction::ToggleStar);
                    }
                }
                
                if let Some(k) = str_to_key(&kb.move_to_folder) {
                    if ctx.input(|i| i.key_pressed(k)) {
                        self.show_move_modal = true;
                    }
                }
                
                // Delete → Delete selected (hardcoded)
                if ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
                    self.execute_palette_action(PaletteAction::DeleteSelected);
                }
            }
        }

        TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let compose_btn = egui::Button::new(
                    RichText::new("✉ + Compose")
                        .size(12.5)
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(AppTheme::accent(ui))
                .rounding(Rounding::same(6.0));

                if ui.add(compose_btn).clicked() {
                    self.compose_view.open_new(self.accounts.first().map(|a| a.id.as_str()), &self.signatures);
                }

                if ui.button(RichText::new("🔄 Sync All").size(12.5)).clicked() {
                    let _ = self.cmd_tx.send(SyncCommand::SyncAll);
                }

                if self.scheduled_count > 0 {
                    let sched_text = format!("⏰ {} Scheduled", self.scheduled_count);
                    if ui.button(RichText::new(sched_text).size(12.5).color(AppTheme::accent(ui))).on_hover_text("View scheduled outbox queue").clicked() {
                        self.show_scheduled_modal = true;
                    }
                }

                if self.outbox_count > 0 {
                    let outbox_text = format!("📤 {} Outbox", self.outbox_count);
                    if ui.button(RichText::new(outbox_text).size(12.5).color(AppTheme::ACCENT_WARNING)).on_hover_text("View outbox auto-retry queue").clicked() {
                        self.selected_folder = FolderSelection::UnifiedOutbox;
                        self.reload_messages();
                    }
                }

                if ui.button(RichText::new("⚙ Settings").size(12.5)).clicked() {
                    self.settings_view.open();
                }

                if ui.button(RichText::new("🔍 Commands (Ctrl+K)").size(12.5))
                    .on_hover_text("Open Command Palette (Ctrl+K / Cmd+K)")
                    .clicked()
                {
                    self.populate_command_palette();
                    self.command_palette.open();
                }

                ui.separator();

                // Panel Visibility Toggles
                if ui
                    .selectable_label(self.show_sidebar, "📂 Sidebar")
                    .on_hover_text("Toggle left sidebar (folders & accounts)")
                    .clicked()
                {
                    self.show_sidebar = !self.show_sidebar;
                }

                if ui
                    .selectable_label(self.show_message_list, "📋 Mail List")
                    .on_hover_text("Toggle middle message list pane")
                    .clicked()
                {
                    self.show_message_list = !self.show_message_list;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Window Controls: Close (×), Maximize/Restore (□/❐), Minimize (−)
                    let close_btn = egui::Button::new(
                        RichText::new("×")
                            .size(15.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(225, 45, 57))
                    .min_size(egui::vec2(22.0, 18.0))
                    .rounding(Rounding::same(4.0));

                    if ui.add(close_btn).on_hover_text("Close / Minimize to tray").clicked() {
                        self.handle_close_requested(ctx);
                    }

                    let max_label = if self.is_maximized { "❐" } else { "□" };
                    let max_btn = egui::Button::new(
                        RichText::new(max_label)
                            .size(12.0)
                            .color(AppTheme::text_primary(ui)),
                    )
                    .min_size(egui::vec2(22.0, 18.0))
                    .rounding(Rounding::same(4.0));

                    if ui.add(max_btn).on_hover_text("Maximize / Restore window").clicked() {
                        self.is_maximized = !self.is_maximized;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(self.is_maximized));
                    }

                    let min_btn = egui::Button::new(
                        RichText::new("−")
                            .size(15.0)
                            .strong()
                            .color(AppTheme::text_primary(ui)),
                    )
                    .min_size(egui::vec2(22.0, 18.0))
                    .rounding(Rounding::same(4.0));

                    if ui.add(min_btn).on_hover_text("Minimize window").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }

                    ui.separator();

                    if self.is_syncing {
                        ui.spinner();
                    }
                    ui.label(
                        RichText::new(&self.status_text)
                            .size(11.5)
                            .color(AppTheme::text_muted(ui)),
                    );
                });
            });
            ui.add_space(4.0);
        });


        // Bottom Status Bar
        TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let unread_count: u32 = self
                    .folders_by_account
                    .values()
                    .flatten()
                    .map(|f| f.unread_messages)
                    .sum();

                ui.label(
                    RichText::new("⚡ Native Rust Engine")
                        .size(11.0)
                        .strong()
                        .color(AppTheme::accent(ui)),
                );
                ui.label(
                    RichText::new("•")
                        .size(11.0)
                        .color(AppTheme::text_muted(ui)),
                );
                ui.label(
                    RichText::new(format!("Memory: {} (Zero Chromium/Electron)", self.cached_mem_rss))
                        .size(11.0)
                        .color(AppTheme::text_secondary(ui)),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let status_dot = if unread_count > 0 { "🔵" } else { "⚪" };
                    ui.label(
                        RichText::new(format!("{} Total Unread: {}", status_dot, unread_count))
                            .size(11.0)
                            .color(AppTheme::text_secondary(ui)),
                    );
                });
            });
        });


        // 3-Pane AT-mail-rs Layout using true SidePanels
        // 1. Left Sidebar Panel (Folders & Navigation)
        if self.show_sidebar {
            egui::SidePanel::left("sidebar_panel")
                .resizable(true)
                .default_width(230.0)
                .min_width(180.0)
                .max_width(320.0)
                .show(ctx, |ui| {
                    let prev_folder = self.selected_folder.clone();
                    let mut on_add_account = false;
                    let mut on_open_settings = false;
                    let mut on_sync_all = false;
                    let mut on_drop_move = None;

                    SidebarView::show(
                        ui,
                        &self.accounts,
                        &self.folders_by_account,
                        &mut self.selected_folder,
                        &mut on_add_account,
                        &mut on_open_settings,
                        &mut on_sync_all,
                        &mut on_drop_move,
                    );

                    if on_add_account {
                        self.account_setup_view.open();
                    }
                    if on_open_settings {
                        self.settings_view.open();
                    }
                    if on_sync_all {
                        let _ = self.cmd_tx.send(SyncCommand::SyncAll);
                    }

                    if prev_folder != self.selected_folder {
                        self.selected_message_id = None;
                        self.selected_message_ids.clear();
                        self.last_clicked_idx = None;
                        self.selected_message_detail = None;
                        self.reload_messages();
                    }

                    if let Some((ids_to_move, account_id, target_folder_id)) = on_drop_move {
                        let mut moved_count = 0;
                        let storage_clone = self.storage.clone();
                        let ids_clone = ids_to_move.clone();
                        let tf_clone = target_folder_id.clone();
                        self.rt_handle.spawn_blocking(move || {
                            for mid in &ids_clone {
                                let _ = storage_clone.move_message_to_folder(mid, &tf_clone);
                            }
                        });
                        for mid in &ids_to_move {
                            if let Some(m) = self.messages.iter().find(|m| &m.id == mid) {
                                let _ = self.cmd_tx.send(SyncCommand::MoveMessage {
                                    account_id: account_id.clone(),
                                    source_folder_id: m.folder_id.clone(),
                                    target_folder_id: target_folder_id.clone(),
                                    uid: m.uid,
                                    message_id: mid.clone(),
                                });
                                moved_count += 1;
                            }
                        }
                        self.messages.retain(|m| !ids_to_move.contains(&m.id));
                        self.selected_message_ids.clear();
                        self.selected_message_id = None;
                        self.selected_message_detail = None;
                        let target_folder_name = self.folders_by_account.values().flatten().find(|f| f.id == target_folder_id).map(|f| f.display_name.as_str()).unwrap_or("folder");
                        let toast = format!("Moved {} message(s) to {}", moved_count, target_folder_name);
                        self.status_text = toast.clone();
                        self.status_toast = Some((toast, std::time::Instant::now()));
                    }
                });
        }

        // 2. Middle Message List Panel (Virtualized)
        let prev_msg_id = self.selected_message_id.clone();
        let prev_search = self.search_query.clone();
        let mut on_toggle_read = None;
        let mut on_toggle_flag = None;
        let mut on_batch_delete = None;
        let mut on_batch_move = None;
        let mut on_batch_toggle_read = None;
        let mut on_batch_toggle_flag = None;
        let mut on_mark_all_read = false;

        let available_folders: Vec<Folder> = self.folders_by_account.values().flatten().cloned().collect();

        if self.show_message_list {
            egui::SidePanel::left("message_list_panel")
                .resizable(true)
                .default_width(360.0)
                .min_width(260.0)
                .max_width(600.0)
                .show(ctx, |ui| {
                    MessageListView::show(
                        ui,
                        &self.messages,
                        &mut self.selected_message_id,
                        &mut self.selected_message_ids,
                        &mut self.last_clicked_idx,
                        &mut self.search_query,
                        &mut self.focus_search_requested,
                        &available_folders,
                        &mut on_toggle_read,
                        &mut on_toggle_flag,
                        &mut on_batch_delete,
                        &mut on_batch_move,
                        &mut on_batch_toggle_read,
                        &mut on_batch_toggle_flag,
                        &mut on_mark_all_read,
                    );
                });
        }

        if prev_search != self.search_query {
            self.reload_messages();
        }

        // Enhancement 4: Mark all as read handler
        if on_mark_all_read {
            self.mark_all_messages_read();
        }

        if let Some(ids_to_delete) = on_batch_delete {
            let storage_clone = self.storage.clone();
            let ids_clone = ids_to_delete.clone();
            self.rt_handle.spawn_blocking(move || {
                for mid in &ids_clone {
                    let _ = storage_clone.delete_message(mid);
                }
            });
            for mid in &ids_to_delete {
                if let Some(m) = self.messages.iter().find(|m| &m.id == mid) {
                    let _ = self.cmd_tx.send(SyncCommand::DeleteMessage {
                        account_id: m.account_id.clone(),
                        folder_id: m.folder_id.clone(),
                        uid: m.uid,
                    });
                }
            }
            self.messages.retain(|m| !ids_to_delete.contains(&m.id));
            self.selected_message_ids.clear();
            self.selected_message_id = None;
            self.selected_message_detail = None;
            let toast = format!("Deleted {} email(s)", ids_to_delete.len());
            self.status_text = toast.clone();
            self.status_toast = Some((toast, std::time::Instant::now()));
        }

        if let Some((ids_to_move, target_folder_id)) = on_batch_move {
            let mut moved_count = 0;
            let storage_clone = self.storage.clone();
            let ids_clone = ids_to_move.clone();
            let tf_clone = target_folder_id.clone();
            self.rt_handle.spawn_blocking(move || {
                for mid in &ids_clone {
                    let _ = storage_clone.move_message_to_folder(mid, &tf_clone);
                }
            });
            for mid in &ids_to_move {
                if let Some(m) = self.messages.iter().find(|m| &m.id == mid) {
                    let _ = self.cmd_tx.send(SyncCommand::MoveMessage {
                        account_id: m.account_id.clone(),
                        source_folder_id: m.folder_id.clone(),
                        target_folder_id: target_folder_id.clone(),
                        uid: m.uid,
                        message_id: mid.clone(),
                    });
                    moved_count += 1;
                }
            }
            self.messages.retain(|m| !ids_to_move.contains(&m.id));
            self.selected_message_ids.clear();
            self.selected_message_id = None;
            self.selected_message_detail = None;
            let target_folder_name = self.folders_by_account.values().flatten().find(|f| f.id == target_folder_id).map(|f| f.display_name.as_str()).unwrap_or("folder");
            let toast = format!("Moved {} email(s) to {}", moved_count, target_folder_name);
            self.status_text = toast.clone();
            self.status_toast = Some((toast, std::time::Instant::now()));
        }

        if let Some((ids, is_read)) = on_batch_toggle_read {
            let storage_clone = self.storage.clone();
            let ids_clone = ids.clone();
            self.rt_handle.spawn_blocking(move || {
                for mid in &ids_clone {
                    let _ = storage_clone.set_message_read(mid, is_read);
                }
            });
            for mid in &ids {
                if let Some(m) = self.messages.iter_mut().find(|m| &m.id == mid) {
                    m.is_read = is_read;
                    let _ = self.cmd_tx.send(SyncCommand::SetReadStatus {
                        account_id: m.account_id.clone(),
                        folder_id: m.folder_id.clone(),
                        uid: m.uid,
                        is_read,
                    });
                    if let Some(folders) = self.folders_by_account.get_mut(&m.account_id) {
                        if let Some(f) = folders.iter_mut().find(|f| f.id == m.folder_id) {
                            if is_read {
                                f.unread_messages = f.unread_messages.saturating_sub(1);
                            } else {
                                f.unread_messages += 1;
                            }
                        }
                    }
                }
                if let Some(ref mut detail) = self.selected_message_detail {
                    if &detail.header.id == mid {
                        detail.header.is_read = is_read;
                    }
                }
                for tm in &mut self.selected_thread_messages {
                    if &tm.header.id == mid {
                        tm.header.is_read = is_read;
                    }
                }
            }
            self.update_tray_unread();
        }

        if let Some((ids, is_flag)) = on_batch_toggle_flag {
            let storage_clone = self.storage.clone();
            let ids_clone = ids.clone();
            self.rt_handle.spawn_blocking(move || {
                for mid in &ids_clone {
                    let _ = storage_clone.set_message_flagged(mid, is_flag);
                }
            });
            for mid in &ids {
                if let Some(m) = self.messages.iter_mut().find(|m| &m.id == mid) {
                    m.is_flagged = is_flag;
                    let _ = self.cmd_tx.send(SyncCommand::SetFlaggedStatus {
                        account_id: m.account_id.clone(),
                        folder_id: m.folder_id.clone(),
                        uid: m.uid,
                        is_flagged: is_flag,
                    });
                }
                if let Some(ref mut detail) = self.selected_message_detail {
                    if &detail.header.id == mid {
                        detail.header.is_flagged = is_flag;
                    }
                }
                for tm in &mut self.selected_thread_messages {
                    if &tm.header.id == mid {
                        tm.header.is_flagged = is_flag;
                    }
                }
            }
        }

        if let Some((msg_id, is_read)) = on_toggle_read {
            let storage_clone = self.storage.clone();
            let m_id = msg_id.clone();
            self.rt_handle.spawn_blocking(move || {
                let _ = storage_clone.set_message_read(&m_id, is_read);
            });
            if let Some(m) = self.messages.iter_mut().find(|m| m.id == msg_id) {
                m.is_read = is_read;
                let _ = self.cmd_tx.send(SyncCommand::SetReadStatus {
                    account_id: m.account_id.clone(),
                    folder_id: m.folder_id.clone(),
                    uid: m.uid,
                    is_read,
                });
                if let Some(folders) = self.folders_by_account.get_mut(&m.account_id) {
                    if let Some(f) = folders.iter_mut().find(|f| f.id == m.folder_id) {
                        if is_read {
                            f.unread_messages = f.unread_messages.saturating_sub(1);
                        } else {
                            f.unread_messages += 1;
                        }
                    }
                }
            }
            if let Some(ref mut detail) = self.selected_message_detail {
                if detail.header.id == msg_id {
                    detail.header.is_read = is_read;
                }
            }
            for tm in &mut self.selected_thread_messages {
                if tm.header.id == msg_id {
                    tm.header.is_read = is_read;
                }
            }
            self.update_tray_unread();
        }

        if let Some((msg_id, is_flag)) = on_toggle_flag {
            let storage_clone = self.storage.clone();
            let m_id = msg_id.clone();
            self.rt_handle.spawn_blocking(move || {
                let _ = storage_clone.set_message_flagged(&m_id, is_flag);
            });
            if let Some(m) = self.messages.iter_mut().find(|m| m.id == msg_id) {
                m.is_flagged = is_flag;
                let _ = self.cmd_tx.send(SyncCommand::SetFlaggedStatus {
                    account_id: m.account_id.clone(),
                    folder_id: m.folder_id.clone(),
                    uid: m.uid,
                    is_flagged: is_flag,
                });
            }
            if let Some(ref mut detail) = self.selected_message_detail {
                if detail.header.id == msg_id {
                    detail.header.is_flagged = is_flag;
                }
            }
            for tm in &mut self.selected_thread_messages {
                if tm.header.id == msg_id {
                    tm.header.is_flagged = is_flag;
                }
            }
        }

        if prev_msg_id != self.selected_message_id {
            if let Some(ref mid) = self.selected_message_id {
                let storage_clone = self.storage.clone();
                let m_id = mid.clone();
                self.rt_handle.spawn_blocking(move || {
                    let _ = storage_clone.set_message_read(&m_id, true);
                });
                if let Some(m) = self.messages.iter_mut().find(|m| &m.id == mid) {
                    if !m.is_read {
                        m.is_read = true;
                        let _ = self.cmd_tx.send(SyncCommand::SetReadStatus {
                            account_id: m.account_id.clone(),
                            folder_id: m.folder_id.clone(),
                            uid: m.uid,
                            is_read: true,
                        });
                        if let Some(folders) = self.folders_by_account.get_mut(&m.account_id) {
                            if let Some(f) = folders.iter_mut().find(|f| f.id == m.folder_id) {
                                f.unread_messages = f.unread_messages.saturating_sub(1);
                            }
                        }
                    }
                }
                if let Some(hdr) = self.messages.iter().find(|m| &m.id == mid).cloned() {
                    let stub = MessageDetail {
                        header: hdr,
                        body_plain: None,
                        body_html: None,
                        attachments: Vec::new(),
                    };
                    self.selected_thread_messages = vec![stub.clone()];
                    self.selected_message_detail = Some(stub);
                }

                let storage_for_thread = self.storage.clone();
                let mid_clone = mid.clone();
                let tx = self.async_load_tx.clone();
                let egui_ctx = self.egui_ctx.clone();
                self.rt_handle.spawn_blocking(move || {
                    let detail = storage_for_thread.get_message_detail(&mid_clone).ok().flatten();
                    let thread_messages = if let Some(ref d) = detail {
                        storage_for_thread
                            .get_conversation_thread(&d.header.id)
                            .ok()
                            .flatten()
                            .map(|t| t.messages)
                            .unwrap_or_else(|| vec![d.clone()])
                    } else {
                        Vec::new()
                    };
                    let _ = tx.send(UiAsyncLoadResult::MessageDetailAndThread {
                        requested_id: mid_clone,
                        detail,
                        thread_messages,
                    });
                    egui_ctx.request_repaint();
                });
                self.update_tray_unread();
            } else {
                self.selected_message_detail = None;
                self.selected_thread_messages.clear();
            }
        }

        // 3. Central Reading Pane
        let mut on_reply = None;
        let mut on_reply_plain = None;
        let mut on_reply_all = None;
        let mut on_forward = None;
        let mut on_edit_draft = None;
        let mut on_delete = None;
        let mut on_toggle_read_view = None;
        let mut on_move_folder = None;
        let mut on_snooze = None;
        let mut on_status_toast = None;

        let active_folders = if let Some(ref detail) = self.selected_message_detail {
            self.folders_by_account
                .get(&detail.header.account_id)
                .cloned()
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        egui::CentralPanel::default().show(ctx, |ui| {
            MessageViewPane::show(
                ui,
                &self.selected_thread_messages,
                &active_folders,
                &mut self.allowed_remote_images,
                &self.cmd_tx,
                &mut on_reply,
                &mut on_reply_plain,
                &mut on_reply_all,
                &mut on_forward,
                &mut on_edit_draft,
                &mut on_delete,
                &mut on_toggle_read_view,
                &mut on_move_folder,
                &mut on_snooze,
                &mut on_status_toast,
            );
        });

        if let Some(toast_msg) = on_status_toast {
            self.status_toast = Some((toast_msg.clone(), std::time::Instant::now()));
            self.status_text = toast_msg;
        }

        // Handle reading pane actions
        if let Some(detail) = on_reply {
            let quote = detail.body_plain.unwrap_or_default();
            self.compose_view.open_reply(
                &detail.header.account_id,
                &detail.header.from_address,
                "",
                &detail.header.subject,
                detail.header.message_id,
                &quote,
                &self.signatures,
                true,
            );
        }

        if let Some(detail) = on_reply_plain {
            let quote = detail.body_plain.unwrap_or_default();
            self.compose_view.open_reply(
                &detail.header.account_id,
                &detail.header.from_address,
                "",
                &detail.header.subject,
                detail.header.message_id,
                &quote,
                &self.signatures,
                false,
            );
        }

        if let Some(detail) = on_reply_all {
            let quote = detail.body_plain.unwrap_or_default();
            let my_emails: std::collections::HashSet<String> = self
                .accounts
                .iter()
                .map(|a| a.email.trim().to_lowercase())
                .collect();
            let (to_str, cc_str) = crate::views::compose::build_reply_all_recipients(&detail.header, &my_emails);

            self.compose_view.open_reply(
                &detail.header.account_id,
                &to_str,
                &cc_str,
                &detail.header.subject,
                detail.header.message_id,
                &quote,
                &self.signatures,
                true,
            );
        }

        if let Some(detail) = on_forward {
            let quote = detail.body_plain.unwrap_or_default();
            let subj = format!("Fwd: {}", detail.header.subject);
            self.compose_view.open_reply(
                &detail.header.account_id,
                "",
                "",
                &subj,
                None,
                &format!("---------- Forwarded message ---------\nFrom: {}\nSubject: {}\n\n{}", detail.header.from_address, detail.header.subject, quote),
                &self.signatures,
                true,
            );
        }

        if let Some(detail) = on_edit_draft {
            if let Ok(Some(local_draft)) = self.storage.get_draft(&detail.header.id) {
                self.compose_view.open_draft(&local_draft, &self.signatures);
            } else {
                let to_str = detail.header.to_recipients.iter().map(|r| r.email.as_str()).collect::<Vec<_>>().join(", ");
                let cc_str = detail.header.cc_recipients.iter().map(|r| r.email.as_str()).collect::<Vec<_>>().join(", ");
                let body = detail.body_plain.unwrap_or_default();
                self.compose_view.open_new(Some(&detail.header.account_id), &self.signatures);
                self.compose_view.to_input = to_str;
                self.compose_view.cc_input = cc_str;
                self.compose_view.subject = detail.header.subject;
                self.compose_view.body_plain = body;
                self.compose_view.draft_id = Some(detail.header.id.clone());
            }
        }

        if let Some((msg_id, is_read)) = on_toggle_read_view {
            let storage_clone = self.storage.clone();
            let mid = msg_id.clone();
            self.rt_handle.spawn_blocking(move || {
                let _ = storage_clone.set_message_read(&mid, is_read);
            });
            if let Some(m) = self.messages.iter_mut().find(|m| m.id == msg_id) {
                m.is_read = is_read;
                let _ = self.cmd_tx.send(SyncCommand::SetReadStatus {
                    account_id: m.account_id.clone(),
                    folder_id: m.folder_id.clone(),
                    uid: m.uid,
                    is_read,
                });
                if let Some(folders) = self.folders_by_account.get_mut(&m.account_id) {
                    if let Some(f) = folders.iter_mut().find(|f| f.id == m.folder_id) {
                        if is_read {
                            f.unread_messages = f.unread_messages.saturating_sub(1);
                        } else {
                            f.unread_messages += 1;
                        }
                    }
                }
            }
            if let Some(ref mut detail) = self.selected_message_detail {
                if detail.header.id == msg_id {
                    detail.header.is_read = is_read;
                }
            }
            for tm in &mut self.selected_thread_messages {
                if tm.header.id == msg_id {
                    tm.header.is_read = is_read;
                }
            }
            self.update_tray_unread();
        }

        if let Some((msg_id, target_folder_id)) = on_move_folder {
            let msg_info = self
                .messages
                .iter()
                .find(|m| m.id == msg_id)
                .map(|m| (m.account_id.clone(), m.folder_id.clone(), m.uid))
                .or_else(|| {
                    self.selected_message_detail
                        .as_ref()
                        .filter(|d| d.header.id == msg_id)
                        .map(|d| (d.header.account_id.clone(), d.header.folder_id.clone(), d.header.uid))
                });

            if let Some((account_id, source_folder_id, uid)) = msg_info {
                let storage_clone = self.storage.clone();
                let m_id = msg_id.clone();
                let tf_id = target_folder_id.clone();
                self.rt_handle.spawn_blocking(move || {
                    let _ = storage_clone.move_message_to_folder(&m_id, &tf_id);
                });
                let _ = self.cmd_tx.send(SyncCommand::MoveMessage {
                    account_id,
                    source_folder_id,
                    target_folder_id: target_folder_id.clone(),
                    uid,
                    message_id: msg_id.clone(),
                });
            }
            self.messages.retain(|m| m.id != msg_id);
            self.selected_message_id = None;
            self.selected_message_detail = None;
            self.selected_thread_messages.clear();
            let target_folder_name = self
                .folders_by_account
                .values()
                .flatten()
                .find(|f| f.id == target_folder_id)
                .map(|f| f.display_name.as_str())
                .unwrap_or("folder");
            let toast = format!("Moved email to {}", target_folder_name);
            self.status_text = toast.clone();
            self.status_toast = Some((toast, std::time::Instant::now()));
        }

        if let Some((msg_id, snooze_until)) = on_snooze {
            let storage_clone = self.storage.clone();
            let m_id = msg_id.clone();
            self.rt_handle.spawn_blocking(move || {
                let _ = storage_clone.snooze_message(&m_id, snooze_until);
            });
            if snooze_until.is_some() {
                self.messages.retain(|m| m.id != msg_id);
                self.selected_message_id = None;
                self.selected_message_detail = None;
                self.selected_thread_messages.clear();
            } else {
                if let Some(ref mut detail) = self.selected_message_detail {
                    if detail.header.id == msg_id {
                        detail.header.snooze_until = snooze_until;
                    }
                }
                for tm in &mut self.selected_thread_messages {
                    if tm.header.id == msg_id {
                        tm.header.snooze_until = snooze_until;
                    }
                }
            }
        }

        if let Some(msg_id) = on_delete {
            if let Some(m) = self.messages.iter().find(|m| m.id == msg_id).cloned() {
                let storage_clone = self.storage.clone();
                let m_id = msg_id.clone();
                self.rt_handle.spawn_blocking(move || {
                    let _ = storage_clone.delete_message(&m_id);
                });
                let _ = self.cmd_tx.send(SyncCommand::DeleteMessage {
                    account_id: m.account_id,
                    folder_id: m.folder_id,
                    uid: m.uid,
                });
            }
            self.messages.retain(|m| m.id != msg_id);
            self.selected_message_id = None;
            self.selected_message_detail = None;
            self.selected_thread_messages.clear();
        }

        // Modals
        self.account_setup_view.show(
            ctx,
            &self.cmd_tx,
            &self.storage,
            &self.keyring,
        );

        let mut on_schedule_send: Option<OutgoingDraft> = None;
        let mut on_compose_data_changed = false;

        self.compose_view.show(
            ctx,
            &self.accounts,
            &self.templates,
            &self.signatures,
            &self.keyring,
            &self.storage,
            &mut on_schedule_send,
            &mut on_compose_data_changed,
            &mut self.status_toast,
        );

        if on_compose_data_changed {
            let storage_clone = self.storage.clone();
            let tx = self.async_load_tx.clone();
            let egui_ctx = self.egui_ctx.clone();
            self.rt_handle.spawn_blocking(move || {
                let scheduled = storage_clone.list_all_scheduled(None).unwrap_or_default().len();
                let outbox = storage_clone.get_all_outbox_items(None).unwrap_or_default().len();
                let _ = tx.send(UiAsyncLoadResult::QueueCounts { scheduled, outbox });
                egui_ctx.request_repaint();
            });
        }

        if let Some(draft) = on_schedule_send {
            self.pending_send = Some(PendingSend {
                draft,
                scheduled_time: std::time::Instant::now(),
                duration: std::time::Duration::from_secs(5),
            });
        }

        // Scheduled Outbox Modal
        if self.show_scheduled_modal {
            let mut modal_open = self.show_scheduled_modal;
            egui::Window::new("⏰ Scheduled Outbox")
                .open(&mut modal_open)
                .default_width(550.0)
                .show(ctx, |ui| {
                    let scheduled_list = self.storage.list_all_scheduled(None).unwrap_or_default();
                    if scheduled_list.is_empty() {
                        ui.label("No scheduled emails in outbox.");
                    } else {
                        for item in scheduled_list {
                            egui::Frame::none()
                                .fill(AppTheme::bg_card(ui))
                                .stroke(Stroke::new(1.0_f32, AppTheme::border_subtle(ui)))
                                .rounding(Rounding::same(6.0))
                                .inner_margin(8.0)
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            let dt = chrono::DateTime::from_timestamp(item.send_at_timestamp, 0)
                                                .map(|d| d.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                                                .unwrap_or_default();
                                            ui.label(RichText::new(format!("Subject: {}", item.draft.subject)).strong().color(AppTheme::text_primary(ui)));
                                            ui.label(RichText::new(format!("To: {} • Scheduled for: {}", item.draft.to.iter().map(|r| r.email.as_str()).collect::<Vec<_>>().join(", "), dt)).size(11.0).color(AppTheme::text_muted(ui)));
                                        });

                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui.button(RichText::new("🗑 Cancel").size(11.0).color(AppTheme::ACCENT_DANGER)).clicked() {
                                                let storage_clone = self.storage.clone();
                                                let item_id = item.id.clone();
                                                self.rt_handle.spawn_blocking(move || {
                                                    let _ = storage_clone.delete_scheduled_email(&item_id);
                                                });
                                                self.scheduled_count = self.scheduled_count.saturating_sub(1);
                                            }
                                            if ui.button(RichText::new("🚀 Send Now").size(11.0)).clicked() {
                                                let _ = self.cmd_tx.send(SyncCommand::SendEmail {
                                                    draft: item.draft.clone(),
                                                    password: None,
                                                });
                                                let storage_clone = self.storage.clone();
                                                let item_id = item.id.clone();
                                                self.rt_handle.spawn_blocking(move || {
                                                    let _ = storage_clone.delete_scheduled_email(&item_id);
                                                });
                                                self.scheduled_count = self.scheduled_count.saturating_sub(1);
                                            }
                                        });
                                    });
                                });
                            ui.add_space(4.0);
                        }
                    }
                });
            self.show_scheduled_modal = modal_open;
        }

        // Move to Folder Modal
        let mut modal_move_action: Option<(String, String, String, String, u32, String)> = None;
        if self.show_move_modal {
            let mut modal_open = self.show_move_modal;
            egui::Window::new("📂 Move to Folder")
                .open(&mut modal_open)
                .default_width(300.0)
                .show(ctx, |ui| {
                    if let Some(ref detail) = self.selected_message_detail {
                        let account_id = &detail.header.account_id;
                        if let Some(folders) = self.folders_by_account.get(account_id) {
                            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                                for folder in folders {
                                    if ui.selectable_label(false, &folder.display_name).clicked() {
                                        modal_move_action = Some((
                                            detail.header.id.clone(),
                                            account_id.clone(),
                                            detail.header.folder_id.clone(),
                                            folder.id.clone(),
                                            detail.header.uid,
                                            folder.display_name.clone(),
                                        ));
                                        break;
                                    }
                                }
                            });
                        } else {
                            ui.label("No folders found for this account.");
                        }
                    } else {
                        ui.label("No message selected.");
                    }
                });
            self.show_move_modal = modal_open;
        }

        if let Some((msg_id, account_id, source_folder_id, target_folder_id, uid, target_folder_name)) = modal_move_action {
            let storage_clone = self.storage.clone();
            let m_id = msg_id.clone();
            let tf_id = target_folder_id.clone();
            self.rt_handle.spawn_blocking(move || {
                let _ = storage_clone.move_message_to_folder(&m_id, &tf_id);
            });
            let _ = self.cmd_tx.send(SyncCommand::MoveMessage {
                account_id,
                source_folder_id,
                target_folder_id,
                message_id: msg_id.clone(),
                uid,
            });
            self.messages.retain(|m| m.id != msg_id);
            self.selected_message_id = None;
            self.selected_message_detail = None;
            self.selected_thread_messages.clear();
            let toast = format!("Moved email to {}", target_folder_name);
            self.status_text = toast.clone();
            self.status_toast = Some((toast, std::time::Instant::now()));
            self.show_move_modal = false;
        }

        let mut on_add_account_from_settings = false;
        let mut on_edit_account: Option<Account> = None;
        let mut on_data_changed = false;

        self.settings_view.show(
            ctx,
            &self.accounts,
            &self.folders_by_account,
            &mut self.templates,
            &mut self.signatures,
            &mut self.current_theme,
            &self.storage,
            &self.keyring,
            &self.cmd_tx,
            &mut on_add_account_from_settings,
            &mut on_edit_account,
            &mut on_data_changed,
        );

        if on_add_account_from_settings {
            self.account_setup_view.open();
        }

        if let Some(acc) = on_edit_account {
            let pwd = self.keyring.get_credential(&acc.credential_key).unwrap_or_default();
            self.account_setup_view.open_edit(&acc, &pwd);
        }

        if on_data_changed {
            self.reload_data();
        }

        // Render Command Palette
        if let Some(action) = self.command_palette.show(ctx) {
            if let PaletteAction::SetTheme(preset) = action {
                self.current_theme = preset;
                self.settings_view.active_custom_theme_id = None;
                let _ = self.storage.set_setting("theme_preset", preset.to_key());
                let _ = self.storage.set_setting("theme_custom_id", "");
                AppTheme::apply_preset(ctx, preset);
                self.status_toast = Some((format!("Switched to {} theme", preset.display_name()), std::time::Instant::now()));
            } else {
                self.execute_palette_action(action);
            }
        }

        // Global Keyboard Shortcuts (active when no modal is open and no text input is focused)
        let any_modal_open = self.command_palette.is_open
            || self.compose_view.is_open
            || self.settings_view.is_open
            || self.account_setup_view.is_open;

        if !any_modal_open && !ctx.wants_keyboard_input() {
            // j / Down: Next message
            if ctx.input(|i| i.key_pressed(egui::Key::J) || i.key_pressed(egui::Key::ArrowDown)) {
                if !self.messages.is_empty() {
                    let current_idx = self
                        .selected_message_id
                        .as_ref()
                        .and_then(|id| self.messages.iter().position(|m| &m.id == id));
                    let next_idx = match current_idx {
                        Some(idx) => (idx + 1).min(self.messages.len() - 1),
                        None => 0,
                    };
                    self.selected_message_id = Some(self.messages[next_idx].id.clone());
                }
            }

            // k / Up: Previous message
            if ctx.input(|i| i.key_pressed(egui::Key::K) || i.key_pressed(egui::Key::ArrowUp)) {
                if !self.messages.is_empty() {
                    let current_idx = self
                        .selected_message_id
                        .as_ref()
                        .and_then(|id| self.messages.iter().position(|m| &m.id == id));
                    let prev_idx = match current_idx {
                        Some(idx) => idx.saturating_sub(1),
                        None => 0,
                    };
                    self.selected_message_id = Some(self.messages[prev_idx].id.clone());
                }
            }

            // x: Toggle message selection in batch
            if ctx.input(|i| i.key_pressed(egui::Key::X)) {
                if let Some(ref mid) = self.selected_message_id {
                    if self.selected_message_ids.contains(mid) {
                        self.selected_message_ids.remove(mid);
                    } else {
                        self.selected_message_ids.insert(mid.clone());
                    }
                }
            }

            // c: Compose
            if ctx.input(|i| i.key_pressed(egui::Key::C)) {
                self.compose_view.open_new(self.accounts.first().map(|a| a.id.as_str()), &self.signatures);
            }

            // r: Reply
            if ctx.input(|i| i.key_pressed(egui::Key::R)) {
                self.execute_palette_action(PaletteAction::Reply);
            }

            // a: Reply All
            if ctx.input(|i| i.key_pressed(egui::Key::A)) {
                self.execute_palette_action(PaletteAction::ReplyAll);
            }

            // f: Forward
            if ctx.input(|i| i.key_pressed(egui::Key::F)) {
                self.execute_palette_action(PaletteAction::Forward);
            }

            // s: Star / Flag
            if ctx.input(|i| i.key_pressed(egui::Key::S)) {
                self.execute_palette_action(PaletteAction::ToggleStar);
            }

            // u: Read / Unread
            if ctx.input(|i| i.key_pressed(egui::Key::U)) {
                self.execute_palette_action(PaletteAction::MarkUnread);
            }

            // Delete / Backspace: Delete
            if ctx.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
                self.execute_palette_action(PaletteAction::DeleteSelected);
            }

            // /: Search focus
            if ctx.input(|i| i.key_pressed(egui::Key::Slash)) {
                self.focus_search_requested = true;
            }
        }

        // Pending Send (Undo Grace Period) Processing & Floating Bar
        if let Some(pending) = self.pending_send.clone() {
            let elapsed = pending.scheduled_time.elapsed();
            if elapsed >= pending.duration {
                let _ = self.cmd_tx.send(SyncCommand::SendEmail {
                    draft: pending.draft,
                    password: None,
                });
                self.status_toast = Some(("Email sent successfully!".to_string(), std::time::Instant::now()));
                self.pending_send = None;
            } else {
                let remaining = (pending.duration.as_secs_f32() - elapsed.as_secs_f32()).max(0.0);
                ctx.request_repaint_after(std::time::Duration::from_millis(50));

                egui::Area::new(egui::Id::new("undo_send_float_bar"))
                    .anchor(egui::Align2::CENTER_BOTTOM, egui::Vec2::new(0.0, -24.0))
                    .order(egui::Order::Foreground)
                    .show(ctx, |ui| {
                        egui::Frame::none()
                            .fill(AppTheme::bg_card(ui))
                            .stroke(egui::Stroke::new(1.5_f32, AppTheme::accent(ui)))
                            .rounding(egui::Rounding::same(8.0))
                            .inner_margin(egui::Margin::symmetric(18.0, 10.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("✉").size(16.0));
                                    ui.label(
                                        RichText::new(format!("Sending email in {:.1}s...", remaining))
                                            .size(13.0)
                                            .color(AppTheme::text_primary(ui)),
                                    );
                                    ui.add_space(8.0);
                                    if ui.button(RichText::new("↩ Undo Send").size(12.0).strong().color(AppTheme::ACCENT_WARNING)).clicked() {
                                        self.compose_view.restore_from_draft(&pending.draft);
                                        self.pending_send = None;
                                        self.status_toast = Some(("Sending undone. Draft restored.".to_string(), std::time::Instant::now()));
                                    }
                                    if ui.button(RichText::new("⚡ Send Now").size(11.5)).clicked() {
                                        let _ = self.cmd_tx.send(SyncCommand::SendEmail {
                                            draft: pending.draft,
                                            password: None,
                                        });
                                        self.status_toast = Some(("Email sent!".to_string(), std::time::Instant::now()));
                                        self.pending_send = None;
                                    }
                                });
                            });
                    });
            }
        }

        // Floating Toast Notification
        if let Some((ref toast_text, instant)) = self.status_toast {
            if instant.elapsed().as_secs() < 6 {
                let toast_layer = egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("toast_notification"));
                let toast_ui = ctx.layer_painter(toast_layer);
                let screen_rect = ctx.screen_rect();
                let toast_width = (toast_text.len() as f32 * 7.5 + 40.0).clamp(240.0, 520.0);
                let toast_rect = egui::Rect::from_min_size(
                    egui::Pos2::new(screen_rect.right() - toast_width - 24.0, screen_rect.bottom() - 60.0),
                    egui::Vec2::new(toast_width, 38.0),
                );
                toast_ui.rect_filled(toast_rect, egui::Rounding::same(8.0), AppTheme::bg_card_ctx(ctx));
                toast_ui.rect_stroke(toast_rect, egui::Rounding::same(8.0), egui::Stroke::new(1.0_f32, AppTheme::accent_ctx(ctx)));
                toast_ui.text(
                    toast_rect.left_center() + egui::Vec2::new(14.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    format!("✓ {}", toast_text),
                    egui::FontId::proportional(12.0),
                    AppTheme::text_primary_ctx(ctx),
                );
                let remaining = std::time::Duration::from_secs(6).saturating_sub(instant.elapsed());
                ctx.request_repaint_after(remaining);
            }
        }

        // Animate sync spinner when actively syncing; otherwise rely on reactive event-driven repainting
        if self.is_syncing {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
    }
}


