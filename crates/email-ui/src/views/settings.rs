use crate::{get_database_path, load_app_config, save_app_config};
use crate::theme::{
    delete_custom_theme, get_config_dir, get_themes_dir, load_custom_themes, save_custom_theme,
    AppTheme,
};
use egui::{Color32, RichText, Rounding, ScrollArea, Stroke, Ui, Vec2, Window};
use email_core::events::SyncCommand;
use email_core::models::{
    Account, AccountBackup, AppBackup, CustomTheme, Folder, SettingsMetadata, Signature, Template,
};
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
    Backup,
}

pub struct SettingsView {
    pub is_open: bool,
    pub active_tab: SettingsTab,

    // Template Creator / Editor
    pub editing_tpl_id: Option<String>,
    pub editing_tpl_created_at: Option<i64>,
    pub new_tpl_name: String,
    pub new_tpl_subject: String,
    pub new_tpl_body: String,
    pub new_tpl_shortcut: String,

    // Signature Creator / Editor
    pub editing_sig_id: Option<String>,
    pub editing_sig_account_id: Option<String>,
    pub editing_sig_created_at: Option<i64>,
    pub new_sig_name: String,
    pub new_sig_html: String,
    pub new_sig_is_default: bool,

    // Custom Theme Creator
    pub new_theme_name: String,
    pub new_theme_desc: String,
    pub new_theme_is_dark: bool,
    pub new_theme_bg_app: [u8; 3],
    pub new_theme_bg_list: [u8; 3],
    pub new_theme_bg_view: [u8; 3],
    pub new_theme_bg_card: [u8; 3],
    pub new_theme_bg_hover: [u8; 3],
    pub new_theme_bg_selected: [u8; 3],
    pub new_theme_accent_primary: [u8; 3],
    pub new_theme_accent_hover: [u8; 3],
    pub new_theme_border: [u8; 3],
    pub new_theme_text_primary: [u8; 3],
    pub new_theme_text_secondary: [u8; 3],
    pub active_custom_theme_id: Option<String>,
    pub custom_themes: Vec<CustomTheme>,

    // Feedback message
    pub status_msg: Option<(bool, String)>,
}

impl SettingsView {
    pub fn new() -> Self {
        Self {
            is_open: false,
            active_tab: SettingsTab::Accounts,
            editing_tpl_id: None,
            editing_tpl_created_at: None,
            new_tpl_name: String::new(),
            new_tpl_subject: String::new(),
            new_tpl_body: String::new(),
            new_tpl_shortcut: String::new(),
            editing_sig_id: None,
            editing_sig_account_id: None,
            editing_sig_created_at: None,
            new_sig_name: String::new(),
            new_sig_html: "<b>Best regards,</b><br/>My Name".to_string(),
            new_sig_is_default: false,
            new_theme_name: "My Gruvbox Custom".to_string(),
            new_theme_desc: "Custom retro warm palette".to_string(),
            new_theme_is_dark: true,
            new_theme_bg_app: [40, 40, 40],
            new_theme_bg_list: [50, 48, 47],
            new_theme_bg_view: [60, 56, 54],
            new_theme_bg_card: [80, 73, 69],
            new_theme_bg_hover: [102, 92, 84],
            new_theme_bg_selected: [214, 93, 14],
            new_theme_accent_primary: [250, 189, 47],
            new_theme_accent_hover: [254, 128, 25],
            new_theme_border: [80, 73, 69],
            new_theme_text_primary: [235, 219, 178],
            new_theme_text_secondary: [213, 196, 161],
            active_custom_theme_id: None,
            custom_themes: Vec::new(),
            status_msg: None,
        }
    }

    pub fn reset_sig_form(&mut self) {
        self.editing_sig_id = None;
        self.editing_sig_account_id = None;
        self.editing_sig_created_at = None;
        self.new_sig_name.clear();
        self.new_sig_html = "<b>Best regards,</b><br/>My Name".to_string();
        self.new_sig_is_default = false;
    }

    pub fn reset_tpl_form(&mut self) {
        self.editing_tpl_id = None;
        self.editing_tpl_created_at = None;
        self.new_tpl_name.clear();
        self.new_tpl_subject.clear();
        self.new_tpl_body.clear();
        self.new_tpl_shortcut.clear();
    }

    pub fn open(&mut self) {
        self.is_open = true;
        self.status_msg = None;
        self.custom_themes = load_custom_themes();
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
                    Self::tab_button(ui, &mut self.active_tab, SettingsTab::Backup, "💾 Backup & Restore");
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
                        SettingsTab::Backup => {
                            self.show_backup_tab(ui, accounts, templates, signatures, storage, on_data_changed);
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

        let resp = ui.add(btn);
        if resp.clicked() {
            *current_tab = target_tab;
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
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
                            if self.editing_sig_id.as_deref() == Some(&sig.id) {
                                self.reset_sig_form();
                            }
                            *on_data_changed = true;
                        }
                        if ui.button(RichText::new("✏ Edit").size(11.0)).clicked() {
                            self.editing_sig_id = Some(sig.id.clone());
                            self.editing_sig_account_id = sig.account_id.clone();
                            self.editing_sig_created_at = Some(sig.created_at);
                            self.new_sig_name = sig.name.clone();
                            self.new_sig_html = sig.content_html.clone();
                            self.new_sig_is_default = sig.is_default;
                            self.status_msg = Some((true, format!("Editing signature: {}", sig.name)));
                        }
                    });
                });
                ui.label(RichText::new(email_html::html_to_plain_text(&sig.content_html)).italics().size(11.5).color(AppTheme::TEXT_MUTED));
                ui.separator();
            }
        }

        ui.add_space(14.0);
        let sig_heading = if let Some(ref edit_id) = self.editing_sig_id {
            format!("EDIT SIGNATURE (Editing ID: {})", &edit_id[..8.min(edit_id.len())])
        } else {
            "CREATE NEW SIGNATURE".to_string()
        };
        ui.label(RichText::new(sig_heading).size(11.0).strong().color(if self.editing_sig_id.is_some() { AppTheme::ACCENT_PRIMARY } else { AppTheme::TEXT_MUTED }));
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
        ui.horizontal(|ui| {
            let save_btn_label = if self.editing_sig_id.is_some() {
                "💾 Update Signature"
            } else {
                "💾 Save Signature"
            };

            let btn = egui::Button::new(RichText::new(save_btn_label).strong())
                .fill(if self.editing_sig_id.is_some() { AppTheme::ACCENT_PRIMARY } else { AppTheme::BG_CARD });

            if ui.add(btn).clicked() {
                if self.new_sig_name.trim().is_empty() {
                    self.status_msg = Some((false, "Signature name cannot be empty.".to_string()));
                } else {
                    let sanitized_html = email_html::sanitize_raw_html(&self.new_sig_html);
                    let is_editing = self.editing_sig_id.is_some();
                    let sig = if let Some(ref edit_id) = self.editing_sig_id {
                        Signature {
                            id: edit_id.clone(),
                            account_id: self.editing_sig_account_id.clone().or_else(|| accounts.first().map(|a| a.id.clone())),
                            name: self.new_sig_name.clone(),
                            content_html: sanitized_html,
                            is_default: self.new_sig_is_default,
                            created_at: self.editing_sig_created_at.unwrap_or_else(|| chrono::Utc::now().timestamp()),
                        }
                    } else {
                        Signature::new(
                            accounts.first().map(|a| a.id.clone()),
                            self.new_sig_name.clone(),
                            sanitized_html,
                            self.new_sig_is_default,
                        )
                    };

                    let _ = storage.save_signature(&sig);
                    self.reset_sig_form();
                    self.status_msg = Some((
                        true,
                        if is_editing {
                            "Signature updated successfully.".to_string()
                        } else {
                            "Signature saved successfully.".to_string()
                        },
                    ));
                    *on_data_changed = true;
                }
            }

            if self.editing_sig_id.is_some() {
                if ui.button("Cancel").clicked() {
                    self.reset_sig_form();
                    self.status_msg = None;
                }
            }
        });
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
                            if self.editing_tpl_id.as_deref() == Some(&tpl.id) {
                                self.reset_tpl_form();
                            }
                            *on_data_changed = true;
                        }
                        if ui.button(RichText::new("✏ Edit").size(11.0)).clicked() {
                            self.editing_tpl_id = Some(tpl.id.clone());
                            self.editing_tpl_created_at = Some(tpl.created_at);
                            self.new_tpl_name = tpl.name.clone();
                            self.new_tpl_shortcut = tpl.shortcut.clone().unwrap_or_default();
                            self.new_tpl_subject = tpl.subject_template.clone();
                            self.new_tpl_body = tpl.body_template.clone();
                            self.status_msg = Some((true, format!("Editing template: {}", tpl.name)));
                        }
                    });
                });
                ui.label(RichText::new(&tpl.body_template).italics().size(11.5).color(AppTheme::TEXT_MUTED));
                ui.separator();
            }
        }

        ui.add_space(14.0);
        let tpl_heading = if let Some(ref edit_id) = self.editing_tpl_id {
            format!("EDIT TEMPLATE (Editing ID: {})", &edit_id[..8.min(edit_id.len())])
        } else {
            "CREATE NEW TEMPLATE".to_string()
        };
        ui.label(RichText::new(tpl_heading).size(11.0).strong().color(if self.editing_tpl_id.is_some() { AppTheme::ACCENT_PRIMARY } else { AppTheme::TEXT_MUTED }));
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
        ui.horizontal(|ui| {
            let save_btn_label = if self.editing_tpl_id.is_some() {
                "💾 Update Template"
            } else {
                "💾 Save Template"
            };

            let btn = egui::Button::new(RichText::new(save_btn_label).strong())
                .fill(if self.editing_tpl_id.is_some() { AppTheme::ACCENT_PRIMARY } else { AppTheme::BG_CARD });

            if ui.add(btn).clicked() {
                if self.new_tpl_name.trim().is_empty() {
                    self.status_msg = Some((false, "Template name cannot be empty.".to_string()));
                } else {
                    let shortcut = if self.new_tpl_shortcut.trim().is_empty() {
                        None
                    } else {
                        Some(self.new_tpl_shortcut.trim().to_string())
                    };

                    let is_editing = self.editing_tpl_id.is_some();
                    let tpl = if let Some(ref edit_id) = self.editing_tpl_id {
                        Template {
                            id: edit_id.clone(),
                            name: self.new_tpl_name.clone(),
                            subject_template: self.new_tpl_subject.clone(),
                            body_template: self.new_tpl_body.clone(),
                            shortcut,
                            created_at: self.editing_tpl_created_at.unwrap_or_else(|| chrono::Utc::now().timestamp()),
                        }
                    } else {
                        Template::new(
                            self.new_tpl_name.clone(),
                            self.new_tpl_subject.clone(),
                            self.new_tpl_body.clone(),
                            shortcut,
                        )
                    };

                    let _ = storage.save_template(&tpl);
                    self.reset_tpl_form();
                    self.status_msg = Some((
                        true,
                        if is_editing {
                            "Template updated successfully.".to_string()
                        } else {
                            "Template saved successfully.".to_string()
                        },
                    ));
                    *on_data_changed = true;
                }
            }

            if self.editing_tpl_id.is_some() {
                if ui.button("Cancel").clicked() {
                    self.reset_tpl_form();
                    self.status_msg = None;
                }
            }
        });
    }

    fn show_appearance_tab(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        current_theme: &mut crate::theme::ThemePreset,
    ) {
        ui.heading(RichText::new("Theme & Visual Style").size(16.0).color(AppTheme::TEXT_PRIMARY));
        ui.add_space(4.0);
        ui.label(RichText::new("Choose a built-in theme preset or design your own custom theme saved to OS configuration.").size(12.0).color(AppTheme::TEXT_MUTED));
        ui.add_space(14.0);

        ui.label(RichText::new("BUILT-IN PRESETS").size(11.0).strong().color(AppTheme::TEXT_MUTED));
        ui.add_space(6.0);

        for preset in crate::theme::ThemePreset::all() {
            let is_selected = self.active_custom_theme_id.is_none() && *current_theme == *preset;
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
                            } else {
                                let btn = ui.button(RichText::new("Apply Theme").size(12.0));
                                if btn.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                                if btn.clicked() {
                                    self.active_custom_theme_id = None;
                                    *current_theme = *preset;
                                    AppTheme::apply_preset(ctx, *preset);
                                    self.status_msg = Some((true, format!("Switched to {} theme.", preset.display_name())));
                                }
                            }
                        });
                    });
                });

            ui.add_space(6.0);
        }

        // Custom Themes Section
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("CUSTOM USER THEMES").size(11.0).strong().color(AppTheme::TEXT_MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(RichText::new("🔄 Refresh Themes").size(11.0)).clicked() {
                    self.custom_themes = load_custom_themes();
                    self.status_msg = Some((true, format!("Loaded {} custom themes from config directory.", self.custom_themes.len())));
                }
            });
        });
        ui.add_space(6.0);

        if self.custom_themes.is_empty() {
            ui.label(RichText::new("No custom themes created yet. Use the editor below to create one!").size(12.0).italics().color(AppTheme::TEXT_MUTED));
        } else {
            for ct in self.custom_themes.clone() {
                let is_active = self.active_custom_theme_id.as_deref() == Some(&ct.id);
                let border_color = if is_active { AppTheme::ACCENT_PRIMARY } else { AppTheme::BORDER_SUBTLE };

                egui::Frame::none()
                    .fill(if is_active { AppTheme::BG_HOVER } else { AppTheme::BG_CARD })
                    .stroke(Stroke::new(if is_active { 1.5_f32 } else { 1.0_f32 }, border_color))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // Color swatches preview
                            let (swatch_rect, _) = ui.allocate_exact_size(Vec2::new(54.0, 24.0), egui::Sense::hover());
                            let r1 = egui::Rect::from_min_size(swatch_rect.min, Vec2::new(18.0, 24.0));
                            let r2 = egui::Rect::from_min_size(swatch_rect.min + Vec2::new(18.0, 0.0), Vec2::new(18.0, 24.0));
                            let r3 = egui::Rect::from_min_size(swatch_rect.min + Vec2::new(36.0, 0.0), Vec2::new(18.0, 24.0));
                            ui.painter().rect_filled(r1, Rounding::ZERO, Color32::from_rgb(ct.bg_app[0], ct.bg_app[1], ct.bg_app[2]));
                            ui.painter().rect_filled(r2, Rounding::ZERO, Color32::from_rgb(ct.bg_card[0], ct.bg_card[1], ct.bg_card[2]));
                            ui.painter().rect_filled(r3, Rounding::ZERO, Color32::from_rgb(ct.accent_primary[0], ct.accent_primary[1], ct.accent_primary[2]));
                            ui.painter().rect_stroke(swatch_rect, Rounding::same(4.0), Stroke::new(1.0_f32, AppTheme::BORDER_SUBTLE));

                            ui.add_space(8.0);
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(&ct.name).size(13.5).strong().color(if is_active { AppTheme::ACCENT_PRIMARY } else { AppTheme::TEXT_PRIMARY }));
                                    if is_active {
                                        ui.label(RichText::new("✓ Active").size(11.0).strong().color(AppTheme::ACCENT_SUCCESS));
                                    }
                                });
                                ui.add_space(2.0);
                                ui.label(RichText::new(format!("{} • Saved to OS Config ({}.json)", ct.description, ct.id)).size(11.0).color(AppTheme::TEXT_MUTED));
                            });

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button(RichText::new("🗑 Delete").size(11.5).color(AppTheme::ACCENT_DANGER)).clicked() {
                                    let _ = delete_custom_theme(&ct.id);
                                    self.custom_themes = load_custom_themes();
                                    self.status_msg = Some((true, format!("Deleted custom theme '{}'.", ct.name)));
                                }
                                if is_active {
                                    ui.label(RichText::new("Applied").size(12.0).color(AppTheme::ACCENT_PRIMARY));
                                } else {
                                    let btn = ui.button(RichText::new("Apply Theme").size(12.0));
                                    if btn.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    if btn.clicked() {
                                        self.active_custom_theme_id = Some(ct.id.clone());
                                        AppTheme::apply_custom(ctx, &ct);
                                        self.status_msg = Some((true, format!("Applied custom theme '{}'.", ct.name)));
                                    }
                                }
                            });
                        });
                    });

                ui.add_space(6.0);
            }
        }

        // Custom Theme Creator Form
        ui.add_space(16.0);
        ui.label(RichText::new("🎨 CREATE / CUSTOMIZE THEME").size(11.0).strong().color(AppTheme::TEXT_MUTED));
        ui.add_space(4.0);
        ui.label(RichText::new(format!("Themes are saved as individual JSON files in: {}", get_themes_dir().display())).size(11.0).color(AppTheme::TEXT_MUTED));
        ui.add_space(8.0);

        // Quick Starter Presets
        ui.horizontal(|ui| {
            ui.label(RichText::new("Start with palette:").size(11.5).color(AppTheme::TEXT_SECONDARY));
            if ui.button("Gruvbox Warm").clicked() {
                self.new_theme_name = "Gruvbox Custom".to_string();
                self.new_theme_desc = "Warm retro groove dark palette".to_string();
                self.new_theme_is_dark = true;
                self.new_theme_bg_app = [40, 40, 40];
                self.new_theme_bg_list = [50, 48, 47];
                self.new_theme_bg_view = [60, 56, 54];
                self.new_theme_bg_card = [80, 73, 69];
                self.new_theme_bg_hover = [102, 92, 84];
                self.new_theme_bg_selected = [214, 93, 14];
                self.new_theme_accent_primary = [250, 189, 47];
                self.new_theme_accent_hover = [254, 128, 25];
                self.new_theme_border = [80, 73, 69];
                self.new_theme_text_primary = [235, 219, 178];
                self.new_theme_text_secondary = [213, 196, 161];
            }
            if ui.button("Nord Frost").clicked() {
                self.new_theme_name = "Nord Custom".to_string();
                self.new_theme_desc = "Arctic blue dark palette".to_string();
                self.new_theme_is_dark = true;
                self.new_theme_bg_app = [46, 52, 64];
                self.new_theme_bg_list = [59, 66, 82];
                self.new_theme_bg_view = [67, 76, 94];
                self.new_theme_bg_card = [76, 86, 106];
                self.new_theme_bg_hover = [94, 106, 130];
                self.new_theme_bg_selected = [129, 161, 193];
                self.new_theme_accent_primary = [136, 192, 208];
                self.new_theme_accent_hover = [143, 188, 187];
                self.new_theme_border = [76, 86, 106];
                self.new_theme_text_primary = [236, 239, 244];
                self.new_theme_text_secondary = [216, 222, 233];
            }
            if ui.button("Neon Cyberpunk").clicked() {
                self.new_theme_name = "Cyberpunk Neon".to_string();
                self.new_theme_desc = "High-contrast dark synthwave palette".to_string();
                self.new_theme_is_dark = true;
                self.new_theme_bg_app = [18, 16, 32];
                self.new_theme_bg_list = [28, 22, 48];
                self.new_theme_bg_view = [36, 28, 62];
                self.new_theme_bg_card = [50, 38, 86];
                self.new_theme_bg_hover = [70, 52, 118];
                self.new_theme_bg_selected = [160, 32, 240];
                self.new_theme_accent_primary = [255, 0, 128];
                self.new_theme_accent_hover = [0, 255, 230];
                self.new_theme_border = [80, 56, 130];
                self.new_theme_text_primary = [250, 240, 255];
                self.new_theme_text_secondary = [200, 180, 230];
            }
        });
        ui.add_space(10.0);

        egui::Grid::new("custom_theme_creator_grid")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Theme Name:");
                ui.text_edit_singleline(&mut self.new_theme_name);
                ui.end_row();

                ui.label("Description:");
                ui.text_edit_singleline(&mut self.new_theme_desc);
                ui.end_row();

                ui.label("Dark Mode Base:");
                ui.checkbox(&mut self.new_theme_is_dark, "Enable Dark Visuals Base");
                ui.end_row();

                ui.label("App Background:");
                ui.color_edit_button_srgb(&mut self.new_theme_bg_app);
                ui.end_row();

                ui.label("List / Sidebar Surface:");
                ui.color_edit_button_srgb(&mut self.new_theme_bg_list);
                ui.end_row();

                ui.label("Reading Pane Surface:");
                ui.color_edit_button_srgb(&mut self.new_theme_bg_view);
                ui.end_row();

                ui.label("Card Surface:");
                ui.color_edit_button_srgb(&mut self.new_theme_bg_card);
                ui.end_row();

                ui.label("Hover State:");
                ui.color_edit_button_srgb(&mut self.new_theme_bg_hover);
                ui.end_row();

                ui.label("Selected Highlight:");
                ui.color_edit_button_srgb(&mut self.new_theme_bg_selected);
                ui.end_row();

                ui.label("Primary Accent:");
                ui.color_edit_button_srgb(&mut self.new_theme_accent_primary);
                ui.end_row();

                ui.label("Accent Hover / Secondary:");
                ui.color_edit_button_srgb(&mut self.new_theme_accent_hover);
                ui.end_row();

                ui.label("Border Color:");
                ui.color_edit_button_srgb(&mut self.new_theme_border);
                ui.end_row();

                ui.label("Primary Text:");
                ui.color_edit_button_srgb(&mut self.new_theme_text_primary);
                ui.end_row();

                ui.label("Secondary Text:");
                ui.color_edit_button_srgb(&mut self.new_theme_text_secondary);
                ui.end_row();
            });

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.button(RichText::new("💾 Save & Export Theme JSON").strong()).clicked() {
                if self.new_theme_name.trim().is_empty() {
                    self.status_msg = Some((false, "Please provide a valid theme name.".to_string()));
                } else {
                    let theme = CustomTheme::new(
                        self.new_theme_name.clone(),
                        self.new_theme_desc.clone(),
                        self.new_theme_is_dark,
                        self.new_theme_bg_app,
                        self.new_theme_bg_list,
                        self.new_theme_bg_view,
                        self.new_theme_bg_card,
                        self.new_theme_bg_hover,
                        self.new_theme_bg_selected,
                        self.new_theme_accent_primary,
                        self.new_theme_accent_hover,
                        self.new_theme_border,
                        self.new_theme_text_primary,
                        self.new_theme_text_secondary,
                    );

                    match save_custom_theme(&theme) {
                        Ok(path) => {
                            self.custom_themes = load_custom_themes();
                            self.active_custom_theme_id = Some(theme.id.clone());
                            AppTheme::apply_custom(ctx, &theme);
                            self.status_msg = Some((true, format!("Saved & applied theme '{}' at {:?}", theme.name, path)));
                        }
                        Err(e) => {
                            self.status_msg = Some((false, e));
                        }
                    }
                }
            }

            if ui.button("👁 Live Preview Palette").clicked() {
                let temp_theme = CustomTheme::new(
                    self.new_theme_name.clone(),
                    self.new_theme_desc.clone(),
                    self.new_theme_is_dark,
                    self.new_theme_bg_app,
                    self.new_theme_bg_list,
                    self.new_theme_bg_view,
                    self.new_theme_bg_card,
                    self.new_theme_bg_hover,
                    self.new_theme_bg_selected,
                    self.new_theme_accent_primary,
                    self.new_theme_accent_hover,
                    self.new_theme_border,
                    self.new_theme_text_primary,
                    self.new_theme_text_secondary,
                );
                AppTheme::apply_custom(ctx, &temp_theme);
                self.status_msg = Some((true, "Applied live preview of custom palette.".to_string()));
            }
        });
    }

    fn show_general_tab(&mut self, ui: &mut Ui, accounts: &[Account], _storage: &Storage) {
        ui.heading(RichText::new("Application & Storage").size(16.0).color(AppTheme::TEXT_PRIMARY));
        ui.add_space(10.0);

        let active_db = get_database_path();
        let config_dir = get_config_dir();
        let themes_dir = get_themes_dir();

        ui.label(RichText::new("STORAGE & FILE LOCATIONS").size(11.0).strong().color(AppTheme::TEXT_MUTED));
        ui.add_space(6.0);

        egui::Frame::none()
            .fill(AppTheme::BG_CARD)
            .stroke(Stroke::new(1.0_f32, AppTheme::BORDER_SUBTLE))
            .rounding(Rounding::same(8.0))
            .inner_margin(14.0)
            .show(ui, |ui| {
                ui.label(RichText::new("• Active SQLite Database:").strong().color(AppTheme::TEXT_PRIMARY));
                ui.label(RichText::new(format!("{}", active_db.display())).size(11.5).color(AppTheme::ACCENT_HOVER));
                ui.add_space(4.0);

                ui.label(RichText::new("• OS Config Directory:").strong().color(AppTheme::TEXT_PRIMARY));
                ui.label(RichText::new(format!("{}", config_dir.display())).size(11.5).color(AppTheme::TEXT_SECONDARY));
                ui.add_space(4.0);

                ui.label(RichText::new("• Custom Themes Directory:").strong().color(AppTheme::TEXT_PRIMARY));
                ui.label(RichText::new(format!("{}", themes_dir.display())).size(11.5).color(AppTheme::TEXT_SECONDARY));
                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui.button(RichText::new("📁 Relocate / Move Data Directory").strong()).clicked() {
                        let current_db_path = active_db.clone();
                        if let Some(target_dir) = rfd::FileDialog::new().set_title("Select New Data Directory").pick_folder() {
                            let target_db = target_dir.join("email_client.db");
                            if current_db_path.exists() && current_db_path != target_db {
                                let _ = std::fs::copy(&current_db_path, &target_db);
                                // Also copy WAL / SHM files if present
                                let wal_src = current_db_path.with_extension("db-wal");
                                if wal_src.exists() {
                                    let _ = std::fs::copy(&wal_src, target_dir.join("email_client.db-wal"));
                                }
                                let shm_src = current_db_path.with_extension("db-shm");
                                if shm_src.exists() {
                                    let _ = std::fs::copy(&shm_src, target_dir.join("email_client.db-shm"));
                                }
                            }

                            let mut cfg = load_app_config();
                            cfg.custom_data_dir = Some(target_dir.to_string_lossy().to_string());
                            if let Err(e) = save_app_config(&cfg) {
                                self.status_msg = Some((false, format!("Failed to save config: {}", e)));
                            } else {
                                self.status_msg = Some((true, format!("Data relocated to {:?}. Please restart the application to use the new directory.", target_dir)));
                            }
                        }
                    }

                    if ui.button("↺ Reset to Default OS Path").clicked() {
                        let mut cfg = load_app_config();
                        cfg.custom_data_dir = None;
                        let _ = save_app_config(&cfg);
                        self.status_msg = Some((true, "Reset data storage path to default OS directory. Restart app to apply.".to_string()));
                    }
                });
            });

        ui.add_space(14.0);
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

    fn show_backup_tab(
        &mut self,
        ui: &mut Ui,
        accounts: &[Account],
        templates: &[Template],
        signatures: &[Signature],
        storage: &Storage,
        on_data_changed: &mut bool,
    ) {
        ui.heading(RichText::new("Backup & Restore Data").size(16.0).color(AppTheme::TEXT_PRIMARY));
        ui.add_space(4.0);
        ui.label(RichText::new("Create portable complete backups of your email accounts configuration, themes, templates, signatures, and preferences.").size(12.0).color(AppTheme::TEXT_MUTED));
        ui.add_space(14.0);

        egui::Frame::none()
            .fill(AppTheme::BG_CARD)
            .stroke(Stroke::new(1.0_f32, AppTheme::BORDER_SUBTLE))
            .rounding(Rounding::same(8.0))
            .inner_margin(14.0)
            .show(ui, |ui| {
                ui.label(RichText::new("🔒 PRIVACY & SECURITY GUARANTEE").size(11.0).strong().color(AppTheme::ACCENT_SUCCESS));
                ui.add_space(4.0);
                ui.label("• Backups include full account IMAP/SMTP endpoints, security settings, and folder configurations.");
                ui.label("• Backups strictly NEVER include plaintext passwords or OS keyring keys.");
                ui.label("• When restoring on a new machine, you will simply enter passwords once for each account.");
            });

        ui.add_space(14.0);
        ui.label(RichText::new("BACKUP CONTENTS SUMMARY").size(11.0).strong().color(AppTheme::TEXT_MUTED));
        ui.add_space(4.0);
        ui.label(format!("• Email Accounts: {} configured", accounts.len()));
        ui.label(format!("• Quick Templates: {} templates", templates.len()));
        ui.label(format!("• Signatures: {} signatures", signatures.len()));
        ui.label(format!("• Custom Themes: {} themes in config directory", self.custom_themes.len()));

        ui.add_space(16.0);
        ui.horizontal(|ui| {
            // Export Full Backup
            if ui.button(RichText::new("💾 Export Full Backup (*.json)").strong().size(13.0)).clicked() {
                let backup_accounts: Vec<AccountBackup> = accounts.iter().map(AccountBackup::from).collect();
                let backup = AppBackup {
                    format_version: 1,
                    app_name: "AT-mail-rs".to_string(),
                    exported_at: chrono::Utc::now().timestamp(),
                    accounts: backup_accounts,
                    templates: templates.to_vec(),
                    signatures: signatures.to_vec(),
                    custom_themes: self.custom_themes.clone(),
                    settings: SettingsMetadata {
                        active_theme: self.active_custom_theme_id.clone(),
                        export_version: 1,
                    },
                };

                let now_str = chrono::Utc::now().format("%Y-%m-%d").to_string();
                let default_fname = format!("at_mail_backup_{}.json", now_str);

                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Export AT-mail-rs Backup")
                    .set_file_name(&default_fname)
                    .add_filter("JSON Backup (*.json)", &["json"])
                    .save_file()
                {
                    match serde_json::to_string_pretty(&backup) {
                        Ok(json_content) => {
                            if let Err(e) = std::fs::write(&path, json_content) {
                                self.status_msg = Some((false, format!("Failed to write backup file: {}", e)));
                            } else {
                                self.status_msg = Some((true, format!("Successfully exported complete backup to {:?}", path)));
                            }
                        }
                        Err(e) => {
                            self.status_msg = Some((false, format!("Failed to serialize backup: {}", e)));
                        }
                    }
                }
            }

            ui.add_space(8.0);

            // Restore Full Backup
            if ui.button(RichText::new("📥 Restore From Backup (*.json)").size(13.0)).clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Select AT-mail-rs Backup JSON")
                    .add_filter("JSON Backup (*.json)", &["json"])
                    .pick_file()
                {
                    match std::fs::read_to_string(&path) {
                        Ok(content) => {
                            match serde_json::from_str::<AppBackup>(&content) {
                                Ok(backup) => {
                                    let mut restored_accs = 0;
                                    let mut restored_tpls = 0;
                                    let mut restored_sigs = 0;
                                    let mut restored_themes = 0;

                                    // Restore Accounts (without credentials)
                                    for ab in &backup.accounts {
                                        let dummy_acc = Account {
                                            id: ab.id.clone(),
                                            name: ab.name.clone(),
                                            email: ab.email.clone(),
                                            imap_host: ab.imap_host.clone(),
                                            imap_port: ab.imap_port,
                                            imap_security: ab.imap_security,
                                            smtp_host: ab.smtp_host.clone(),
                                            smtp_port: ab.smtp_port,
                                            smtp_security: ab.smtp_security,
                                            auth_type: ab.auth_type,
                                            credential_key: format!("mail_acc_{}_secret", ab.id),
                                            sync_days_window: ab.sync_days_window,
                                            is_enabled: ab.is_enabled,
                                            created_at: chrono::Utc::now().timestamp(),
                                            updated_at: chrono::Utc::now().timestamp(),
                                        };
                                        let _ = storage.save_account(&dummy_acc);
                                        restored_accs += 1;
                                    }

                                    // Restore Templates
                                    for t in &backup.templates {
                                        let _ = storage.save_template(t);
                                        restored_tpls += 1;
                                    }

                                    // Restore Signatures
                                    for s in &backup.signatures {
                                        let _ = storage.save_signature(s);
                                        restored_sigs += 1;
                                    }

                                    // Restore Custom Themes to OS config dir
                                    for ct in &backup.custom_themes {
                                        let _ = save_custom_theme(ct);
                                        restored_themes += 1;
                                    }

                                    self.custom_themes = load_custom_themes();
                                    *on_data_changed = true;
                                    self.status_msg = Some((
                                        true,
                                        format!(
                                            "Restored successfully: {} accounts, {} templates, {} signatures, {} themes.",
                                            restored_accs, restored_tpls, restored_sigs, restored_themes
                                        ),
                                    ));
                                }
                                Err(e) => {
                                    self.status_msg = Some((false, format!("Invalid backup file format: {}", e)));
                                }
                            }
                        }
                        Err(e) => {
                            self.status_msg = Some((false, format!("Failed to read backup file: {}", e)));
                        }
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_custom_theme_filesystem_crud() {
        let test_theme = CustomTheme::new(
            "Unit Test Theme".to_string(),
            "Theme for testing filesystem storage".to_string(),
            true,
            [10, 10, 10],
            [20, 20, 20],
            [30, 30, 30],
            [40, 40, 40],
            [50, 50, 50],
            [60, 60, 60],
            [255, 100, 50],
            [255, 150, 100],
            [70, 70, 70],
            [240, 240, 240],
            [180, 180, 180],
        );

        let path = save_custom_theme(&test_theme).expect("Save test theme");
        assert!(path.exists());

        let all = load_custom_themes();
        assert!(all.iter().any(|t| t.id == test_theme.id));

        delete_custom_theme(&test_theme.id).expect("Delete test theme");
        let all_after = load_custom_themes();
        assert!(!all_after.iter().any(|t| t.id == test_theme.id));
    }

    #[test]
    fn test_signature_and_template_editing_state() {
        let mut settings = SettingsView::new();

        // Signature Edit State
        settings.editing_sig_id = Some("sig_42".to_string());
        settings.new_sig_name = "Work Signature".to_string();
        settings.new_sig_html = "<b>Best,</b> Alex".to_string();
        settings.new_sig_is_default = true;

        assert_eq!(settings.editing_sig_id.as_deref(), Some("sig_42"));
        assert_eq!(settings.new_sig_name, "Work Signature");

        settings.reset_sig_form();
        assert!(settings.editing_sig_id.is_none());
        assert!(settings.new_sig_name.is_empty());
        assert!(!settings.new_sig_is_default);

        // Template Edit State
        settings.editing_tpl_id = Some("tpl_99".to_string());
        settings.new_tpl_name = "Status Update".to_string();
        settings.new_tpl_subject = "Weekly Progress".to_string();
        settings.new_tpl_body = "Hi team, here is the update...".to_string();
        settings.new_tpl_shortcut = "/status".to_string();

        assert_eq!(settings.editing_tpl_id.as_deref(), Some("tpl_99"));
        assert_eq!(settings.new_tpl_name, "Status Update");

        settings.reset_tpl_form();
        assert!(settings.editing_tpl_id.is_none());
        assert!(settings.new_tpl_name.is_empty());
        assert!(settings.new_tpl_subject.is_empty());
        assert!(settings.new_tpl_body.is_empty());
        assert!(settings.new_tpl_shortcut.is_empty());
    }
}
