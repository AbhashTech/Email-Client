use crate::theme::AppTheme;
use egui::{Color32, RichText, Rounding, Window};
use email_core::events::SyncCommand;
use email_core::models::{Account, AuthType, Folder, SecurityType, SyncWindow};
use email_keychain::CredentialStore;
use email_storage::Storage;
use log::info;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct AccountSetupView {
    pub is_open: bool,
    pub editing_account: Option<Account>,
    pub name: String,
    pub email: String,
    pub password: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_security: SecurityType,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_security: SecurityType,
    pub sync_window: SyncWindow,
    pub custom_sync_days: i64,

    // Discovered folders for selective sync
    pub discovered_folders: Vec<Folder>,
    pub test_status_msg: Option<(bool, String)>,
    pub is_testing: bool,
}

impl AccountSetupView {
    pub fn new() -> Self {
        Self {
            is_open: false,
            editing_account: None,
            name: "My Account".to_string(),
            email: String::new(),
            password: String::new(),
            imap_host: String::new(),
            imap_port: 993,
            imap_security: SecurityType::Tls,
            smtp_host: String::new(),
            smtp_port: 465,
            smtp_security: SecurityType::Tls,
            sync_window: SyncWindow::Days30,
            custom_sync_days: 45,
            discovered_folders: Vec::new(),
            test_status_msg: None,
            is_testing: false,
        }
    }

    pub fn open(&mut self) {
        self.is_open = true;
        self.editing_account = None;
        self.name = "My Account".to_string();
        self.email.clear();
        self.password.clear();
        self.imap_host.clear();
        self.imap_port = 993;
        self.imap_security = SecurityType::Tls;
        self.smtp_host.clear();
        self.smtp_port = 465;
        self.smtp_security = SecurityType::Tls;
        self.sync_window = SyncWindow::Days30;
        self.custom_sync_days = 45;
        self.discovered_folders.clear();
        self.test_status_msg = None;
        self.is_testing = false;
    }

    pub fn open_edit(&mut self, account: &Account, existing_password: &str) {
        self.is_open = true;
        self.editing_account = Some(account.clone());
        self.name = account.name.clone();
        self.email = account.email.clone();
        self.password = existing_password.to_string();
        self.imap_host = account.imap_host.clone();
        self.imap_port = account.imap_port;
        self.imap_security = account.imap_security;
        self.smtp_host = account.smtp_host.clone();
        self.smtp_port = account.smtp_port;
        self.smtp_security = account.smtp_security;
        self.sync_window = account.sync_days_window;
        if let SyncWindow::Custom(d) = account.sync_days_window {
            self.custom_sync_days = d;
        } else {
            self.custom_sync_days = 45;
        }
        self.discovered_folders.clear();
        self.test_status_msg = None;
        self.is_testing = false;
    }

    pub fn auto_fill_presets(&mut self, email: &str) {
        let domain = email.split('@').nth(1).unwrap_or("");
        match domain.to_lowercase().as_str() {
            "gmail.com" | "googlemail.com" => {
                self.imap_host = "imap.gmail.com".to_string();
                self.imap_port = 993;
                self.imap_security = SecurityType::Tls;
                self.smtp_host = "smtp.gmail.com".to_string();
                self.smtp_port = 465;
                self.smtp_security = SecurityType::Tls;
            }
            "outlook.com" | "hotmail.com" | "live.com" => {
                self.imap_host = "outlook.office365.com".to_string();
                self.imap_port = 993;
                self.imap_security = SecurityType::Tls;
                self.smtp_host = "smtp.office365.com".to_string();
                self.smtp_port = 587;
                self.smtp_security = SecurityType::StartTls;
            }
            "yahoo.com" => {
                self.imap_host = "imap.mail.yahoo.com".to_string();
                self.imap_port = 993;
                self.imap_security = SecurityType::Tls;
                self.smtp_host = "smtp.mail.yahoo.com".to_string();
                self.smtp_port = 465;
                self.smtp_security = SecurityType::Tls;
            }
            _ => {}
        }
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        cmd_tx: &mpsc::UnboundedSender<SyncCommand>,
        storage: &Storage,
        keyring: &Arc<dyn CredentialStore>,
    ) {
        if !self.is_open {
            return;
        }

        let is_editing = self.editing_account.is_some();
        let title = if is_editing {
            "✏ Edit Email Account"
        } else {
            "⚙ Add Email Account"
        };

        let mut is_open = self.is_open;
        Window::new(title)
            .open(&mut is_open)
            .collapsible(false)
            .resizable(true)
            .default_width(580.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);

                // Security Banner
                ui.horizontal(|ui| {
                    ui.label(RichText::new("🔐").size(16.0));
                    ui.label(
                        RichText::new("Credentials stored securely in native OS Keyring (Zero plaintext in database)")
                            .size(12.0)
                            .color(AppTheme::accent_hover(ui)),
                    );
                });
                ui.add_space(8.0);

                // Provider Presets (only show if adding new)
                if !is_editing {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Presets:").size(12.0).color(AppTheme::text_muted(ui)));
                        if ui.button("🔴 Gmail").clicked() {
                            self.imap_host = "imap.gmail.com".to_string();
                            self.imap_port = 993;
                            self.imap_security = SecurityType::Tls;
                            self.smtp_host = "smtp.gmail.com".to_string();
                            self.smtp_port = 465;
                            self.smtp_security = SecurityType::Tls;
                        }
                        if ui.button("🔵 Outlook / 365").clicked() {
                            self.imap_host = "outlook.office365.com".to_string();
                            self.imap_port = 993;
                            self.imap_security = SecurityType::Tls;
                            self.smtp_host = "smtp.office365.com".to_string();
                            self.smtp_port = 587;
                            self.smtp_security = SecurityType::StartTls;
                        }
                        if ui.button("🟣 Yahoo").clicked() {
                            self.imap_host = "imap.mail.yahoo.com".to_string();
                            self.imap_port = 993;
                            self.imap_security = SecurityType::Tls;
                            self.smtp_host = "smtp.mail.yahoo.com".to_string();
                            self.smtp_port = 465;
                            self.smtp_security = SecurityType::Tls;
                        }
                    });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);
                }

                egui::Grid::new("account_form_grid")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Display Name:").size(12.5).color(AppTheme::text_secondary(ui)));
                        ui.text_edit_singleline(&mut self.name);
                        ui.end_row();

                        ui.label(RichText::new("Email Address:").size(12.5).color(AppTheme::text_secondary(ui)));
                        let email_response = ui.text_edit_singleline(&mut self.email);
                        if email_response.lost_focus() && self.imap_host.is_empty() {
                            self.auto_fill_presets(&self.email.clone());
                        }
                        ui.end_row();

                        ui.label(RichText::new("Password / App Key:").size(12.5).color(AppTheme::text_secondary(ui)));
                        ui.add(egui::TextEdit::singleline(&mut self.password).password(true));
                        ui.end_row();

                        ui.label(RichText::new("IMAP Server:").size(12.5).color(AppTheme::text_secondary(ui)));
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.imap_host);
                            ui.label("Port:");
                            ui.add(egui::DragValue::new(&mut self.imap_port));
                        });
                        ui.end_row();

                        ui.label(RichText::new("SMTP Server:").size(12.5).color(AppTheme::text_secondary(ui)));
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.smtp_host);
                            ui.label("Port:");
                            ui.add(egui::DragValue::new(&mut self.smtp_port));
                        });
                        ui.end_row();

                        ui.label(RichText::new("Sync Window:").size(12.5).color(AppTheme::text_secondary(ui)));
                        ui.horizontal(|ui| {
                            let is_custom = matches!(self.sync_window, SyncWindow::Custom(_));
                            let selected_text = self.sync_window.label();

                            egui::ComboBox::from_id_salt("sync_window_combo")
                                .selected_text(selected_text)
                                .show_ui(ui, |ui| {
                                    if ui.selectable_label(self.sync_window == SyncWindow::Days7, SyncWindow::Days7.label()).clicked() {
                                        self.sync_window = SyncWindow::Days7;
                                    }
                                    if ui.selectable_label(self.sync_window == SyncWindow::Days14, SyncWindow::Days14.label()).clicked() {
                                        self.sync_window = SyncWindow::Days14;
                                    }
                                    if ui.selectable_label(self.sync_window == SyncWindow::Days30, SyncWindow::Days30.label()).clicked() {
                                        self.sync_window = SyncWindow::Days30;
                                    }
                                    if ui.selectable_label(self.sync_window == SyncWindow::All, SyncWindow::All.label()).clicked() {
                                        self.sync_window = SyncWindow::All;
                                    }
                                    if ui.selectable_label(is_custom, "Custom days...").clicked() {
                                        self.sync_window = SyncWindow::Custom(60);
                                    }
                                });

                            if let SyncWindow::Custom(ref mut days) = self.sync_window {
                                ui.label("Days:");
                                ui.add(egui::DragValue::new(days).range(1..=3650));
                            }
                        });
                        ui.end_row();
                    });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(8.0);

                // Selective Folder Sync Checklist
                if !self.discovered_folders.is_empty() {
                    ui.label(RichText::new("SELECT FOLDERS TO SYNC:").size(11.0).strong().color(AppTheme::text_muted(ui)));
                    ui.add_space(4.0);

                    egui::ScrollArea::vertical()
                        .max_height(140.0)
                        .show(ui, |ui| {
                            for folder in &mut self.discovered_folders {
                                ui.checkbox(&mut folder.is_synced, &folder.display_name);
                            }
                        });
                    ui.add_space(6.0);
                }

                // Status message alert banner
                if let Some((success, ref msg)) = self.test_status_msg {
                    let color = if success {
                        AppTheme::ACCENT_SUCCESS
                    } else {
                        AppTheme::ACCENT_DANGER
                    };
                    ui.label(RichText::new(msg).size(12.0).color(color));
                }

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button("⚡ Test Connection").clicked() {
                        if self.email.is_empty() || self.password.is_empty() || self.imap_host.is_empty() {
                            self.test_status_msg = Some((false, "Please fill in Email, Password, and IMAP Host.".to_string()));
                        } else {
                            let dummy_account = Account::new(
                                self.name.clone(),
                                self.email.clone(),
                                self.imap_host.clone(),
                                self.imap_port,
                                self.imap_security,
                                self.smtp_host.clone(),
                                self.smtp_port,
                                self.smtp_security,
                                AuthType::Password,
                                self.sync_window,
                            );

                            let _ = cmd_tx.send(SyncCommand::TestConnection {
                                account: dummy_account,
                                password: self.password.clone(),
                            });

                            self.test_status_msg = Some((true, "Testing IMAP & SMTP server connections...".to_string()));
                        }
                    }


                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let btn_label = if is_editing { "💾 Update Account" } else { "💾 Save & Sync" };
                        let save_btn = egui::Button::new(
                            RichText::new(btn_label)
                                .size(13.0)
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(AppTheme::accent(ui))
                        .rounding(Rounding::same(6.0));

                        if ui.add(save_btn).clicked() {
                            if self.email.is_empty() || self.password.is_empty() || self.imap_host.is_empty() {
                                self.test_status_msg = Some((false, "Email, password, and IMAP host are required.".to_string()));
                            } else {
                                let account = if let Some(ref existing) = self.editing_account {
                                    let mut acc = existing.clone();
                                    acc.name = self.name.clone();
                                    acc.email = self.email.clone();
                                    acc.imap_host = self.imap_host.clone();
                                    acc.imap_port = self.imap_port;
                                    acc.imap_security = self.imap_security;
                                    acc.smtp_host = self.smtp_host.clone();
                                    acc.smtp_port = self.smtp_port;
                                    acc.smtp_security = self.smtp_security;
                                    acc.sync_days_window = self.sync_window;
                                    acc.updated_at = chrono::Utc::now().timestamp();
                                    acc
                                } else {
                                    Account::new(
                                        self.name.clone(),
                                        self.email.clone(),
                                        self.imap_host.clone(),
                                        self.imap_port,
                                        self.imap_security,
                                        self.smtp_host.clone(),
                                        self.smtp_port,
                                        self.smtp_security,
                                        AuthType::Password,
                                        self.sync_window,
                                    )
                                };

                                // Save secret securely in OS Keyring
                                if let Err(e) = keyring.set_credential(&account.credential_key, &self.password) {
                                    self.test_status_msg = Some((false, format!("Failed to save secret to OS Keyring: {}", e)));
                                } else if let Err(e) = storage.save_account(&account) {
                                    self.test_status_msg = Some((false, format!("Failed to save account to database: {}", e)));
                                } else {
                                    // Save any discovered folder preferences
                                    if !self.discovered_folders.is_empty() {
                                        for f in &mut self.discovered_folders {
                                            f.account_id = account.id.clone();
                                        }
                                        let _ = storage.save_folders(&self.discovered_folders);
                                    }

                                    // Trigger sync
                                    let _ = cmd_tx.send(SyncCommand::SyncAccount {
                                        account_id: account.id.clone(),
                                    });

                                    info!("Account {} saved successfully.", account.email);
                                    self.is_open = false;
                                }
                            }
                        }

                        if ui.button(RichText::new("Cancel").size(12.0)).clicked() {
                            self.is_open = false;
                        }
                    });
                });
            });

        self.is_open = self.is_open && is_open;
    }
}
