use crate::theme::AppTheme;
use chrono::{Duration, Utc};
use egui::{Color32, RichText, Rounding, Stroke, Window};
use email_core::models::{Account, Draft, MessageHeader, OutgoingDraft, Recipient, ScheduledEmail, Signature, Template};
use email_keychain::CredentialStore;
use email_storage::Storage;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposeFormat {
    Html,
    PlainText,
    Markdown,
}

pub struct ComposeView {
    pub is_open: bool,
    pub draft_id: Option<String>,
    pub selected_account_id: String,
    pub selected_signature_id: Option<String>,
    pub to_input: String,
    pub cc_input: String,
    pub bcc_input: String,
    pub subject: String,
    pub body_plain: String,
    pub reply_quote: Option<String>,
    pub show_cc_bcc: bool,
    pub format: ComposeFormat,
    pub show_markdown_preview: bool,
    pub send_as_html: bool,
    pub in_reply_to: Option<String>,
    pub references: Option<String>,
    pub error_msg: Option<String>,
    pub status_msg: Option<(bool, String)>,
    pub show_custom_schedule_dialog: bool,
    pub custom_schedule_hours: u32,
    pub custom_schedule_mins: u32,
    pub enable_pgp_encryption: bool,
    pub enable_pgp_signing: bool,
    pub attachments: Vec<email_core::models::OutgoingAttachment>,
    pub show_quoted_text: bool,
    pub include_quote: bool,
    pub show_signature_card: bool,
}

fn find_default_signature<'a>(signatures: &'a [Signature], account_id: Option<&str>) -> Option<&'a Signature> {
    if let Some(aid) = account_id {
        // 1. Account-specific default signature
        if let Some(sig) = signatures.iter().find(|s| s.account_id.as_deref() == Some(aid) && s.is_default) {
            return Some(sig);
        }
    }
    // 2. Global default signature
    if let Some(sig) = signatures.iter().find(|s| s.account_id.is_none() && s.is_default) {
        return Some(sig);
    }
    // 3. Any signature marked as default
    if let Some(sig) = signatures.iter().find(|s| s.is_default) {
        return Some(sig);
    }
    // 4. Any account-matching signature fallback
    if let Some(aid) = account_id {
        if let Some(sig) = signatures.iter().find(|s| s.account_id.as_deref() == Some(aid)) {
            return Some(sig);
        }
    }
    None
}

impl ComposeView {
    pub fn new() -> Self {
        Self {
            is_open: false,
            draft_id: None,
            selected_account_id: String::new(),
            selected_signature_id: None,
            to_input: String::new(),
            cc_input: String::new(),
            bcc_input: String::new(),
            subject: String::new(),
            body_plain: String::new(),
            reply_quote: None,
            show_cc_bcc: false,
            format: ComposeFormat::Markdown,
            show_markdown_preview: false,
            send_as_html: true,
            in_reply_to: None,
            references: None,
            error_msg: None,
            status_msg: None,
            show_custom_schedule_dialog: false,
            custom_schedule_hours: 1,
            custom_schedule_mins: 0,
            enable_pgp_encryption: false,
            enable_pgp_signing: false,
            attachments: Vec::new(),
            show_quoted_text: true,
            include_quote: true,
            show_signature_card: true,
        }
    }

    pub fn open_new(&mut self, default_account_id: Option<&str>, signatures: &[Signature]) {
        self.is_open = true;
        self.draft_id = None;
        self.to_input.clear();
        self.cc_input.clear();
        self.bcc_input.clear();
        self.subject.clear();
        self.body_plain.clear();
        self.reply_quote = None;
        self.show_cc_bcc = false;
        self.format = ComposeFormat::Markdown;
        self.show_markdown_preview = false;
        self.send_as_html = true;
        self.in_reply_to = None;
        self.references = None;
        self.error_msg = None;
        self.attachments.clear();
        self.status_msg = None;
        self.show_custom_schedule_dialog = false;
        self.show_quoted_text = true;
        self.include_quote = true;
        self.show_signature_card = true;
        if let Some(aid) = default_account_id {
            self.selected_account_id = aid.to_string();
        }
        self.selected_signature_id = find_default_signature(signatures, default_account_id).map(|s| s.id.clone());
    }

    pub fn open_reply(
        &mut self,
        account_id: &str,
        to: &str,
        cc: &str,
        subject: &str,
        in_reply_to: Option<String>,
        body_quote: &str,
        signatures: &[Signature],
        send_as_html: bool,
    ) {
        self.is_open = true;
        self.draft_id = None;
        self.selected_account_id = account_id.to_string();
        self.to_input = to.to_string();
        self.cc_input = cc.to_string();
        self.bcc_input.clear();
        self.show_cc_bcc = !cc.is_empty();
        self.subject = if subject.to_lowercase().starts_with("re:") {
            subject.to_string()
        } else {
            format!("Re: {}", subject)
        };
        self.in_reply_to = in_reply_to.clone();
        self.references = in_reply_to;
        self.body_plain.clear();
        self.reply_quote = Some(email_html::html_to_plain_text(body_quote));
        self.selected_signature_id = find_default_signature(signatures, Some(account_id)).map(|s| s.id.clone());
        self.format = if send_as_html { ComposeFormat::Html } else { ComposeFormat::PlainText };
        self.show_markdown_preview = false;
        self.send_as_html = send_as_html;
        self.error_msg = None;
        self.status_msg = None;
        self.show_custom_schedule_dialog = false;
        self.show_quoted_text = true;
        self.include_quote = true;
        self.show_signature_card = true;
    }

    pub fn open_draft(&mut self, draft: &Draft, signatures: &[Signature]) {
        self.is_open = true;
        self.draft_id = Some(draft.id.clone());
        self.selected_account_id = draft.account_id.clone();
        self.to_input = draft.to_input.clone();
        self.cc_input = draft.cc_input.clone();
        self.bcc_input = draft.bcc_input.clone();
        self.show_cc_bcc = !draft.cc_input.is_empty() || !draft.bcc_input.is_empty();
        self.subject = draft.subject.clone();
        self.body_plain = draft.body_plain.clone();
        self.format = match draft.format.as_str() {
            "html" => ComposeFormat::Html,
            "plaintext" => ComposeFormat::PlainText,
            _ => ComposeFormat::Markdown,
        };
        self.send_as_html = self.format != ComposeFormat::PlainText;
        self.selected_signature_id = draft.signature_id.clone().or_else(|| {
            find_default_signature(signatures, Some(&draft.account_id)).map(|s| s.id.clone())
        });
        self.in_reply_to = draft.in_reply_to.clone();
        self.references = draft.references.clone();
        self.error_msg = None;
        self.status_msg = Some((true, "Loaded saved draft".to_string()));
        self.show_custom_schedule_dialog = false;
        self.show_quoted_text = true;
        self.include_quote = true;
        self.show_signature_card = true;
    }

    pub fn restore_from_draft(&mut self, draft: &OutgoingDraft) {
        self.is_open = true;
        self.draft_id = None;
        self.selected_account_id = draft.account_id.clone();
        self.to_input = draft.to.iter().map(|r| r.email.clone()).collect::<Vec<_>>().join(", ");
        self.cc_input = draft.cc.iter().map(|r| r.email.clone()).collect::<Vec<_>>().join(", ");
        self.bcc_input = draft.bcc.iter().map(|r| r.email.clone()).collect::<Vec<_>>().join(", ");
        self.show_cc_bcc = !self.cc_input.is_empty() || !self.bcc_input.is_empty();
        self.subject = draft.subject.clone();
        self.body_plain = draft.body_plain.clone();
        self.in_reply_to = draft.in_reply_to.clone();
        self.references = draft.references.clone();
        self.format = if draft.body_html.is_some() { ComposeFormat::Markdown } else { ComposeFormat::PlainText };
        self.error_msg = None;
        self.status_msg = None;
        self.show_quoted_text = true;
        self.include_quote = true;
        self.show_signature_card = true;
    }

    pub fn save_draft_to_storage(&mut self, storage: &Storage) -> Result<String, String> {
        if self.selected_account_id.is_empty() {
            return Err("Please choose an account first.".to_string());
        }

        let draft_id = self.draft_id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        self.draft_id = Some(draft_id.clone());

        let format_str = match self.format {
            ComposeFormat::Html => "html",
            ComposeFormat::PlainText => "plaintext",
            ComposeFormat::Markdown => "markdown",
        };

        let draft = Draft {
            id: draft_id.clone(),
            account_id: self.selected_account_id.clone(),
            to_input: self.to_input.clone(),
            cc_input: self.cc_input.clone(),
            bcc_input: self.bcc_input.clone(),
            subject: self.subject.clone(),
            body_plain: self.body_plain.clone(),
            format: format_str.to_string(),
            signature_id: self.selected_signature_id.clone(),
            in_reply_to: self.in_reply_to.clone(),
            references: self.references.clone(),
            updated_at: Utc::now().timestamp(),
        };

        match storage.save_draft(&draft) {
            Ok(()) => {
                let now_str = chrono::Local::now().format("%H:%M:%S").to_string();
                self.status_msg = Some((true, format!("✓ Draft saved at {}", now_str)));
                Ok(draft_id)
            }
            Err(e) => {
                self.status_msg = Some((false, format!("Failed to save draft: {}", e)));
                Err(e.to_string())
            }
        }
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        accounts: &[Account],
        templates: &[Template],
        signatures: &[Signature],
        keyring: &Arc<dyn CredentialStore>,
        storage: &Storage,
        on_schedule_send: &mut Option<(OutgoingDraft, String)>,
        on_data_changed: &mut bool,
        status_toast: &mut Option<(String, std::time::Instant)>,
    ) {
        if !self.is_open {
            return;
        }

        let mut is_open = self.is_open;
        Window::new("✉ Compose Email")
            .open(&mut is_open)
            .collapsible(false)
            .resizable(true)
            .default_width(820.0)
            .default_height(640.0)
            .min_width(540.0)
            .min_height(420.0)
            .show(ctx, |ui| {
                if accounts.is_empty() {
                    ui.label("No email accounts configured. Please add an account first.");
                    return;
                }

                // Process Drag and Drop files
                let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
                if !dropped_files.is_empty() {
                    for file in dropped_files {
                        if let Some(ref path) = file.path {
                            if let Ok(bytes) = std::fs::read(path) {
                                let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "file.bin".to_string());
                                self.attachments.push(email_core::models::OutgoingAttachment::new(name, "application/octet-stream".to_string(), &bytes));
                            }
                        } else if let Some(bytes) = file.bytes {
                            let name = if file.name.is_empty() { "dropped_file.bin".to_string() } else { file.name };
                            self.attachments.push(email_core::models::OutgoingAttachment::new(name, "application/octet-stream".to_string(), &bytes.to_vec()));
                        }
                    }
                }

                if self.selected_account_id.is_empty() {
                    self.selected_account_id = accounts[0].id.clone();
                    if self.selected_signature_id.is_none() {
                        self.selected_signature_id = find_default_signature(signatures, Some(&self.selected_account_id)).map(|s| s.id.clone());
                    }
                }

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // 1. Primary Action Toolbar (Row 1)
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);

                            let send_btn_label = if self.send_as_html { "🚀 Send" } else { "🚀 Send (Text)" };

                            let top_send_btn = egui::Button::new(
                                RichText::new(send_btn_label)
                                    .size(12.5)
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .fill(AppTheme::ACCENT_PRIMARY)
                            .rounding(Rounding::same(6.0));

                            if ui.add(top_send_btn).clicked() {
                                if self.execute_send(accounts, signatures, keyring, on_schedule_send, storage) {
                                    *on_data_changed = true;
                                }
                            }

                            // Send Later Dropdown
                            egui::ComboBox::from_id_salt("send_later_combo")
                                .selected_text(RichText::new("⏰ Send Later").size(12.0).color(AppTheme::ACCENT_PRIMARY))
                                .show_ui(ui, |ui| {
                                    let now = Utc::now();
                                    if ui.button("⚡ In 15 minutes").clicked() {
                                        let target_ts = (now + Duration::minutes(15)).timestamp();
                                        if self.execute_schedule_send(accounts, signatures, keyring, storage, target_ts) {
                                            *status_toast = Some(("✓ Email scheduled for in 15 minutes".to_string(), std::time::Instant::now()));
                                            *on_data_changed = true;
                                        }
                                    }
                                    if ui.button("⏰ In 1 hour").clicked() {
                                        let target_ts = (now + Duration::hours(1)).timestamp();
                                        if self.execute_schedule_send(accounts, signatures, keyring, storage, target_ts) {
                                            *status_toast = Some(("✓ Email scheduled for in 1 hour".to_string(), std::time::Instant::now()));
                                            *on_data_changed = true;
                                        }
                                    }
                                    if ui.button("🕒 In 3 hours").clicked() {
                                        let target_ts = (now + Duration::hours(3)).timestamp();
                                        if self.execute_schedule_send(accounts, signatures, keyring, storage, target_ts) {
                                            *status_toast = Some(("✓ Email scheduled for in 3 hours".to_string(), std::time::Instant::now()));
                                            *on_data_changed = true;
                                        }
                                    }
                                    if ui.button("📅 Custom Schedule...").clicked() {
                                        self.show_custom_schedule_dialog = true;
                                    }
                                });

                            // Save Draft Button
                            if ui.button(RichText::new("💾 Save Draft").size(12.0)).clicked() {
                                let _ = self.save_draft_to_storage(storage);
                                *on_data_changed = true;
                            }

                            // Attach Files Button
                            if ui.button(RichText::new("📎 Attach").size(12.0)).on_hover_text("Attach files or drag and drop into composer").clicked() {
                                let dialog = rfd::FileDialog::new().set_title("Select Files to Attach");
                                if let Some(paths) = dialog.pick_files() {
                                    for path in paths {
                                        if let Ok(bytes) = std::fs::read(&path) {
                                            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "attachment.bin".to_string());
                                            self.attachments.push(email_core::models::OutgoingAttachment::new(name, "application/octet-stream".to_string(), &bytes));
                                        }
                                    }
                                }
                            }

                            // Templates picker
                            if !templates.is_empty() {
                                egui::ComboBox::from_id_salt("compose_template_picker")
                                    .selected_text("📋 Template")
                                    .show_ui(ui, |ui| {
                                        for t in templates {
                                            if ui.button(&t.name).clicked() {
                                                if self.subject.is_empty() && !t.subject_template.is_empty() {
                                                    self.subject = t.subject_template.clone();
                                                }
                                                self.body_plain.push_str(&t.body_template);
                                            }
                                        }
                                    });
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button(RichText::new("Discard").size(11.5)).clicked() {
                                    self.is_open = false;
                                }
                            });
                        });

                        ui.add_space(4.0);

                        // 2. Account & Security Options (Row 2)
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);
                            ui.label(RichText::new("From:").size(12.0).strong().color(AppTheme::TEXT_MUTED));
                            let current_account = accounts.iter().find(|a| a.id == self.selected_account_id).unwrap_or(&accounts[0]);
                            let prev_account_id = self.selected_account_id.clone();
                            egui::ComboBox::from_id_salt("compose_from_combo")
                                .selected_text(format!("{} <{}>", current_account.name, current_account.email))
                                .show_ui(ui, |ui| {
                                    for acc in accounts {
                                        ui.selectable_value(&mut self.selected_account_id, acc.id.clone(), format!("{} <{}>", acc.name, acc.email));
                                    }
                                });

                            if self.selected_account_id != prev_account_id {
                                self.selected_signature_id = find_default_signature(signatures, Some(&self.selected_account_id)).map(|s| s.id.clone());
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.checkbox(&mut self.enable_pgp_signing, RichText::new("✍ Sign (PGP)").size(11.5))
                                    .on_hover_text("Sign email body with your account's private key (OpenPGP)");
                                ui.add_space(4.0);
                                ui.checkbox(&mut self.enable_pgp_encryption, RichText::new("🔒 Encrypt (PGP)").size(11.5))
                                    .on_hover_text("Encrypt email body with recipient's public key (OpenPGP)");
                            });
                        });

                        if let Some((success, ref msg)) = self.status_msg {
                            ui.add_space(2.0);
                            let col = if success { AppTheme::ACCENT_SUCCESS } else { AppTheme::ACCENT_DANGER };
                            ui.label(RichText::new(msg).size(11.0).color(col));
                        }

                        if self.show_custom_schedule_dialog {
                            ui.add_space(4.0);
                            egui::Frame::none().fill(AppTheme::BG_CARD).stroke(Stroke::new(1.0_f32, AppTheme::ACCENT_PRIMARY)).rounding(Rounding::same(6.0)).inner_margin(8.0).show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Schedule Send in:").strong().size(12.0));
                                    ui.add(egui::DragValue::new(&mut self.custom_schedule_hours).range(0..=720).prefix("Hours: "));
                                    ui.add(egui::DragValue::new(&mut self.custom_schedule_mins).range(0..=59).prefix("Mins: "));
                                    if ui.button(RichText::new("✓ Confirm Schedule").strong()).clicked() {
                                        let total_mins = (self.custom_schedule_hours as i64 * 60) + (self.custom_schedule_mins as i64);
                                        let target_ts = (Utc::now() + Duration::minutes(total_mins.max(1))).timestamp();
                                        if self.execute_schedule_send(accounts, signatures, keyring, storage, target_ts) {
                                            *status_toast = Some((format!("✓ Scheduled for {}h {}m from now", self.custom_schedule_hours, self.custom_schedule_mins), std::time::Instant::now()));
                                            *on_data_changed = true;
                                            self.show_custom_schedule_dialog = false;
                                        }
                                    }
                                    if ui.button("Cancel").clicked() { self.show_custom_schedule_dialog = false; }
                                });
                            });
                        }

                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);

                        // 3. To, Cc, Bcc
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("To:").size(12.5).color(AppTheme::TEXT_MUTED));
                            ui.add(egui::TextEdit::singleline(&mut self.to_input).desired_width(ui.available_width() - 80.0));
                            if ui.button(if self.show_cc_bcc { "Hide Cc" } else { "Cc/Bcc" }).clicked() { self.show_cc_bcc = !self.show_cc_bcc; }
                        });
                        if self.show_cc_bcc {
                            ui.horizontal(|ui| { ui.label("Cc:"); ui.text_edit_singleline(&mut self.cc_input); });
                            ui.horizontal(|ui| { ui.label("Bcc:"); ui.text_edit_singleline(&mut self.bcc_input); });
                        }
                        ui.add_space(4.0);
                        ui.horizontal(|ui| { ui.label("Subject:"); ui.text_edit_singleline(&mut self.subject); });

                        // Attachments List View
                        if !self.attachments.is_empty() {
                            ui.add_space(4.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.label(RichText::new("📎 Attachments:").size(12.0).strong().color(AppTheme::TEXT_MUTED));
                                let mut to_remove = None;
                                for (idx, att) in self.attachments.iter().enumerate() {
                                    let size_kb = att.size_bytes as f64 / 1024.0;
                                    let size_str = if size_kb > 1024.0 {
                                        format!("{:.1} MB", size_kb / 1024.0)
                                    } else {
                                        format!("{:.1} KB", size_kb)
                                    };
                                    egui::Frame::none()
                                        .fill(AppTheme::BG_CARD)
                                        .stroke(Stroke::new(1.0_f32, AppTheme::BORDER_SUBTLE))
                                        .rounding(Rounding::same(6.0))
                                        .inner_margin(egui::Margin::symmetric(6.0, 3.0))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(RichText::new(format!("📄 {} ({})", att.filename, size_str)).size(11.5));
                                                if ui.small_button(RichText::new("×").size(11.0).color(AppTheme::ACCENT_DANGER)).clicked() {
                                                    to_remove = Some(idx);
                                                }
                                            });
                                        });
                                }
                                if let Some(idx) = to_remove {
                                    self.attachments.remove(idx);
                                }
                            });
                        }

                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(4.0);

                        // 4. Dynamic Height Multiline Editor
                        let available_h = ui.available_height();
                        let extra_bottom_space = if self.reply_quote.is_some() { 240.0 } else { 120.0 };
                        let editor_min_h = (available_h - extra_bottom_space).max(180.0);

                        if self.format == ComposeFormat::Markdown && self.show_markdown_preview {
                            ui.label("Editor...");
                            ui.add(
                                egui::TextEdit::multiline(&mut self.body_plain)
                                    .desired_width(f32::INFINITY)
                                    .min_size(egui::vec2(0.0, editor_min_h))
                            );
                        } else {
                            ui.add(
                                egui::TextEdit::multiline(&mut self.body_plain)
                                    .desired_width(f32::INFINITY)
                                    .min_size(egui::vec2(0.0, editor_min_h))
                            );
                        }

                        // 5. Attached Signature Indicator & Live Preview
                        ui.add_space(8.0);
                        let attached_sig = self.selected_signature_id.as_ref().and_then(|id| {
                            signatures.iter().find(|s| &s.id == id)
                        });

                        egui::Frame::none()
                            .fill(AppTheme::BG_CARD)
                            .stroke(Stroke::new(1.0_f32, AppTheme::BORDER_SUBTLE))
                            .rounding(Rounding::same(6.0))
                            .inner_margin(egui::Margin::symmetric(10.0, 7.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    if let Some(sig) = attached_sig {
                                        ui.label(RichText::new("🖋️ Attached Signature:").size(12.0).strong().color(AppTheme::ACCENT_PRIMARY));
                                        ui.label(RichText::new(&sig.name).size(12.0).strong().color(AppTheme::TEXT_PRIMARY));

                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui.small_button(RichText::new("× Detach").color(AppTheme::ACCENT_DANGER)).on_hover_text("Do not attach signature to this email").clicked() {
                                                self.selected_signature_id = None;
                                            }

                                            egui::ComboBox::from_id_salt("compose_sig_switch_combo")
                                                .selected_text("Change")
                                                .show_ui(ui, |ui| {
                                                    for s in signatures {
                                                        let is_sel = self.selected_signature_id.as_deref() == Some(&s.id);
                                                        let label = if s.is_default { format!("{} [Default]", s.name) } else { s.name.clone() };
                                                        if ui.selectable_label(is_sel, label).clicked() {
                                                            self.selected_signature_id = Some(s.id.clone());
                                                        }
                                                    }
                                                });
                                        });
                                    } else {
                                        ui.label(RichText::new("🖋️ No signature attached").size(12.0).color(AppTheme::TEXT_MUTED));
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            egui::ComboBox::from_id_salt("compose_sig_add_combo")
                                                .selected_text("+ Attach Signature")
                                                .show_ui(ui, |ui| {
                                                    for s in signatures {
                                                        let label = if s.is_default { format!("{} [Default]", s.name) } else { s.name.clone() };
                                                        if ui.selectable_label(false, label).clicked() {
                                                            self.selected_signature_id = Some(s.id.clone());
                                                        }
                                                    }
                                                });
                                        });
                                    }
                                });

                                if let Some(sig) = attached_sig {
                                    let preview = email_html::html_to_plain_text(&sig.content_html);
                                    if !preview.trim().is_empty() {
                                        ui.add_space(3.0);
                                        ui.label(RichText::new(format!("--\n{}", preview.trim())).size(11.0).color(AppTheme::TEXT_MUTED));
                                    }
                                }
                            });

                        // 6. Quoted Original Message (Previous Email)
                        if let Some(ref mut quote) = self.reply_quote {
                            ui.add_space(8.0);
                            egui::Frame::none()
                                .fill(AppTheme::BG_VIEW)
                                .stroke(Stroke::new(1.0_f32, AppTheme::BORDER_SUBTLE))
                                .rounding(Rounding::same(6.0))
                                .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("💬 Quoted Original Message:").size(12.0).strong().color(AppTheme::TEXT_SECONDARY));

                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            let toggle_label = if self.show_quoted_text { "Hide Quote" } else { "Show / Edit Quote" };
                                            if ui.small_button(RichText::new(toggle_label).size(11.0)).clicked() {
                                                self.show_quoted_text = !self.show_quoted_text;
                                            }
                                            ui.checkbox(&mut self.include_quote, RichText::new("Include in email").size(11.5));
                                        });
                                    });

                                    if self.show_quoted_text {
                                        ui.add_space(4.0);
                                        if self.include_quote {
                                            ui.label(RichText::new("Original email text below will be quoted in your reply (you can edit or trim it):").size(11.0).color(AppTheme::TEXT_MUTED));
                                        } else {
                                            ui.label(RichText::new("Original email text (will NOT be included in outgoing email):").size(11.0).color(AppTheme::ACCENT_WARNING));
                                        }
                                        ui.add_space(2.0);
                                        ui.add(
                                            egui::TextEdit::multiline(quote)
                                                .desired_rows(6)
                                                .desired_width(f32::INFINITY)
                                                .text_color(if self.include_quote { AppTheme::TEXT_MUTED } else { AppTheme::BORDER_SUBTLE })
                                        );
                                    }
                                });
                        }

                        ui.add_space(10.0);
                    });
            });
        self.is_open = self.is_open && is_open;
    }

    fn build_outgoing_draft(
        &self,
        signatures: &[Signature],
        accounts: &[Account],
        storage: &Storage,
    ) -> Result<OutgoingDraft, String> {
        if self.to_input.trim().is_empty() {
            return Err("Please specify at least one recipient email address.".to_string());
        }

        let to_list: Vec<Recipient> = self
            .to_input
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|email| Recipient::new(None, email.to_string()))
            .collect();

        let cc_list: Vec<Recipient> = self
            .cc_input
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|email| Recipient::new(None, email.to_string()))
            .collect();

        let bcc_list: Vec<Recipient> = self
            .bcc_input
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|email| Recipient::new(None, email.to_string()))
            .collect();

        // Build signature parts
        let attached_sig = self.selected_signature_id.as_ref().and_then(|id| {
            signatures.iter().find(|s| &s.id == id)
        });

        let (sig_plain, sig_html) = if let Some(sig) = attached_sig {
            let clean_html = email_html::sanitize_raw_html(&sig.content_html);
            let plain = email_html::html_to_plain_text(&sig.content_html);
            (format!("\n\n--\n{}", plain), format!("<br/><br/>--<br/>{}", clean_html))
        } else {
            (String::new(), String::new())
        };

        // Build quote parts
        let (quote_plain, quote_html) = if self.include_quote {
            if let Some(ref quote) = self.reply_quote {
                if !quote.trim().is_empty() {
                    (
                        format!("\n\n---\nOn previous discussion, wrote:\n{}", quote),
                        format!("<br/><br/>---<br/>On previous discussion, wrote:<br/>{}", quote.replace('\n', "<br/>"))
                    )
                } else {
                    (String::new(), String::new())
                }
            } else {
                (String::new(), String::new())
            }
        } else {
            (String::new(), String::new())
        };

        let (body_plain, body_html) = match self.format {
            ComposeFormat::Markdown => {
                let generated_html = email_html::markdown_to_html(&self.body_plain);
                let user_plain = self.body_plain.clone();
                let plain = format!("{}{}{}", user_plain, sig_plain, quote_plain);
                let html = format!(
                    "<div style=\"font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; font-size: 14px; line-height: 1.5; color: #222222;\">{}{}{}</div>",
                    generated_html,
                    sig_html,
                    quote_html
                );
                (plain, Some(html))
            }
            ComposeFormat::Html => {
                let user_plain = email_html::html_to_plain_text(&self.body_plain);
                let plain = format!("{}{}{}", user_plain, sig_plain, quote_plain);
                let html = format!(
                    "<div style=\"font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; font-size: 14px; line-height: 1.5; color: #222222;\">{}{}{}</div>",
                    self.body_plain.replace('\n', "<br/>"),
                    sig_html,
                    quote_html
                );
                (plain, Some(html))
            }
            ComposeFormat::PlainText => {
                let user_plain = email_html::html_to_plain_text(&self.body_plain);
                let plain = format!("{}{}{}", user_plain, sig_plain, quote_plain);
                (plain, None)
            }
        };

        let mut final_plain = body_plain;
        let mut final_html = body_html;
        let mut final_subject = self.subject.clone();

        if self.enable_pgp_encryption {
            let first_to = to_list.first().map(|r| r.email.clone()).unwrap_or_default();
            let mut recipient_key = storage.get_pgp_key(&first_to).ok().flatten();
            if recipient_key.is_none() {
                if let Some(acc) = accounts.iter().find(|a| a.id == self.selected_account_id) {
                    recipient_key = storage.get_pgp_key(&acc.email).ok().flatten();
                }
            }

            if let Some(key) = recipient_key {
                let encrypted = email_core::pgp::pgp_encrypt(&final_plain, &key.public_key_armored)
                    .map_err(|e| format!("PGP encryption failed: {}", e))?;
                final_plain = encrypted;
                final_html = None;
                if !final_subject.to_lowercase().contains("encrypted") {
                    final_subject = format!("[PGP Encrypted] {}", final_subject);
                }
            } else {
                return Err(format!("No PGP public key found for recipient '{}'. Please import their public key in Settings -> Security (PGP).", first_to));
            }
        } else if self.enable_pgp_signing {
            let current_account = accounts.iter().find(|a| a.id == self.selected_account_id);
            if let Some(acc) = current_account {
                if let Some(key) = storage.get_pgp_key(&acc.email).ok().flatten() {
                    if !key.private_key_armored.is_empty() {
                        let signed = email_core::pgp::pgp_sign(&final_plain, &key.private_key_armored)
                            .map_err(|e| format!("PGP signing failed: {}", e))?;
                        final_plain = signed;
                    }
                }
            }
        }

        Ok(OutgoingDraft {
            account_id: self.selected_account_id.clone(),
            to: to_list,
            cc: cc_list,
            bcc: bcc_list,
            subject: final_subject,
            body_plain: final_plain,
            body_html: final_html,
            in_reply_to: self.in_reply_to.clone(),
            references: self.references.clone(),
            attachments: self.attachments.clone(),
        })
    }

    fn execute_send(
        &mut self,
        accounts: &[Account],
        signatures: &[Signature],
        keyring: &Arc<dyn CredentialStore>,
        on_schedule_send: &mut Option<(OutgoingDraft, String)>,
        storage: &Storage,
    ) -> bool {
        match self.build_outgoing_draft(signatures, accounts, storage) {
            Ok(draft) => {
                let current_account = accounts.iter().find(|a| a.id == self.selected_account_id);
                if let Some(acc) = current_account {
                    match keyring.get_credential(&acc.credential_key) {
                        Ok(pwd) => {
                            if let Some(ref did) = self.draft_id {
                                let _ = storage.delete_draft(did);
                            }
                            *on_schedule_send = Some((draft, pwd));
                            self.is_open = false;
                            true
                        }
                        Err(e) => {
                            self.error_msg = Some(format!("Could not retrieve account credentials: {}", e));
                            false
                        }
                    }
                } else {
                    self.error_msg = Some("Account not found.".to_string());
                    false
                }
            }
            Err(e) => {
                self.error_msg = Some(e);
                false
            }
        }
    }

    fn execute_schedule_send(
        &mut self,
        accounts: &[Account],
        signatures: &[Signature],
        _keyring: &Arc<dyn CredentialStore>,
        storage: &Storage,
        target_timestamp: i64,
    ) -> bool {
        match self.build_outgoing_draft(signatures, accounts, storage) {
            Ok(draft) => {
                let current_account = accounts.iter().find(|a| a.id == self.selected_account_id);
                if let Some(acc) = current_account {
                    let scheduled = ScheduledEmail::new(acc.id.clone(), draft, target_timestamp);
                    if let Err(e) = storage.save_scheduled_email(&scheduled) {
                        self.error_msg = Some(format!("Failed to schedule email: {}", e));
                        return false;
                    }

                    if let Some(ref did) = self.draft_id {
                        let _ = storage.delete_draft(did);
                    }

                    self.is_open = false;
                    true
                } else {
                    self.error_msg = Some("Account not found.".to_string());
                    false
                }
            }
            Err(e) => {
                self.error_msg = Some(e);
                false
            }
        }
    }
}

pub fn build_reply_all_recipients(
    header: &MessageHeader,
    my_emails: &std::collections::HashSet<String>,
) -> (String, String) {
    let from_clean = header.from_address.trim().to_string();
    let from_lower = from_clean.to_lowercase();

    let mut to_list: Vec<String> = Vec::new();
    let mut cc_list: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    // If 'from' is not one of our own accounts, 'from' is the primary recipient
    if !from_clean.is_empty() && !my_emails.contains(&from_lower) {
        to_list.push(from_clean.clone());
        seen.insert(from_lower);
    }

    // Process all 'to_recipients' from original header
    for r in &header.to_recipients {
        let email_clean = r.email.trim().to_string();
        let email_lower = email_clean.to_lowercase();
        if email_clean.is_empty() || my_emails.contains(&email_lower) || seen.contains(&email_lower) {
            continue;
        }
        seen.insert(email_lower);
        if to_list.is_empty() {
            to_list.push(email_clean);
        } else {
            cc_list.push(email_clean);
        }
    }

    // Process all 'cc_recipients' from original header
    for r in &header.cc_recipients {
        let email_clean = r.email.trim().to_string();
        let email_lower = email_clean.to_lowercase();
        if email_clean.is_empty() || my_emails.contains(&email_lower) || seen.contains(&email_lower) {
            continue;
        }
        seen.insert(email_lower);
        if to_list.is_empty() {
            to_list.push(email_clean);
        } else {
            cc_list.push(email_clean);
        }
    }

    (to_list.join(", "), cc_list.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_default_signature() {
        let sig1 = Signature::new(Some("acc1".to_string()), "Acc1 Sig".to_string(), "Acc 1 Content".to_string(), true);
        let sig2 = Signature::new(None, "Global Sig".to_string(), "Global Content".to_string(), true);
        let sig3 = Signature::new(Some("acc2".to_string()), "Acc2 Other".to_string(), "Acc 2 Other Content".to_string(), false);

        let sigs = vec![sig1, sig2, sig3];

        // Should find account-specific default
        let found = find_default_signature(&sigs, Some("acc1"));
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Acc1 Sig");

        // Should find global default for unmatched account
        let found_other = find_default_signature(&sigs, Some("acc3"));
        assert!(found_other.is_some());
        assert_eq!(found_other.unwrap().name, "Global Sig");
    }

    #[test]
    fn test_open_new_with_default_signature() {
        let mut compose = ComposeView::new();
        let sig = Signature::new(None, "Default".to_string(), "<b>Best Regards</b><br/>User".to_string(), true);
        let sigs = vec![sig];

        compose.open_new(Some("acc1"), &sigs);
        assert!(compose.is_open);
        assert!(compose.send_as_html); // HTML default
        assert!(compose.selected_signature_id.is_some());
        assert_eq!(compose.body_plain, ""); // Clean editor
    }

    #[test]
    fn test_open_reply_text_only() {
        let mut compose = ComposeView::new();
        let sig = Signature::new(None, "Default".to_string(), "<b>Kind regards</b><br/>Sender".to_string(), true);
        let sigs = vec![sig];

        compose.open_reply(
            "acc1",
            "test@example.com",
            "",
            "Hello",
            Some("msg-123".to_string()),
            "This is the previous message.",
            &sigs,
            false, // Text only reply
        );

        assert!(compose.is_open);
        assert!(!compose.send_as_html); // Plain text mode
        assert_eq!(compose.subject, "Re: Hello");
        assert!(compose.selected_signature_id.is_some());
        assert!(compose.reply_quote.as_ref().unwrap().contains("This is the previous message."));
    }

    #[test]
    fn test_build_reply_all_recipients_excludes_self() {
        let mut my_emails = std::collections::HashSet::new();
        my_emails.insert("kunal@abhashtech.com".to_string());

        let header = MessageHeader {
            id: "msg-1".to_string(),
            account_id: "acc-1".to_string(),
            folder_id: "inbox".to_string(),
            uid: 100,
            message_id: Some("mid-1".to_string()),
            in_reply_to: None,
            subject: "Project Update".to_string(),
            from_name: Some("Alice".to_string()),
            from_address: "alice@work.com".to_string(),
            to_recipients: vec![
                Recipient::new(Some("Kunal".to_string()), "kunal@abhashtech.com".to_string()),
                Recipient::new(Some("Bob".to_string()), "bob@work.com".to_string()),
            ],
            cc_recipients: vec![
                Recipient::new(Some("Carol".to_string()), "carol@work.com".to_string()),
            ],
            date_epoch: 1234567890,
            snippet: "Snippet".to_string(),
            is_read: true,
            is_flagged: false,
            is_draft: false,
            is_deleted: false,
            body_fetched: true,
            size_bytes: 1024,
            snooze_until: None,
        };

        let (to, cc) = build_reply_all_recipients(&header, &my_emails);
        assert_eq!(to, "alice@work.com");
        assert_eq!(cc, "bob@work.com, carol@work.com");
        assert!(!to.contains("kunal@abhashtech.com"));
        assert!(!cc.contains("kunal@abhashtech.com"));
    }

    #[test]
    fn test_markdown_compose_mode() {
        let mut compose = ComposeView::new();
        compose.open_new(Some("acc1"), &[]);
        assert_eq!(compose.format, ComposeFormat::Markdown);
        assert!(!compose.show_markdown_preview);

        compose.show_markdown_preview = true;
        assert!(compose.show_markdown_preview);
    }
}


