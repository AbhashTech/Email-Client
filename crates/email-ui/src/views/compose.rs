use crate::theme::AppTheme;
use egui::{Color32, RichText, Rounding, Window};
use email_core::events::SyncCommand;
use email_core::models::{Account, OutgoingDraft, Recipient, Signature, Template};
use email_keychain::CredentialStore;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct ComposeView {
    pub is_open: bool,
    pub selected_account_id: String,
    pub to_input: String,
    pub cc_input: String,
    pub bcc_input: String,
    pub subject: String,
    pub body_plain: String,
    pub show_cc_bcc: bool,
    pub in_reply_to: Option<String>,
    pub references: Option<String>,
    pub error_msg: Option<String>,
}

impl ComposeView {
    pub fn new() -> Self {
        Self {
            is_open: false,
            selected_account_id: String::new(),
            to_input: String::new(),
            cc_input: String::new(),
            bcc_input: String::new(),
            subject: String::new(),
            body_plain: String::new(),
            show_cc_bcc: false,
            in_reply_to: None,
            references: None,
            error_msg: None,
        }
    }

    pub fn open_new(&mut self, default_account_id: Option<&str>) {
        self.is_open = true;
        self.to_input.clear();
        self.cc_input.clear();
        self.bcc_input.clear();
        self.subject.clear();
        self.body_plain.clear();
        self.show_cc_bcc = false;
        self.in_reply_to = None;
        self.references = None;
        self.error_msg = None;
        if let Some(aid) = default_account_id {
            self.selected_account_id = aid.to_string();
        }
    }

    pub fn open_reply(&mut self, account_id: &str, to: &str, subject: &str, in_reply_to: Option<String>, body_quote: &str) {
        self.is_open = true;
        self.selected_account_id = account_id.to_string();
        self.to_input = to.to_string();
        self.cc_input.clear();
        self.bcc_input.clear();
        self.subject = if subject.to_lowercase().starts_with("re:") {
            subject.to_string()
        } else {
            format!("Re: {}", subject)
        };
        self.in_reply_to = in_reply_to.clone();
        self.references = in_reply_to;
        self.body_plain = format!("\n\n---\nOn previous discussion, wrote:\n{}", body_quote);
        self.error_msg = None;
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        accounts: &[Account],
        templates: &[Template],
        signatures: &[Signature],
        cmd_tx: &mpsc::UnboundedSender<SyncCommand>,
        keyring: &Arc<dyn CredentialStore>,
    ) {
        if !self.is_open {
            return;
        }

        let mut is_open = self.is_open;
        Window::new("✉ Compose New Email")
            .open(&mut is_open)
            .collapsible(false)
            .resizable(true)
            .default_width(680.0)
            .default_height(540.0)
            .show(ctx, |ui| {
                if accounts.is_empty() {
                    ui.label("No email accounts configured. Please add an account first.");
                    return;
                }

                if self.selected_account_id.is_empty() {
                    self.selected_account_id = accounts[0].id.clone();
                }

                // 1. Account Selector & Quick Snippets Toolbar
                ui.horizontal(|ui| {
                    ui.label(RichText::new("From:").size(12.5).color(AppTheme::TEXT_MUTED));
                    let current_account = accounts.iter().find(|a| a.id == self.selected_account_id).unwrap_or(&accounts[0]);
                    egui::ComboBox::from_id_salt("compose_from_combo")
                        .selected_text(format!("{} <{}>", current_account.name, current_account.email))
                        .show_ui(ui, |ui| {
                            for acc in accounts {
                                ui.selectable_value(
                                    &mut self.selected_account_id,
                                    acc.id.clone(),
                                    format!("{} <{}>", acc.name, acc.email),
                                );
                            }
                        });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Templates picker
                        if !templates.is_empty() {
                            egui::ComboBox::from_id_salt("template_picker_combo")
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

                        // Signature picker
                        if !signatures.is_empty() {
                            egui::ComboBox::from_id_salt("sig_picker_combo")
                                .selected_text("✍ Signature")
                                .show_ui(ui, |ui| {
                                    for s in signatures {
                                        if ui.button(&s.name).clicked() {
                                            self.body_plain.push_str("\n\n--\n");
                                            self.body_plain.push_str(&email_html::html_to_plain_text(&s.content_html));
                                        }
                                    }
                                });
                        }
                    });
                });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // 2. To & Cc/Bcc Fields
                ui.horizontal(|ui| {
                    ui.label(RichText::new("To:").size(12.5).color(AppTheme::TEXT_MUTED));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.to_input)
                            .hint_text("recipient@example.com")
                            .desired_width(ui.available_width() - 80.0),
                    );

                    let toggle_label = if self.show_cc_bcc { "Hide Cc" } else { "Cc/Bcc" };
                    if ui.button(RichText::new(toggle_label).size(11.0)).clicked() {
                        self.show_cc_bcc = !self.show_cc_bcc;
                    }
                });

                if self.show_cc_bcc {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Cc:").size(12.5).color(AppTheme::TEXT_MUTED));
                        ui.text_edit_singleline(&mut self.cc_input);
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Bcc:").size(12.5).color(AppTheme::TEXT_MUTED));
                        ui.text_edit_singleline(&mut self.bcc_input);
                    });
                }

                // 3. Subject Line
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Subject:").size(12.5).color(AppTheme::TEXT_MUTED));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.subject)
                            .hint_text("Enter subject...")
                            .desired_width(ui.available_width() - 8.0),
                    );
                });

                ui.add_space(6.0);

                // 4. Rich Formatting Action Bar
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("B").strong()).on_hover_text("Bold").clicked() {
                        self.body_plain.push_str("<b></b>");
                    }
                    if ui.button(RichText::new("I").italics()).on_hover_text("Italic").clicked() {
                        self.body_plain.push_str("<i></i>");
                    }
                    if ui.button(RichText::new("🔗").size(12.0)).on_hover_text("Insert Link").clicked() {
                        self.body_plain.push_str("<a href=\"https://\">Link text</a>");
                    }
                    if ui.button(RichText::new("• List").size(11.0)).on_hover_text("Bullet list").clicked() {
                        self.body_plain.push_str("\n • ");
                    }
                    if ui.button(RichText::new("❝ Quote").size(11.0)).on_hover_text("Blockquote").clicked() {
                        self.body_plain.push_str("\n > ");
                    }
                });

                ui.add_space(4.0);

                // 5. Body Text Editor
                let text_height = ui.available_height() - 55.0;
                ui.add_sized(
                    [ui.available_width(), text_height],
                    egui::TextEdit::multiline(&mut self.body_plain)
                        .hint_text("Type your message here...")
                        .font(egui::TextStyle::Body),
                );

                if let Some(ref err) = self.error_msg {
                    ui.label(RichText::new(err).color(AppTheme::ACCENT_DANGER));
                }

                ui.add_space(8.0);

                // 6. Send / Discard Action Bar
                ui.horizontal(|ui| {
                    let send_btn = egui::Button::new(
                        RichText::new("🚀 Send Message")
                            .size(13.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(AppTheme::ACCENT_PRIMARY)
                    .rounding(Rounding::same(6.0));

                    if ui.add(send_btn).clicked() {
                        if self.to_input.trim().is_empty() {
                            self.error_msg = Some("Please specify at least one recipient email address.".to_string());
                        } else {
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

                            let draft = OutgoingDraft {
                                account_id: self.selected_account_id.clone(),
                                to: to_list,
                                cc: cc_list,
                                bcc: bcc_list,
                                subject: self.subject.clone(),
                                body_plain: self.body_plain.clone(),
                                body_html: Some(format!(
                                    "<div>{}</div>",
                                    self.body_plain.replace('\n', "<br/>")
                                )),
                                in_reply_to: self.in_reply_to.clone(),
                                references: self.references.clone(),
                            };

                            let current_account = accounts.iter().find(|a| a.id == self.selected_account_id);
                            if let Some(acc) = current_account {
                                match keyring.get_credential(&acc.credential_key) {
                                    Ok(pwd) => {
                                        let _ = cmd_tx.send(SyncCommand::SendEmail {
                                            draft,
                                            password: pwd,
                                        });
                                        self.is_open = false;
                                    }
                                    Err(e) => {
                                        self.error_msg = Some(format!("Could not retrieve account credentials from OS Keyring: {}", e));
                                    }
                                }
                            }
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(RichText::new("Discard").size(12.0)).clicked() {
                            self.is_open = false;
                        }
                    });
                });
            });

        self.is_open = self.is_open && is_open;
    }
}

