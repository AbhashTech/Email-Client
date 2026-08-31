use crate::theme::AppTheme;
use egui::{Color32, RichText, Rounding, ScrollArea, Stroke, Ui, Vec2, Window};
use email_core::events::SyncCommand;
use email_core::models::{Account, Folder, Signature, Template};
use email_keychain::CredentialStore;
use email_storage::Storage;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;


#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SettingsTab {
    Accounts,
    Signatures,
    Templates,
    Appearance,
    General,
}

pub struct SettingsView {
    pub is_open: bool,
    pub active_tab: SettingsTab,

    // Template Creator
    pub new_tpl_name: String,
    pub new_tpl_subject: String,
    pub new_tpl_body: String,
    pub new_tpl_shortcut: String,

    // Signature Creator
    pub new_sig_name: String,
    pub new_sig_html: String,
    pub new_sig_is_default: bool,

    // Feedback message
    pub status_msg: Option<(bool, String)>,
}

impl SettingsView {
    pub fn new() -> Self {
        Self {
            is_open: false,
            active_tab: SettingsTab::Accounts,
            new_tpl_name: String::new(),
            new_tpl_subject: String::new(),
            new_tpl_body: String::new(),
            new_tpl_shortcut: String::new(),
            new_sig_name: String::new(),
            new_sig_html: "<b>Best regards,</b><br/>My Name".to_string(),
            new_sig_is_default: false,
            status_msg: None,
        }
    }

    pub fn open(&mut self) {
        self.is_open = true;
        self.status_msg = None;
    }

    #[allow(dead_code)]
    pub fn open_tab(&mut self, tab: SettingsTab) {
        self.is_open = true;
        self.active_tab = tab;
        self.status_msg = None;
    }


    pub fn show(
        &mut self,
        ctx: &egui::Context,
        accounts: &[Account],
        folders_by_account: &HashMap<String, Vec<Folder>>,
        templates: &mut Vec<Template>,
        signatures: &mut Vec<Signature>,
        current_theme: &mut crate::theme::ThemePreset,
        storage: &Storage,
        keyring: &Arc<dyn CredentialStore>,
        cmd_tx: &mpsc::UnboundedSender<SyncCommand>,
        on_add_account: &mut bool,
        on_edit_account: &mut Option<Account>,
        on_data_changed: &mut bool,
    ) {
        if !self.is_open {
            return;
        }

        let mut is_open = self.is_open;
        Window::new("⚙ Preferences & Settings")
            .open(&mut is_open)
            .collapsible(false)
            .resizable(true)
            .default_width(740.0)
            .default_height(560.0)
            .show(ctx, |ui| {
                // Top Navigation Tabs
                ui.horizontal(|ui| {
                    Self::tab_button(ui, &mut self.active_tab, SettingsTab::Accounts, "📬 Accounts");
                    Self::tab_button(ui, &mut self.active_tab, SettingsTab::Signatures, "📝 Signatures");
                    Self::tab_button(ui, &mut self.active_tab, SettingsTab::Templates, "📋 Templates & Snippets");
                    Self::tab_button(ui, &mut self.active_tab, SettingsTab::Appearance, "🎨 Appearance");
                    Self::tab_button(ui, &mut self.active_tab, SettingsTab::General, "⚙ General & Storage");
                });

                ui.add_space(6.0);
                ui.painter().hline(
                    ui.available_rect_before_wrap().x_range(),
                    ui.cursor().top(),
                    Stroke::new(1.0_f32, AppTheme::BORDER_SUBTLE),
                );
                ui.add_space(8.0);

                if let Some((success, ref msg)) = self.status_msg {
                    let color = if success {
                        AppTheme::ACCENT_SUCCESS
                    } else {
                        AppTheme::ACCENT_DANGER
                    };
                    ui.label(RichText::new(msg).size(12.0).color(color));
                    ui.add_space(4.0);
                }

                ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                    match self.active_tab {
                        SettingsTab::Accounts => {
                            self.show_accounts_tab(
                                ui,
                                accounts,
                                folders_by_account,
                                storage,
                                keyring,
                                cmd_tx,
                                on_add_account,
                                on_edit_account,
                                on_data_changed,
                            );
                        }
                        SettingsTab::Signatures => {
                            self.show_signatures_tab(ui, accounts, signatures, storage, on_data_changed);
                        }
                        SettingsTab::Templates => {
                            self.show_templates_tab(ui, templates, storage, on_data_changed);
                        }
                        SettingsTab::Appearance => {
                            self.show_appearance_tab(ui, ctx, current_theme);
                        }
                        SettingsTab::General => {
                            self.show_general_tab(ui, accounts, storage);
                        }
                    }
                });
            });

        self.is_open = self.is_open && is_open;
    }


    fn tab_button(ui: &mut Ui, current_tab: &mut SettingsTab, target_tab: SettingsTab, title: &str) {
        let is_active = *current_tab == target_tab;
        let text = if is_active {
            RichText::new(title).strong().size(13.0).color(Color32::WHITE)
        } else {
            RichText::new(title).size(13.0).color(AppTheme::TEXT_SECONDARY)
        };

        let btn = egui::Button::new(text)
            .fill(if is_active { AppTheme::ACCENT_PRIMARY } else { AppTheme::BG_CARD })
            .rounding(Rounding::same(6.0));

        if ui.add(btn).clicked() {
            *current_tab = target_tab;
        }
    }

    fn show_accounts_tab(
        &mut self,
        ui: &mut Ui,
        accounts: &[Account],
        folders_by_account: &HashMap<String, Vec<Folder>>,
        storage: &Storage,
        keyring: &Arc<dyn CredentialStore>,
        cmd_tx: &mpsc::UnboundedSender<SyncCommand>,
        on_add_account: &mut bool,
        on_edit_account: &mut Option<Account>,
        on_data_changed: &mut bool,
    ) {
        ui.horizontal(|ui| {
            ui.heading(RichText::new("Configured Email Accounts").size(16.0).color(AppTheme::TEXT_PRIMARY));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let add_btn = egui::Button::new(
                    RichText::new("➕ Add New Account")
                        .size(12.0)
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(AppTheme::ACCENT_PRIMARY)
                .rounding(Rounding::same(6.0));

                if ui.add(add_btn).clicked() {
                    *on_add_account = true;
                }
            });
        });

        ui.add_space(10.0);

        if accounts.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(30.0);
                ui.label(RichText::new("No email accounts configured yet.").size(13.0).color(AppTheme::TEXT_MUTED));
                ui.add_space(6.0);
                if ui.button("➕ Set Up Your First Account").clicked() {
                    *on_add_account = true;
                }
            });
            return;
        }

        for acc in accounts {
            let folders_opt = folders_by_account.get(&acc.id);
            let folder_count = folders_opt.map(|f| f.len()).unwrap_or(0);
            let rows_needed = (folder_count + 2) / 3;
            let card_height = 100.0 + (rows_needed as f32 * 26.0) + 30.0;

            let card_rect = ui.available_rect_before_wrap();
            let (rect, _) = ui.allocate_exact_size(Vec2::new(card_rect.width(), card_height.max(130.0)), egui::Sense::hover());
            ui.painter().rect_filled(rect, Rounding::same(8.0), AppTheme::BG_CARD);
            ui.painter().rect_stroke(rect, Rounding::same(8.0), Stroke::new(1.0_f32, AppTheme::BORDER_SUBTLE));

            let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
            child_ui.horizontal(|ui| {
                ui.add_space(12.0);

                // Avatar
                let (avatar_rect, _) = ui.allocate_exact_size(Vec2::new(38.0, 38.0), egui::Sense::hover());
                ui.painter().circle_filled(avatar_rect.center(), 19.0, AppTheme::avatar_color(&acc.name));
                ui.painter().text(
                    avatar_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    AppTheme::get_initials(&acc.name),
                    egui::FontId::proportional(14.0),
                    Color32::WHITE,
                );

                ui.add_space(8.0);

                ui.vertical(|ui| {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&acc.name).size(14.0).strong().color(AppTheme::TEXT_PRIMARY));
                        ui.label(RichText::new(format!("<{}>", acc.email)).size(12.0).color(AppTheme::TEXT_MUTED));

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(12.0);
                            if ui.button(RichText::new("🗑 Remove").size(11.5).color(AppTheme::ACCENT_DANGER)).clicked() {
                                let _ = keyring.delete_credential(&acc.credential_key);
                                let _ = storage.delete_account(&acc.id);
                                *on_data_changed = true;
                            }
                            if ui.button(RichText::new("✏ Edit Details").size(11.5)).clicked() {
                                *on_edit_account = Some(acc.clone());
                            }
                            if ui.button(RichText::new("🔄 Sync Now").size(11.5)).clicked() {
                                let _ = cmd_tx.send(SyncCommand::SyncAccount { account_id: acc.id.clone() });
                            }
                        });
                    });

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("IMAP: {}:{} • SMTP: {}:{}", acc.imap_host, acc.imap_port, acc.smtp_host, acc.smtp_port)).size(11.5).color(AppTheme::TEXT_SECONDARY));
                        ui.add_space(12.0);
                        ui.label(RichText::new(format!("Sync Window: {}", acc.sync_days_window.label())).size(11.5).color(AppTheme::ACCENT_HOVER));
                    });

                    // Interactive Folder Sync Selector
                    if let Some(folders) = folders_opt {
                        ui.add_space(8.0);
                        ui.label(RichText::new("CHOOSE FOLDERS TO SYNC:").size(10.5).strong().color(AppTheme::TEXT_MUTED));
                        ui.add_space(4.0);

                        egui::Grid::new(format!("folders_grid_{}", acc.id))
                            .num_columns(3)
                            .spacing([18.0, 4.0])
                            .show(ui, |ui| {
                                for (idx, folder) in folders.iter().enumerate() {
                                    let mut is_synced = folder.is_synced;
                                    let label = format!("{} ({})", folder.display_name, folder.unread_messages);
                                    if ui.checkbox(&mut is_synced, label).changed() {
                                        let _ = storage.set_folder_sync_enabled(&folder.id, is_synced);
                                        *on_data_changed = true;
                                    }
                                    if (idx + 1) % 3 == 0 {
                                        ui.end_row();
                                    }
                                }
                            });
                    }
                });
            });

            ui.add_space(8.0);
        }

    }

    fn show_signatures_tab(
        &mut self,
        ui: &mut Ui,
        accounts: &[Account],
        signatures: &mut Vec<Signature>,
        storage: &Storage,
        on_data_changed: &mut bool,
    ) {
        ui.heading(RichText::new("Email Signatures").size(16.0).color(AppTheme::TEXT_PRIMARY));
        ui.add_space(8.0);

        // Signatures List
        if signatures.is_empty() {
            ui.label(RichText::new("No signatures created yet.").size(12.5).color(AppTheme::TEXT_MUTED));
        } else {
            for sig in signatures.iter() {
                ui.horizontal(|ui| {
                    let default_tag = if sig.is_default { " [Default]" } else { "" };
                    ui.label(RichText::new(format!("📝 {}{}", sig.name, default_tag)).strong().size(13.0));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(RichText::new("🗑 Delete").size(11.0).color(AppTheme::ACCENT_DANGER)).clicked() {
                            let _ = storage.delete_signature(&sig.id);
                            *on_data_changed = true;
                        }
                    });
                });
                ui.label(RichText::new(email_html::html_to_plain_text(&sig.content_html)).italics().size(11.5).color(AppTheme::TEXT_MUTED));
                ui.separator();
            }
        }

        ui.add_space(14.0);
        ui.label(RichText::new("CREATE NEW SIGNATURE").size(11.0).strong().color(AppTheme::TEXT_MUTED));
        ui.add_space(6.0);

        egui::Grid::new("new_signature_grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Signature Name:");
                ui.text_edit_singleline(&mut self.new_sig_name);
                ui.end_row();

                ui.label("HTML / Text Body:");
                ui.text_edit_multiline(&mut self.new_sig_html);
                ui.end_row();

                ui.label("Options:");
                ui.checkbox(&mut self.new_sig_is_default, "Set as Default Signature");
                ui.end_row();
            });

        ui.add_space(8.0);
        if ui.button("💾 Save Signature").clicked() {
            if self.new_sig_name.trim().is_empty() {
                self.status_msg = Some((false, "Signature name cannot be empty.".to_string()));
            } else {
                let sanitized_html = email_html::sanitize_raw_html(&self.new_sig_html);
                let sig = Signature::new(
                    accounts.first().map(|a| a.id.clone()),
                    self.new_sig_name.clone(),
                    sanitized_html,
                    self.new_sig_is_default,
                );
                let _ = storage.save_signature(&sig);
                self.new_sig_name.clear();
                self.status_msg = Some((true, "Signature saved successfully.".to_string()));
                *on_data_changed = true;
            }
        }
    }

    fn show_templates_tab(
        &mut self,
        ui: &mut Ui,
        templates: &mut Vec<Template>,
        storage: &Storage,
        on_data_changed: &mut bool,
    ) {
        ui.heading(RichText::new("Quick Templates & Snippets").size(16.0).color(AppTheme::TEXT_PRIMARY));
        ui.add_space(8.0);

        if templates.is_empty() {
            ui.label(RichText::new("No templates created yet.").size(12.5).color(AppTheme::TEXT_MUTED));
        } else {
            for tpl in templates.iter() {
                ui.horizontal(|ui| {
                    let sc = tpl.shortcut.as_deref().unwrap_or("");
                    let shortcut_tag = if !sc.is_empty() { format!(" ({})", sc) } else { "".to_string() };
                    ui.label(RichText::new(format!("📋 {}{}", tpl.name, shortcut_tag)).strong().size(13.0));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(RichText::new("🗑 Delete").size(11.0).color(AppTheme::ACCENT_DANGER)).clicked() {
                            let _ = storage.delete_template(&tpl.id);
                            *on_data_changed = true;
                        }
                    });
                });
                ui.label(RichText::new(&tpl.body_template).italics().size(11.5).color(AppTheme::TEXT_MUTED));
                ui.separator();
            }
        }

        ui.add_space(14.0);
        ui.label(RichText::new("CREATE NEW TEMPLATE").size(11.0).strong().color(AppTheme::TEXT_MUTED));
        ui.add_space(6.0);

        egui::Grid::new("new_tpl_grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Template Name:");
                ui.text_edit_singleline(&mut self.new_tpl_name);
                ui.end_row();

                ui.label("Quick Shortcut (e.g. /meeting):");
                ui.text_edit_singleline(&mut self.new_tpl_shortcut);
                ui.end_row();

                ui.label("Subject Template:");
                ui.text_edit_singleline(&mut self.new_tpl_subject);
                ui.end_row();

                ui.label("Body Content:");
                ui.text_edit_multiline(&mut self.new_tpl_body);
                ui.end_row();
            });

        ui.add_space(8.0);
        if ui.button("💾 Save Template").clicked() {
            if self.new_tpl_name.trim().is_empty() {
                self.status_msg = Some((false, "Template name cannot be empty.".to_string()));
            } else {
                let shortcut = if self.new_tpl_shortcut.is_empty() {
                    None
                } else {
                    Some(self.new_tpl_shortcut.clone())
                };
                let tpl = Template::new(
                    self.new_tpl_name.clone(),
                    self.new_tpl_subject.clone(),
                    self.new_tpl_body.clone(),
                    shortcut,
                );
                let _ = storage.save_template(&tpl);
                self.new_tpl_name.clear();
                self.new_tpl_subject.clear();
                self.new_tpl_body.clear();
                self.new_tpl_shortcut.clear();
                self.status_msg = Some((true, "Template saved successfully.".to_string()));
                *on_data_changed = true;
            }
        }
    }

    fn show_appearance_tab(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        current_theme: &mut crate::theme::ThemePreset,
    ) {
        ui.heading(RichText::new("Theme & Visual Style").size(16.0).color(AppTheme::TEXT_PRIMARY));
        ui.add_space(4.0);
        ui.label(RichText::new("Choose a theme preset tailored for productivity, low-light environments, or high-contrast OLED displays.").size(12.0).color(AppTheme::TEXT_MUTED));
        ui.add_space(14.0);

        for preset in crate::theme::ThemePreset::all() {
            let is_selected = *current_theme == *preset;
            let border_color = if is_selected { AppTheme::ACCENT_PRIMARY } else { AppTheme::BORDER_SUBTLE };

            egui::Frame::none()
                .fill(if is_selected { AppTheme::BG_HOVER } else { AppTheme::BG_CARD })
                .stroke(Stroke::new(if is_selected { 1.5_f32 } else { 1.0_f32 }, border_color))
                .rounding(Rounding::same(8.0))
                .inner_margin(12.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(preset.display_name()).size(13.5).strong().color(if is_selected { AppTheme::ACCENT_PRIMARY } else { AppTheme::TEXT_PRIMARY }));
                                if is_selected {
                                    ui.label(RichText::new("✓ Active").size(11.0).strong().color(AppTheme::ACCENT_SUCCESS));
                                }
                            });
                            ui.add_space(2.0);
                            ui.label(RichText::new(preset.description()).size(11.5).color(AppTheme::TEXT_MUTED));
                        });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if is_selected {
                                ui.label(RichText::new("Applied").size(12.0).color(AppTheme::ACCENT_PRIMARY));
                            } else if ui.button(RichText::new("Apply Theme").size(12.0)).clicked() {
                                *current_theme = *preset;
                                AppTheme::apply_preset(ctx, *preset);
                                self.status_msg = Some((true, format!("Switched to {} theme.", preset.display_name())));
                            }
                        });
                    });
                });

            ui.add_space(8.0);
        }
    }

    fn show_general_tab(&mut self, ui: &mut Ui, accounts: &[Account], _storage: &Storage) {

        ui.heading(RichText::new("Application & Storage").size(16.0).color(AppTheme::TEXT_PRIMARY));
        ui.add_space(10.0);

        ui.label(RichText::new("PERFORMANCE & ARCHITECTURE").size(11.0).strong().color(AppTheme::TEXT_MUTED));
        ui.add_space(4.0);
        ui.label("• Architecture: Fully native Rust multi-crate engine (Zero Chromium / Zero Electron)");
        ui.label("• GUI Toolkit: egui GPU-accelerated rendering (~38 MB base memory)");
        ui.label("• Cryptography: Rustls with Ring provider & OS-native Keyring integration");
        ui.label("• Database: Embedded SQLite in WAL (Write-Ahead Logging) mode");

        ui.add_space(14.0);
        ui.label(RichText::new("STORAGE STATS").size(11.0).strong().color(AppTheme::TEXT_MUTED));
        ui.add_space(4.0);
        ui.label(format!("• Accounts configured: {}", accounts.len()));
        ui.label("• SQLite WAL Mode: Enabled");

        ui.add_space(14.0);
        ui.label(RichText::new("SYSTEM TRAY").size(11.0).strong().color(AppTheme::TEXT_MUTED));
        ui.add_space(4.0);
        ui.label("• StatusNotifierItem DBus tray enabled with live unread badge updates.");
    }
}
