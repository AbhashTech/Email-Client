use crate::theme::AppTheme;
use egui::{Color32, RichText, Rounding, Stroke, Window};
use email_core::events::SyncCommand;
use email_core::models::{Account, MessageHeader, OutgoingDraft, Recipient, Signature, Template};
use email_keychain::CredentialStore;
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposeFormat {
    Html,
    PlainText,
    Markdown,
}

pub struct ComposeView {
    pub is_open: bool,
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
        }
    }

    pub fn open_new(&mut self, default_account_id: Option<&str>, signatures: &[Signature]) {
        self.is_open = true;
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
        Window::new("✉ Compose Email")
            .open(&mut is_open)
            .collapsible(false)
            .resizable(true)
            .default_width(680.0)
            .default_height(480.0)
            .show(ctx, |ui| {
                if accounts.is_empty() {
                    ui.label("No email accounts configured. Please add an account first.");
                    return;
                }

                if self.selected_account_id.is_empty() {
                    self.selected_account_id = accounts[0].id.clone();
                    if self.selected_signature_id.is_none() {
                        self.selected_signature_id = find_default_signature(signatures, Some(&self.selected_account_id)).map(|s| s.id.clone());
                    }
                }

                // 1. Top Action Toolbar (Send, From account, Templates, Signatures, Discard)
                ui.horizontal(|ui| {
                    let send_btn_label = if self.send_as_html {
                        "🚀 Send"
                    } else {
                        "🚀 Send (Text)"
                    };

                    let top_send_btn = egui::Button::new(
                        RichText::new(send_btn_label)
                            .size(12.5)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(AppTheme::ACCENT_PRIMARY)
                    .rounding(Rounding::same(6.0));

                    if ui.add(top_send_btn).clicked() {
                        self.execute_send(accounts, signatures, cmd_tx, keyring);
                    }

                    ui.add_space(4.0);

                    ui.label(RichText::new("From:").size(12.0).color(AppTheme::TEXT_MUTED));
                    let current_account = accounts.iter().find(|a| a.id == self.selected_account_id).unwrap_or(&accounts[0]);
                    let prev_account_id = self.selected_account_id.clone();

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

                    if self.selected_account_id != prev_account_id {
                        self.selected_signature_id = find_default_signature(signatures, Some(&self.selected_account_id)).map(|s| s.id.clone());
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(RichText::new("Discard").size(11.5)).clicked() {
                            self.is_open = false;
                        }

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

                        // Signature picker dropdown
                        let selected_sig_label = if let Some(ref sig_id) = self.selected_signature_id {
                            signatures.iter().find(|s| &s.id == sig_id).map(|s| s.name.as_str()).unwrap_or("(None)")
                        } else {
                            "(None)"
                        };

                        egui::ComboBox::from_id_salt("sig_picker_combo")
                            .selected_text(format!("Sig: {}", selected_sig_label))
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(self.selected_signature_id.is_none(), "None (No Signature)").clicked() {
                                    self.selected_signature_id = None;
                                }
                                for s in signatures {
                                    let is_sel = self.selected_signature_id.as_deref() == Some(&s.id);
                                    let label = if s.is_default { format!("{} [Default]", s.name) } else { s.name.clone() };
                                    if ui.selectable_label(is_sel, label).clicked() {
                                        self.selected_signature_id = Some(s.id.clone());
                                    }
                                }
                            });
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

                // 4. Format Selector & Rich Formatting Action Bar
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Format:").size(12.0).color(AppTheme::TEXT_MUTED));
                    if ui.selectable_label(self.format == ComposeFormat::Markdown, "⚡ Markdown").on_hover_text("Write in Markdown with live preview").clicked() {
                        self.format = ComposeFormat::Markdown;
                        self.send_as_html = true;
                    }
                    if ui.selectable_label(self.format == ComposeFormat::Html, "🌐 HTML").on_hover_text("Send rich HTML email").clicked() {
                        self.format = ComposeFormat::Html;
                        self.send_as_html = true;
                    }
                    if ui.selectable_label(self.format == ComposeFormat::PlainText, "📝 Plain Text").on_hover_text("Send text-only email without HTML markup").clicked() {
                        self.format = ComposeFormat::PlainText;
                        self.send_as_html = false;
                    }

                    ui.separator();

                    match self.format {
                        ComposeFormat::Markdown => {
                            if ui.button(RichText::new("B").strong()).on_hover_text("Bold **text**").clicked() {
                                self.body_plain.push_str("**text**");
                            }
                            if ui.button(RichText::new("I").italics()).on_hover_text("Italic *text*").clicked() {
                                self.body_plain.push_str("*text*");
                            }
                            if ui.button(RichText::new("🔗").size(12.0)).on_hover_text("Insert Link [title](url)").clicked() {
                                self.body_plain.push_str("[Link text](https://)");
                            }
                            if ui.button(RichText::new("H1").strong()).on_hover_text("Heading 1 # ").clicked() {
                                self.body_plain.push_str("\n# ");
                            }
                            if ui.button(RichText::new("H2").strong()).on_hover_text("Heading 2 ## ").clicked() {
                                self.body_plain.push_str("\n## ");
                            }
                            if ui.button(RichText::new("• List").size(11.0)).on_hover_text("Bullet list - ").clicked() {
                                self.body_plain.push_str("\n- ");
                            }
                            if ui.button(RichText::new("> Quote").size(11.0)).on_hover_text("Blockquote > ").clicked() {
                                self.body_plain.push_str("\n> ");
                            }
                            if ui.button(RichText::new("💻 Code").size(11.0)).on_hover_text("Code block ```").clicked() {
                                self.body_plain.push_str("\n```\n\n```\n");
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let preview_icon = if self.show_markdown_preview { "👁 Hide Preview" } else { "👁 Live Preview" };
                                if ui.selectable_label(self.show_markdown_preview, preview_icon).on_hover_text("Toggle live side-by-side Markdown HTML preview").clicked() {
                                    self.show_markdown_preview = !self.show_markdown_preview;
                                }
                            });
                        }
                        ComposeFormat::Html => {
                            if ui.button(RichText::new("B").strong()).on_hover_text("Bold <b></b>").clicked() {
                                self.body_plain.push_str("<b></b>");
                            }
                            if ui.button(RichText::new("I").italics()).on_hover_text("Italic <i></i>").clicked() {
                                self.body_plain.push_str("<i></i>");
                            }
                            if ui.button(RichText::new("🔗").size(12.0)).on_hover_text("Insert Link <a href=\"...\">").clicked() {
                                self.body_plain.push_str("<a href=\"https://\">Link text</a>");
                            }
                            if ui.button(RichText::new("• List").size(11.0)).on_hover_text("Bullet list").clicked() {
                                self.body_plain.push_str("\n • ");
                            }
                            if ui.button(RichText::new("> Quote").size(11.0)).on_hover_text("Blockquote").clicked() {
                                self.body_plain.push_str("\n > ");
                            }
                        }
                        ComposeFormat::PlainText => {
                            ui.label(RichText::new("Plain text mode active").size(11.0).color(AppTheme::TEXT_MUTED));
                        }
                    }
                });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // 5. Body Text Editor & Content
                if self.format == ComposeFormat::Markdown && self.show_markdown_preview {
                    ui.columns(2, |columns| {
                        // Left: Editor
                        columns[0].vertical(|ui| {
                            ui.label(RichText::new("✏ Markdown Editor").size(11.0).color(AppTheme::TEXT_MUTED));
                            ui.add_space(2.0);
                            egui::ScrollArea::vertical()
                                .id_salt("compose_md_editor")
                                .max_height(260.0)
                                .auto_shrink([false; 2])
                                .show(ui, |ui| {
                                    ui.add(
                                        egui::TextEdit::multiline(&mut self.body_plain)
                                            .hint_text("Write Markdown here (#, **, *, -, ```, etc)...")
                                            .font(egui::TextStyle::Monospace)
                                            .desired_width(f32::INFINITY)
                                            .desired_rows(12),
                                    );
                                });
                        });

                        // Right: Live Preview
                        columns[1].vertical(|ui| {
                            ui.label(RichText::new("👁 Live Preview").size(11.0).color(AppTheme::ACCENT_PRIMARY));
                            ui.add_space(2.0);
                            let compiled_html = email_html::markdown_to_html(&self.body_plain);
                            let blocks = email_html::parse_html_to_blocks(&compiled_html);

                            egui::Frame::none()
                                .fill(Color32::from_rgb(22, 27, 40))
                                .stroke(Stroke::new(1.0_f32, AppTheme::BORDER_SUBTLE))
                                .rounding(Rounding::same(6.0))
                                .inner_margin(8.0)
                                .show(ui, |ui| {
                                    egui::ScrollArea::vertical()
                                        .id_salt("compose_md_preview")
                                        .max_height(244.0)
                                        .auto_shrink([false; 2])
                                        .show(ui, |ui| {
                                            if self.body_plain.trim().is_empty() {
                                                ui.label(RichText::new("Live preview will appear here as you type...").italics().color(AppTheme::TEXT_MUTED));
                                            } else {
                                                for block in &blocks {
                                                    match block {
                                                        email_html::HtmlBlock::Heading { level, text, .. } => {
                                                            let size = match level { 1 => 18.0, 2 => 15.0, _ => 13.0 };
                                                            ui.label(RichText::new(text).size(size).strong().color(Color32::WHITE));
                                                        }
                                                        email_html::HtmlBlock::Paragraph { spans, .. } => {
                                                            ui.horizontal_wrapped(|ui| {
                                                                for span in spans {
                                                                    let mut rt = RichText::new(&span.text).size(12.5);
                                                                    if span.link_url.is_some() {
                                                                        rt = rt.color(AppTheme::ACCENT_PRIMARY).underline();
                                                                    } else {
                                                                        rt = rt.color(AppTheme::TEXT_PRIMARY);
                                                                    }
                                                                    if matches!(span.style, email_html::TextStyle::Bold | email_html::TextStyle::BoldItalic) {
                                                                        rt = rt.strong();
                                                                    }
                                                                    if matches!(span.style, email_html::TextStyle::Italic | email_html::TextStyle::BoldItalic) {
                                                                        rt = rt.italics();
                                                                    }
                                                                    ui.label(rt);
                                                                }
                                                            });
                                                        }
                                                        email_html::HtmlBlock::CodeBlock(code) => {
                                                            egui::Frame::none()
                                                                .fill(Color32::from_rgb(14, 18, 28))
                                                                .rounding(Rounding::same(4.0))
                                                                .inner_margin(6.0)
                                                                .show(ui, |ui| {
                                                                    ui.label(RichText::new(code).font(egui::FontId::monospace(11.5)).color(Color32::from_rgb(220, 230, 245)));
                                                                });
                                                        }
                                                        email_html::HtmlBlock::Blockquote(quote) => {
                                                            ui.horizontal(|ui| {
                                                                ui.label(RichText::new("▎").color(AppTheme::ACCENT_PRIMARY));
                                                                ui.label(RichText::new(quote).italics().color(AppTheme::TEXT_SECONDARY));
                                                            });
                                                        }
                                                        email_html::HtmlBlock::ListItem(spans) => {
                                                            ui.horizontal(|ui| {
                                                                ui.label(RichText::new("•").size(12.0).color(AppTheme::ACCENT_PRIMARY));
                                                                for span in spans {
                                                                    ui.label(RichText::new(&span.text).size(12.0).color(AppTheme::TEXT_PRIMARY));
                                                                }
                                                            });
                                                        }
                                                        _ => {}
                                                    }
                                                    ui.add_space(2.0);
                                                }
                                            }
                                        });
                                });
                        });
                    });
                } else {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut self.body_plain)
                                    .hint_text("Type your message here...")
                                    .font(egui::TextStyle::Body)
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(13),
                            );

                            ui.add_space(6.0);

                            // Attached Signature preview info
                            if let Some(ref sig_id) = self.selected_signature_id {
                                if let Some(sig) = signatures.iter().find(|s| &s.id == sig_id) {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("Signature:").size(11.0).color(AppTheme::TEXT_MUTED));
                                        ui.label(RichText::new(&sig.name).size(11.0).strong().color(AppTheme::ACCENT_PRIMARY));
                                        let plain_preview = email_html::html_to_plain_text(&sig.content_html);
                                        let truncated = if plain_preview.len() > 50 {
                                            format!("{}...", &plain_preview[..47])
                                        } else {
                                            plain_preview
                                        };
                                        ui.label(RichText::new(format!("({})", truncated)).size(10.5).color(AppTheme::TEXT_MUTED));
                                    });
                                }
                            }

                            // Quoted Previous Message expandable
                            if let Some(ref quote) = self.reply_quote {
                                ui.add_space(4.0);
                                ui.collapsing(RichText::new("Quoted Previous Message").size(11.0).color(AppTheme::TEXT_MUTED), |ui| {
                                    ui.label(RichText::new(quote).size(11.0).color(AppTheme::TEXT_SECONDARY));
                                });
                            }

                            if let Some(ref err) = self.error_msg {
                                ui.add_space(4.0);
                                ui.label(RichText::new(err).color(AppTheme::ACCENT_DANGER));
                            }

                            ui.add_space(8.0);
                        });
                }
            });

        self.is_open = self.is_open && is_open;
    }

    fn execute_send(
        &mut self,
        accounts: &[Account],
        signatures: &[Signature],
        cmd_tx: &mpsc::UnboundedSender<SyncCommand>,
        keyring: &Arc<dyn CredentialStore>,
    ) {
        if self.to_input.trim().is_empty() {
            self.error_msg = Some("Please specify at least one recipient email address.".to_string());
            return;
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
        let (quote_plain, quote_html) = if let Some(ref quote) = self.reply_quote {
            (
                format!("\n\n---\nOn previous discussion, wrote:\n{}", quote),
                format!("<br/><br/>---<br/>On previous discussion, wrote:<br/>{}", quote.replace('\n', "<br/>"))
            )
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

        let draft = OutgoingDraft {
            account_id: self.selected_account_id.clone(),
            to: to_list,
            cc: cc_list,
            bcc: bcc_list,
            subject: self.subject.clone(),
            body_plain,
            body_html,
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


