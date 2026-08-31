use crate::theme::AppTheme;
use crate::tray::AppTray;
use crate::views::*;
use eframe::App;
use egui::{Color32, RichText, Rounding, TopBottomPanel};
use email_core::events::{SyncCommand, SyncEvent};
use email_core::models::{Account, Folder, MessageDetail, MessageHeader, Signature, Template};
use email_keychain::CredentialStore;
use email_storage::Storage;
use log::error;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

pub struct EmailApp {
    storage: Storage,
    keyring: Arc<dyn CredentialStore>,
    cmd_tx: mpsc::UnboundedSender<SyncCommand>,
    event_rx: broadcast::Receiver<SyncEvent>,
    tray: Option<AppTray>,

    // Data State
    accounts: Vec<Account>,
    folders_by_account: HashMap<String, Vec<Folder>>,
    messages: Vec<MessageHeader>,
    selected_message_detail: Option<MessageDetail>,
    templates: Vec<Template>,
    signatures: Vec<Signature>,

    // UI Navigation State
    selected_folder: FolderSelection,
    selected_message_id: Option<String>,
    search_query: String,
    status_text: String,
    status_toast: Option<(String, std::time::Instant)>,
    is_syncing: bool,

    // Sub-views
    account_setup_view: AccountSetupView,
    compose_view: ComposeView,
    settings_view: SettingsView,
}


impl EmailApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        storage: Storage,
        keyring: Arc<dyn CredentialStore>,
        cmd_tx: mpsc::UnboundedSender<SyncCommand>,
        event_rx: broadcast::Receiver<SyncEvent>,
        rt_handle: tokio::runtime::Handle,
    ) -> Self {
        // Apply our custom sleek dark theme
        AppTheme::apply(&cc.egui_ctx);

        let tray = AppTray::new(cmd_tx.clone(), rt_handle);

        let mut app = Self {
            storage,
            keyring,
            cmd_tx,
            event_rx,
            tray: Some(tray),
            accounts: Vec::new(),
            folders_by_account: HashMap::new(),
            messages: Vec::new(),
            selected_message_detail: None,
            templates: Vec::new(),
            signatures: Vec::new(),
            selected_folder: FolderSelection::UnifiedUnread,
            selected_message_id: None,
            search_query: String::new(),
            status_text: "Ready".to_string(),
            status_toast: None,
            is_syncing: false,
            account_setup_view: AccountSetupView::new(),
            compose_view: ComposeView::new(),
            settings_view: SettingsView::new(),
        };


        app.reload_data();

        // If no accounts, open setup view immediately
        if app.accounts.is_empty() {
            app.account_setup_view.open();
        } else {
            // Auto-sync on startup
            let _ = app.cmd_tx.send(SyncCommand::SyncAll);
        }

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

        // Update unread count for system tray
        let total_unread: u32 = self
            .folders_by_account
            .values()
            .flatten()
            .map(|f| f.unread_messages)
            .sum();

        if let Some(ref tray) = self.tray {
            tray.update_unread_count(total_unread);
        }

        self.reload_messages();
    }

    pub fn reload_messages(&mut self) {
        let search = if self.search_query.is_empty() {
            None
        } else {
            Some(self.search_query.as_str())
        };

        match &self.selected_folder {
            FolderSelection::UnifiedFlagged | FolderSelection::UnifiedUnread => {
                if let Ok(mut msgs) = self.storage.get_messages(None, None, 500, 0, search) {
                    if matches!(self.selected_folder, FolderSelection::UnifiedFlagged) {
                        msgs.retain(|m| m.is_flagged);
                    } else if matches!(self.selected_folder, FolderSelection::UnifiedUnread) {
                        msgs.retain(|m| !m.is_read);
                    }
                    self.messages = msgs;
                }
            }
            FolderSelection::Folder {
                account_id,
                folder_id,
            } => {
                if let Ok(msgs) = self.storage.get_messages(
                    Some(account_id),
                    Some(folder_id),
                    500,
                    0,
                    search,
                ) {
                    self.messages = msgs;
                }
            }
        }
    }

    fn poll_background_events(&mut self) {
        // Poll Sync Events
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                SyncEvent::SyncStatusChanged {
                    is_syncing,
                    status_text,
                } => {
                    self.is_syncing = is_syncing;
                    self.status_text = status_text;
                }
                SyncEvent::FolderSynced { .. } => {
                    self.reload_data();
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
                    self.reload_data();
                }
            }
        }
    }

}

impl App for EmailApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_background_events();

        // Handle Ctrl+, / Cmd+, shortcut to open Settings
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Comma)) {
            self.settings_view.open();
        }

        // Top Navigation Bar
        TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let compose_btn = egui::Button::new(
                    RichText::new("✉ + Compose")
                        .size(12.5)
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(AppTheme::ACCENT_PRIMARY)
                .rounding(Rounding::same(6.0));

                if ui.add(compose_btn).clicked() {
                    self.compose_view.open_new(self.accounts.first().map(|a| a.id.as_str()), &self.signatures);
                }

                if ui.button(RichText::new("🔄 Sync All").size(12.5)).clicked() {
                    let _ = self.cmd_tx.send(SyncCommand::SyncAll);
                }

                if ui.button(RichText::new("⚙ Settings").size(12.5)).clicked() {
                    self.settings_view.open();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.is_syncing {
                        ui.spinner();
                    }
                    ui.label(
                        RichText::new(&self.status_text)
                            .size(11.5)
                            .color(AppTheme::TEXT_MUTED),
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
                        .color(AppTheme::ACCENT_PRIMARY),
                );
                ui.label(
                    RichText::new("•")
                        .size(11.0)
                        .color(AppTheme::TEXT_MUTED),
                );
                ui.label(
                    RichText::new("Memory: ~38 MB (Zero Chromium/Electron)")
                        .size(11.0)
                        .color(AppTheme::TEXT_SECONDARY),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let status_dot = if unread_count > 0 { "🔵" } else { "⚪" };
                    ui.label(
                        RichText::new(format!("{} Total Unread: {}", status_dot, unread_count))
                            .size(11.0)
                            .color(AppTheme::TEXT_SECONDARY),
                    );
                });
            });
        });


        // 3-Pane AT-mail-rs Layout using true SidePanels
        // 1. Left Sidebar Panel (Folders & Navigation)

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

                SidebarView::show(
                    ui,
                    &self.accounts,
                    &self.folders_by_account,
                    &mut self.selected_folder,
                    &mut on_add_account,
                    &mut on_open_settings,
                    &mut on_sync_all,
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
                    self.selected_message_detail = None;
                    self.reload_messages();
                }
            });

        // 2. Middle Message List Panel (Virtualized)
        let prev_msg_id = self.selected_message_id.clone();
        let prev_search = self.search_query.clone();
        let mut on_toggle_read = None;
        let mut on_toggle_flag = None;

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
                    &mut self.search_query,
                    &mut on_toggle_read,
                    &mut on_toggle_flag,
                );
            });

        if prev_search != self.search_query {
            self.reload_messages();
        }

        if let Some((msg_id, is_read)) = on_toggle_read {
            let _ = self.storage.set_message_read(&msg_id, is_read);
            if let Some(m) = self.messages.iter_mut().find(|m| m.id == msg_id) {
                m.is_read = is_read;
                let _ = self.cmd_tx.send(SyncCommand::SetReadStatus {
                    account_id: m.account_id.clone(),
                    folder_id: m.folder_id.clone(),
                    uid: m.uid,
                    is_read,
                });
            }
            if let Some(ref mut detail) = self.selected_message_detail {
                if detail.header.id == msg_id {
                    detail.header.is_read = is_read;
                }
            }
            self.reload_data();
        }

        if let Some((msg_id, is_flag)) = on_toggle_flag {
            let _ = self.storage.set_message_flagged(&msg_id, is_flag);
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
            self.reload_data();
        }

        if prev_msg_id != self.selected_message_id {
            if let Some(ref mid) = self.selected_message_id {
                let _ = self.storage.set_message_read(mid, true);
                if let Some(m) = self.messages.iter_mut().find(|m| &m.id == mid) {
                    if !m.is_read {
                        m.is_read = true;
                        let _ = self.cmd_tx.send(SyncCommand::SetReadStatus {
                            account_id: m.account_id.clone(),
                            folder_id: m.folder_id.clone(),
                            uid: m.uid,
                            is_read: true,
                        });
                    }
                }
                if let Ok(detail_opt) = self.storage.get_message_detail(mid) {
                    self.selected_message_detail = detail_opt;
                }
                self.reload_data();
            }
        }

        // 3. Central Reading Pane
        let mut on_reply = None;
        let mut on_reply_plain = None;
        let mut on_reply_all = None;
        let mut on_forward = None;
        let mut on_delete = None;
        let mut on_toggle_read_view = None;
        let mut on_move_folder = None;
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
                self.selected_message_detail.as_ref(),
                &active_folders,
                &self.cmd_tx,
                &mut on_reply,
                &mut on_reply_plain,
                &mut on_reply_all,
                &mut on_forward,
                &mut on_delete,
                &mut on_toggle_read_view,
                &mut on_move_folder,
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

        if let Some((msg_id, is_read)) = on_toggle_read_view {
            let _ = self.storage.set_message_read(&msg_id, is_read);
            if let Some(m) = self.messages.iter_mut().find(|m| m.id == msg_id) {
                m.is_read = is_read;
                let _ = self.cmd_tx.send(SyncCommand::SetReadStatus {
                    account_id: m.account_id.clone(),
                    folder_id: m.folder_id.clone(),
                    uid: m.uid,
                    is_read,
                });
            }
            if let Some(ref mut detail) = self.selected_message_detail {
                if detail.header.id == msg_id {
                    detail.header.is_read = is_read;
                }
            }
            self.reload_data();
        }

        if let Some((msg_id, target_folder_id)) = on_move_folder {
            if let Some(m) = self.messages.iter().find(|m| m.id == msg_id).cloned() {
                let _ = self.storage.move_message_to_folder(&msg_id, &target_folder_id);
                let _ = self.cmd_tx.send(SyncCommand::MoveMessage {
                    account_id: m.account_id,
                    source_folder_id: m.folder_id,
                    target_folder_id,
                    uid: m.uid,
                    message_id: msg_id.clone(),
                });
            }
            self.selected_message_id = None;
            self.selected_message_detail = None;
            self.reload_data();
        }

        if let Some(msg_id) = on_delete {
            if let Some(m) = self.messages.iter().find(|m| m.id == msg_id).cloned() {
                let _ = self.storage.delete_message(&msg_id);
                let _ = self.cmd_tx.send(SyncCommand::DeleteMessage {
                    account_id: m.account_id,
                    folder_id: m.folder_id,
                    uid: m.uid,
                });
            }
            self.selected_message_id = None;
            self.selected_message_detail = None;
            self.reload_data();
        }

        // Modals
        self.account_setup_view.show(
            ctx,
            &self.cmd_tx,
            &self.storage,
            &self.keyring,
        );

        self.compose_view.show(
            ctx,
            &self.accounts,
            &self.templates,
            &self.signatures,
            &self.cmd_tx,
            &self.keyring,
        );

        let mut on_add_account_from_settings = false;
        let mut on_edit_account: Option<Account> = None;
        let mut on_data_changed = false;

        self.settings_view.show(
            ctx,
            &self.accounts,
            &self.folders_by_account,
            &mut self.templates,
            &mut self.signatures,
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
                toast_ui.rect_filled(toast_rect, egui::Rounding::same(8.0), egui::Color32::from_rgb(20, 30, 48));
                toast_ui.rect_stroke(toast_rect, egui::Rounding::same(8.0), egui::Stroke::new(1.0_f32, AppTheme::ACCENT_PRIMARY));
                toast_ui.text(
                    toast_rect.left_center() + egui::Vec2::new(14.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    format!("✓ {}", toast_text),
                    egui::FontId::proportional(12.0),
                    egui::Color32::WHITE,
                );
                ctx.request_repaint_after(std::time::Duration::from_millis(500));
            }
        }

        // Continuous redraw when syncing
        if self.is_syncing {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}


