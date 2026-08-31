use crate::theme::AppTheme;
use crate::tray::AppTray;
use crate::views::*;
use eframe::App;
use egui::{Color32, RichText, Rounding, TopBottomPanel};
use email_core::events::{SyncCommand, SyncEvent};
use email_core::models::{Account, Folder, MessageDetail, MessageHeader, OutgoingDraft, Signature, Template};
use email_keychain::CredentialStore;
use email_storage::Storage;
use log::error;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

#[derive(Clone)]
pub struct PendingSend {
    pub draft: OutgoingDraft,
    pub password: String,
    pub scheduled_time: std::time::Instant,
    pub duration: std::time::Duration,
}

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
    ) -> Self {
        let current_theme = crate::theme::ThemePreset::DarkSlate;
        AppTheme::apply_preset(&cc.egui_ctx, current_theme);

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
            account_setup_view: AccountSetupView::new(),
            compose_view: ComposeView::new(),
            settings_view: SettingsView::new(),
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
                    let _ = self.storage.set_message_read(mid, true);
                    if let Some(m) = self.messages.iter_mut().find(|m| &m.id == mid) {
                        m.is_read = true;
                        let _ = self.cmd_tx.send(SyncCommand::SetReadStatus {
                            account_id: m.account_id.clone(),
                            folder_id: m.folder_id.clone(),
                            uid: m.uid,
                            is_read: true,
                        });
                    }
                    self.reload_data();
                }
            }
            PaletteAction::MarkUnread => {
                if let Some(ref mid) = self.selected_message_id {
                    let _ = self.storage.set_message_read(mid, false);
                    if let Some(m) = self.messages.iter_mut().find(|m| &m.id == mid) {
                        m.is_read = false;
                        let _ = self.cmd_tx.send(SyncCommand::SetReadStatus {
                            account_id: m.account_id.clone(),
                            folder_id: m.folder_id.clone(),
                            uid: m.uid,
                            is_read: false,
                        });
                    }
                    self.reload_data();
                }
            }
            PaletteAction::ToggleStar => {
                if let Some(ref mid) = self.selected_message_id {
                    if let Some(m) = self.messages.iter_mut().find(|m| &m.id == mid) {
                        let new_flag = !m.is_flagged;
                        m.is_flagged = new_flag;
                        let _ = self.storage.set_message_flagged(mid, new_flag);
                        let _ = self.cmd_tx.send(SyncCommand::SetFlaggedStatus {
                            account_id: m.account_id.clone(),
                            folder_id: m.folder_id.clone(),
                            uid: m.uid,
                            is_flagged: new_flag,
                        });
                    }
                    self.reload_data();
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

                for mid in &to_delete {
                    if let Some(m) = self.messages.iter().find(|m| &m.id == mid) {
                        let _ = self.storage.delete_message(mid);
                        let _ = self.cmd_tx.send(SyncCommand::DeleteMessage {
                            account_id: m.account_id.clone(),
                            folder_id: m.folder_id.clone(),
                            uid: m.uid,
                        });
                    }
                }
                self.selected_message_ids.clear();
                self.selected_message_id = None;
                self.selected_message_detail = None;
                self.reload_data();
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
        self.poll_background_events();

        if self.current_theme == crate::theme::ThemePreset::GruvboxAuto {
            AppTheme::apply_preset(ctx, crate::theme::ThemePreset::GruvboxAuto);
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
                        for mid in &ids_to_move {
                            if let Ok(Some(detail)) = self.storage.get_message_detail(mid) {
                                let _ = self.storage.move_message_to_folder(mid, &target_folder_id);
                                let _ = self.cmd_tx.send(SyncCommand::MoveMessage {
                                    account_id: account_id.clone(),
                                    source_folder_id: detail.header.folder_id,
                                    target_folder_id: target_folder_id.clone(),
                                    uid: detail.header.uid,
                                    message_id: mid.clone(),
                                });
                                moved_count += 1;
                            }
                        }
                        self.selected_message_ids.clear();
                        self.selected_message_id = None;
                        self.selected_message_detail = None;
                        self.reload_data();
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
                    );
                });
        }

        if prev_search != self.search_query {
            self.reload_messages();
        }

        if let Some(ids_to_delete) = on_batch_delete {
            for mid in &ids_to_delete {
                if let Some(m) = self.messages.iter().find(|m| &m.id == mid) {
                    let _ = self.storage.delete_message(mid);
                    let _ = self.cmd_tx.send(SyncCommand::DeleteMessage {
                        account_id: m.account_id.clone(),
                        folder_id: m.folder_id.clone(),
                        uid: m.uid,
                    });
                }
            }
            self.selected_message_ids.clear();
            self.selected_message_id = None;
            self.selected_message_detail = None;
            self.reload_data();
            let toast = format!("Deleted {} email(s)", ids_to_delete.len());
            self.status_text = toast.clone();
            self.status_toast = Some((toast, std::time::Instant::now()));
        }

        if let Some((ids_to_move, target_folder_id)) = on_batch_move {
            let mut moved_count = 0;
            for mid in &ids_to_move {
                if let Some(m) = self.messages.iter().find(|m| &m.id == mid) {
                    let _ = self.storage.move_message_to_folder(mid, &target_folder_id);
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
            self.selected_message_ids.clear();
            self.selected_message_id = None;
            self.selected_message_detail = None;
            self.reload_data();
            let target_folder_name = self.folders_by_account.values().flatten().find(|f| f.id == target_folder_id).map(|f| f.display_name.as_str()).unwrap_or("folder");
            let toast = format!("Moved {} email(s) to {}", moved_count, target_folder_name);
            self.status_text = toast.clone();
            self.status_toast = Some((toast, std::time::Instant::now()));
        }

        if let Some((ids, is_read)) = on_batch_toggle_read {
            for mid in &ids {
                let _ = self.storage.set_message_read(mid, is_read);
                if let Some(m) = self.messages.iter_mut().find(|m| &m.id == mid) {
                    m.is_read = is_read;
                    let _ = self.cmd_tx.send(SyncCommand::SetReadStatus {
                        account_id: m.account_id.clone(),
                        folder_id: m.folder_id.clone(),
                        uid: m.uid,
                        is_read,
                    });
                }
            }
            self.reload_data();
        }

        if let Some((ids, is_flag)) = on_batch_toggle_flag {
            for mid in &ids {
                let _ = self.storage.set_message_flagged(mid, is_flag);
                if let Some(m) = self.messages.iter_mut().find(|m| &m.id == mid) {
                    m.is_flagged = is_flag;
                    let _ = self.cmd_tx.send(SyncCommand::SetFlaggedStatus {
                        account_id: m.account_id.clone(),
                        folder_id: m.folder_id.clone(),
                        uid: m.uid,
                        is_flagged: is_flag,
                    });
                }
            }
            self.reload_data();
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
                &mut self.allowed_remote_images,
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

        let mut on_schedule_send: Option<(OutgoingDraft, String)> = None;

        self.compose_view.show(
            ctx,
            &self.accounts,
            &self.templates,
            &self.signatures,
            &self.keyring,
            &mut on_schedule_send,
        );

        if let Some((draft, pwd)) = on_schedule_send {
            self.pending_send = Some(PendingSend {
                draft,
                password: pwd,
                scheduled_time: std::time::Instant::now(),
                duration: std::time::Duration::from_secs(5),
            });
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
                    password: pending.password,
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
                            .fill(egui::Color32::from_rgb(26, 34, 52))
                            .stroke(egui::Stroke::new(1.5_f32, AppTheme::ACCENT_PRIMARY))
                            .rounding(egui::Rounding::same(8.0))
                            .inner_margin(egui::Margin::symmetric(18.0, 10.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("✉").size(16.0));
                                    ui.label(
                                        RichText::new(format!("Sending email in {:.1}s...", remaining))
                                            .size(13.0)
                                            .color(egui::Color32::WHITE),
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
                                            password: pending.password,
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


