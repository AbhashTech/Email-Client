use base64::Engine;
use chrono::Datelike;
use crate::theme::AppTheme;
use egui::{Color32, FontId, RichText, Rounding, ScrollArea, Sense, Stroke, Ui, Vec2};
use email_core::events::SyncCommand;
use email_core::models::{Folder, MessageDetail};
use email_html::{parse_html_to_blocks, FormattedSpan, HtmlBlock, TextStyle};
use tokio::sync::mpsc;

pub struct MessageViewPane;

impl MessageViewPane {
    pub fn show(
        ui: &mut Ui,
        thread_messages: &[MessageDetail],
        folders: &[Folder],
        allowed_remote_images: &mut std::collections::HashSet<String>,
        cmd_tx: &mpsc::UnboundedSender<SyncCommand>,
        on_reply: &mut Option<MessageDetail>,
        on_reply_plain: &mut Option<MessageDetail>,
        on_reply_all: &mut Option<MessageDetail>,
        on_forward: &mut Option<MessageDetail>,
        on_edit_draft: &mut Option<MessageDetail>,
        on_delete: &mut Option<String>,
        on_toggle_read: &mut Option<(String, bool)>,
        on_move_folder: &mut Option<(String, String)>,
        on_snooze: &mut Option<(String, Option<i64>)>,
        status_toast: &mut Option<String>,
    ) {
        if thread_messages.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(140.0);
                ui.label(RichText::new("📬").size(48.0));
                ui.add_space(12.0);
                ui.heading(RichText::new("No Email Selected").size(18.0).color(AppTheme::TEXT_SECONDARY));
                ui.label(
                    RichText::new("Choose a conversation from the message list to view its contents")
                        .size(13.0)
                        .color(AppTheme::TEXT_MUTED),
                );
            });
            return;
        }

        let latest_detail = &thread_messages[thread_messages.len() - 1];
        let msg = &latest_detail.header;

        // 1. Action Toolbar
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
            ui.spacing_mut().button_padding = Vec2::new(8.0, 4.5);

            if msg.is_draft {
                let edit_btn = egui::Button::new(RichText::new("✏ Edit Draft").strong().size(12.0).color(Color32::WHITE))
                    .fill(AppTheme::ACCENT_PRIMARY)
                    .rounding(Rounding::same(6.0));
                if ui.add(edit_btn).clicked() {
                    *on_edit_draft = Some(latest_detail.clone());
                }
            }

            if ui.button(RichText::new("↩ Reply").size(12.0)).on_hover_text("Reply (HTML format by default)").clicked() {
                *on_reply = Some(latest_detail.clone());
            }
            if ui.button(RichText::new("📝 Text Reply").size(12.0)).on_hover_text("Reply in plain text format only").clicked() {
                *on_reply_plain = Some(latest_detail.clone());
            }
            if ui.button(RichText::new("👥 Reply All").size(12.0)).on_hover_text("Reply to all recipients").clicked() {
                *on_reply_all = Some(latest_detail.clone());
            }
            if ui.button(RichText::new("➡ Forward").size(12.0)).on_hover_text("Forward message").clicked() {
                *on_forward = Some(latest_detail.clone());
            }

            let read_label = if msg.is_read { "✉ Unread" } else { "✉ Read" };
            if ui.button(RichText::new(read_label).size(12.0)).clicked() {
                *on_toggle_read = Some((msg.id.clone(), !msg.is_read));
            }

            // Snooze Dropdown
            let snooze_title = if msg.is_snoozed() { "💤 Snoozed ▾" } else { "💤 Snooze ▾" };
            egui::ComboBox::from_id_salt(format!("snooze_combo_{}", msg.id))
                .selected_text(RichText::new(snooze_title).size(12.0))
                .show_ui(ui, |ui| {
                    let now = chrono::Utc::now();
                    if msg.is_snoozed() {
                        if ui.button(RichText::new("⏰ Unsnooze (Move to Inbox)").size(12.0).color(AppTheme::ACCENT_PRIMARY)).clicked() {
                            *on_snooze = Some((msg.id.clone(), None));
                            *status_toast = Some("Message unsnoozed and returned to Inbox".to_string());
                        }
                        ui.separator();
                    }

                    // Later Today (in 3 hours)
                    let in_3h = now.timestamp() + 3 * 3600;
                    if ui.button(RichText::new("⏰ Later Today (in 3 hours)").size(12.0)).clicked() {
                        *on_snooze = Some((msg.id.clone(), Some(in_3h)));
                        *status_toast = Some("Snoozed for 3 hours".to_string());
                    }

                    // Tomorrow Morning (9:00 AM)
                    let tomorrow_morning = (now.date_naive() + chrono::Days::new(1))
                        .and_hms_opt(9, 0, 0)
                        .unwrap()
                        .and_utc()
                        .timestamp();
                    if ui.button(RichText::new("🌅 Tomorrow Morning (9:00 AM)").size(12.0)).clicked() {
                        *on_snooze = Some((msg.id.clone(), Some(tomorrow_morning)));
                        *status_toast = Some("Snoozed until tomorrow 9:00 AM".to_string());
                    }

                    // Tomorrow Evening (6:00 PM)
                    let tomorrow_evening = (now.date_naive() + chrono::Days::new(1))
                        .and_hms_opt(18, 0, 0)
                        .unwrap()
                        .and_utc()
                        .timestamp();
                    if ui.button(RichText::new("🌆 Tomorrow Evening (6:00 PM)").size(12.0)).clicked() {
                        *on_snooze = Some((msg.id.clone(), Some(tomorrow_evening)));
                        *status_toast = Some("Snoozed until tomorrow 6:00 PM".to_string());
                    }

                    // Next Week (Next Monday 9:00 AM)
                    let days_until_mon = (8 - now.weekday().num_days_from_monday()) % 7;
                    let days_to_add = if days_until_mon == 0 { 7 } else { days_until_mon };
                    let next_monday = (now.date_naive() + chrono::Days::new(days_to_add as u64))
                        .and_hms_opt(9, 0, 0)
                        .unwrap()
                        .and_utc()
                        .timestamp();
                    if ui.button(RichText::new("📅 Next Week (Monday 9:00 AM)").size(12.0)).clicked() {
                        *on_snooze = Some((msg.id.clone(), Some(next_monday)));
                        *status_toast = Some("Snoozed until next Monday 9:00 AM".to_string());
                    }
                });

            // Move to Folder dropdown
            egui::ComboBox::from_id_salt(format!("move_combo_{}", msg.id))
                .selected_text(RichText::new("📁 Move").size(12.0))
                .show_ui(ui, |ui| {
                    for f in folders {
                        if f.id != msg.folder_id {
                            if ui.selectable_label(false, &f.display_name).clicked() {
                                *on_move_folder = Some((msg.id.clone(), f.id.clone()));
                            }
                        }
                    }
                });

            // Export Dropdown
            egui::ComboBox::from_id_salt(format!("export_combo_{}", msg.id))
                .selected_text(RichText::new("📤 Export").size(12.0))
                .show_ui(ui, |ui| {
                    let safe_fn = sanitize_filename_for_export(&msg.subject);

                    if ui.button(RichText::new("📄 Markdown (.md)").size(12.0)).clicked() {
                        let content = export_message_as_markdown(latest_detail);
                        let fname = format!("{}.md", safe_fn);
                        std::thread::spawn(move || {
                            let dialog = rfd::FileDialog::new()
                                .set_file_name(&fname)
                                .add_filter("Markdown Document", &["md"])
                                .set_title("Export Email as Markdown");
                            if let Some(path) = dialog.save_file() {
                                let _ = std::fs::write(path, content);
                            }
                        });
                        *status_toast = Some("Exporting email as Markdown...".to_string());
                    }

                    if ui.button(RichText::new("🌐 HTML Document (.html)").size(12.0)).clicked() {
                        let content = export_message_as_html(latest_detail);
                        let fname = format!("{}.html", safe_fn);
                        std::thread::spawn(move || {
                            let dialog = rfd::FileDialog::new()
                                .set_file_name(&fname)
                                .add_filter("HTML Document", &["html"])
                                .set_title("Export Email as HTML");
                            if let Some(path) = dialog.save_file() {
                                let _ = std::fs::write(path, content);
                            }
                        });
                        *status_toast = Some("Exporting email as HTML...".to_string());
                    }

                    if ui.button(RichText::new("✉ Raw EML (.eml)").size(12.0)).clicked() {
                        let content = export_message_as_eml(latest_detail);
                        let fname = format!("{}.eml", safe_fn);
                        std::thread::spawn(move || {
                            let dialog = rfd::FileDialog::new()
                                .set_file_name(&fname)
                                .add_filter("Email File", &["eml"])
                                .set_title("Export Email as EML");
                            if let Some(path) = dialog.save_file() {
                                let _ = std::fs::write(path, content);
                            }
                        });
                        *status_toast = Some("Exporting email as EML...".to_string());
                    }
                });

            if ui.button(RichText::new("🌐 In-App Web View").size(12.0))
                .on_hover_text("Open in dedicated native WebKit webview reader window (100% pixel-perfect HTML)")
                .clicked()
            {
                let subject_title = if msg.subject.trim().is_empty() {
                    "Email Preview".to_string()
                } else {
                    msg.subject.clone()
                };

                crate::webview::open_webview_window(subject_title, latest_detail);
            }

            if ui.button(RichText::new("↗ Browser").size(12.0))
                .on_hover_text("Open in default system browser")
                .clicked()
            {
                let temp_dir = std::env::temp_dir();
                let preview_file = temp_dir.join(format!("email_preview_{}.html", msg.id));
                let doc = crate::webview::prepare_email_html(latest_detail);

                if std::fs::write(&preview_file, doc).is_ok() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(format!("file://{}", preview_file.display())));
                }
            }

            if ui
                .button(RichText::new("🗑 Delete").size(12.0).color(AppTheme::ACCENT_DANGER))
                .clicked()
            {
                *on_delete = Some(msg.id.clone());
            }
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(6.0);

        let avail_width = ui.available_width();
        let avail_height = ui.available_height();
        let wrap_width = avail_width.max(300.0);

        ScrollArea::both()
            .auto_shrink([false, false])
            .max_width(avail_width)
            .max_height(avail_height)
            .hscroll(true)
            .vscroll(true)
            .show(ui, |ui| {
            if let Some(snooze_ts) = msg.snooze_until {
                if snooze_ts > chrono::Utc::now().timestamp() {
                    let dt = chrono::DateTime::from_timestamp(snooze_ts, 0).unwrap_or_default();
                    let formatted = dt.format("%a, %b %e at %I:%M %p").to_string();
                    egui::Frame::none()
                        .fill(AppTheme::BG_CARD)
                        .stroke(Stroke::new(1.0_f32, AppTheme::ACCENT_WARNING))
                        .rounding(Rounding::same(6.0))
                        .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(format!("💤 Snoozed until {}", formatted)).size(12.5).strong().color(AppTheme::ACCENT_WARNING));
                                if ui.button(RichText::new("Unsnooze").size(11.5)).clicked() {
                                    *on_snooze = Some((msg.id.clone(), None));
                                    *status_toast = Some("Message unsnoozed and returned to Inbox".to_string());
                                }
                            });
                        });
                    ui.add_space(8.0);
                }
            }

            // 2. Email Subject Title & Conversation badge
            let subj = if msg.subject.is_empty() {
                "(No Subject)"
            } else {
                &msg.subject
            };
            ui.horizontal(|ui| {
                ui.heading(
                    RichText::new(subj)
                        .size(20.0)
                        .strong()
                        .color(AppTheme::TEXT_PRIMARY),
                );
                if thread_messages.len() > 1 {
                    ui.label(
                        RichText::new(format!("💬 {} messages in thread", thread_messages.len()))
                            .size(12.0)
                            .color(AppTheme::ACCENT_PRIMARY)
                            .strong(),
                    );
                }
            });
            ui.add_space(12.0);

            // 3. Conversation Thread Messages
            for (idx, item) in thread_messages.iter().enumerate() {
                let is_last = idx == thread_messages.len() - 1;
                let item_msg = &item.header;
                let collapse_id = ui.make_persistent_id(format!("thread_exp_{}", item_msg.id));
                let mut is_expanded = ui.data_mut(|d| *d.get_temp_mut_or_insert_with(collapse_id, || is_last));

                egui::Frame::none()
                    .fill(if is_expanded { AppTheme::BG_CARD } else { AppTheme::BG_VIEW })
                    .stroke(Stroke::new(1.0_f32, if is_expanded { AppTheme::ACCENT_PRIMARY } else { AppTheme::BORDER_SUBTLE }))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(egui::Margin::symmetric(14.0, 10.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // Avatar Circle
                            let avatar_size = 32.0;
                            let (avatar_rect, _) = ui.allocate_exact_size(Vec2::new(avatar_size, avatar_size), Sense::hover());
                            let avatar_bg = AppTheme::avatar_color(item_msg.sender_display());
                            ui.painter().circle_filled(avatar_rect.center(), avatar_size / 2.0, avatar_bg);

                            let initials = AppTheme::get_initials(item_msg.sender_display());
                            ui.painter().text(
                                avatar_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                initials,
                                FontId::proportional(12.0),
                                Color32::WHITE,
                            );

                            ui.add_space(8.0);

                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(item_msg.sender_display())
                                            .strong()
                                            .size(13.0)
                                            .color(AppTheme::TEXT_PRIMARY),
                                    );
                                    if !item_msg.from_address.trim().is_empty() {
                                        ui.label(
                                            RichText::new(format!("<{}>", item_msg.from_address))
                                                .size(11.5)
                                                .color(AppTheme::TEXT_MUTED),
                                        );
                                    }
                                });

                                if !is_expanded {
                                    ui.label(RichText::new(&item_msg.snippet).size(12.0).color(AppTheme::TEXT_MUTED));
                                } else {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("to").size(11.0).color(AppTheme::TEXT_MUTED));
                                        let to_str = if item_msg.to_recipients.is_empty() {
                                            "me".to_string()
                                        } else {
                                            item_msg.to_recipients.iter().map(|r| r.display()).collect::<Vec<_>>().join(", ")
                                        };
                                        ui.label(RichText::new(to_str).size(11.0).color(AppTheme::TEXT_SECONDARY));
                                    });
                                }
                            });

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if thread_messages.len() > 1 {
                                    let btn_text = if is_expanded { "▲ Collapse" } else { "▼ Expand" };
                                    if ui.small_button(btn_text).clicked() {
                                        is_expanded = !is_expanded;
                                        ui.data_mut(|d| *d.get_temp_mut_or_insert_with(collapse_id, || is_last) = is_expanded);
                                    }
                                }
                                let full_date = item_msg.formatted_full_date();
                                if !full_date.is_empty() {
                                    ui.label(
                                        RichText::new(full_date)
                                            .size(11.5)
                                            .color(AppTheme::TEXT_SECONDARY),
                                    );
                                }
                            });
                        });

                        if is_expanded {
                            ui.add_space(10.0);
                            ui.separator();
                            ui.add_space(8.0);

                            // Body download check if needed
                            let has_body = item.body_html.as_ref().map(|b| !b.trim().is_empty()).unwrap_or(false)
                                || item.body_plain.as_ref().map(|b| !b.trim().is_empty()).unwrap_or(false);

                            if !has_body && !item_msg.body_fetched {
                                ui.vertical_centered(|ui| {
                                    ui.add_space(20.0);
                                    ui.spinner();
                                    ui.add_space(6.0);
                                    ui.label(RichText::new("Downloading message...").size(12.0).color(AppTheme::TEXT_MUTED));
                                });
                                let _ = cmd_tx.send(SyncCommand::FetchBody {
                                    account_id: item_msg.account_id.clone(),
                                    folder_id: item_msg.folder_id.clone(),
                                    uid: item_msg.uid,
                                    message_id: item_msg.id.clone(),
                                });
                            } else {
                                Self::render_body_and_attachments(ui, item, allowed_remote_images, wrap_width, status_toast);
                            }
                        }
                    });

                if !is_last {
                    ui.add_space(4.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("│").size(12.0).color(AppTheme::BORDER_SUBTLE));
                    });
                    ui.add_space(4.0);
                }
            }

            ui.add_space(30.0);
        });
    }

    pub fn render_body_and_attachments(
        ui: &mut Ui,
        detail: &MessageDetail,
        allowed_remote_images: &mut std::collections::HashSet<String>,
        wrap_width: f32,
        status_toast: &mut Option<String>,
    ) {
        Self::render_message_body_content(ui, detail, allowed_remote_images, wrap_width, status_toast);
        Self::render_message_attachments(ui, detail, status_toast);
    }

    pub fn render_message_body_content(
        ui: &mut Ui,
        detail: &MessageDetail,
        allowed_remote_images: &mut std::collections::HashSet<String>,
        wrap_width: f32,
        status_toast: &mut Option<String>,
    ) {
        let msg = &detail.header;
        let is_encrypted = detail.body_plain.as_deref().map(email_core::pgp::is_pgp_encrypted).unwrap_or(false);
        let is_signed = detail.body_plain.as_deref().map(email_core::pgp::is_pgp_signed).unwrap_or(false);

        if is_encrypted {
            egui::Frame::none()
                .fill(Color32::from_rgb(18, 30, 49))
                .stroke(Stroke::new(1.0_f32, AppTheme::ACCENT_PRIMARY))
                .rounding(Rounding::same(8.0))
                .inner_margin(egui::Margin::symmetric(14.0, 10.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🔒").size(16.0));
                        ui.vertical(|ui| {
                            ui.label(RichText::new("End-to-End Encrypted Message (PGP / OpenPGP)").strong().size(13.0).color(Color32::WHITE));
                            ui.label(RichText::new("This message was encrypted with a public key. Decrypted automatically for your account.").size(11.5).color(AppTheme::TEXT_SECONDARY));
                        });
                    });
                });
            ui.add_space(8.0);
        } else if is_signed {
            egui::Frame::none()
                .fill(Color32::from_rgb(20, 38, 28))
                .stroke(Stroke::new(1.0_f32, AppTheme::ACCENT_SUCCESS))
                .rounding(Rounding::same(8.0))
                .inner_margin(egui::Margin::symmetric(14.0, 10.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("✍").size(16.0));
                        ui.vertical(|ui| {
                            ui.label(RichText::new("Cryptographically Signed Message (PGP)").strong().size(13.0).color(AppTheme::ACCENT_SUCCESS));
                            ui.label(RichText::new("Sender identity and content integrity verified with RSA-SHA256 signature.").size(11.5).color(AppTheme::TEXT_SECONDARY));
                        });
                    });
                });
            ui.add_space(8.0);
        }

        ui.scope(|ui| {
            if let Some(ref html) = detail.body_html {
                let blocks = parse_html_to_blocks(html);
                if blocks.is_empty() {
                    if let Some(ref plain) = detail.body_plain {
                        ui.scope(|ui| {
                            ui.set_max_width(wrap_width);
                            ui.label(RichText::new(plain).size(13.5).line_height(Some(20.0)));
                        });
                    }
                } else {
                    let has_remote = email_html::has_remote_images(&blocks);
                    let is_remote_allowed = allowed_remote_images.contains(&msg.id);

                    // Privacy Shield Banner
                    if has_remote && !is_remote_allowed {
                        ui.add_space(4.0);
                        let banner_width = wrap_width.min(600.0).max(320.0);
                        ui.vertical_centered(|ui| {
                            let (shield_rect, _) = ui.allocate_exact_size(Vec2::new(banner_width, 36.0), Sense::hover());
                            ui.painter().rect_filled(shield_rect, Rounding::same(6.0), Color32::from_rgb(26, 32, 44));
                            ui.painter().rect_stroke(shield_rect, Rounding::same(6.0), Stroke::new(1.0_f32, AppTheme::ACCENT_PRIMARY));
                            let mut shield_ui = ui.new_child(egui::UiBuilder::new().max_rect(shield_rect));
                            shield_ui.horizontal_centered(|ui| {
                                ui.add_space(10.0);
                                ui.label(RichText::new("🛡️").size(14.0));
                                ui.label(RichText::new("Remote images blocked for privacy.").size(12.0).color(Color32::WHITE));
                                if ui.button(RichText::new("🖼 Load Images").size(11.0).strong()).clicked() {
                                    allowed_remote_images.insert(msg.id.clone());
                                }
                            });
                        });
                        ui.add_space(8.0);
                    }

                    // Centered Canvas Container (like Gmail)
                    ui.vertical_centered(|ui| {
                        egui::Frame::none()
                            .fill(Color32::from_rgb(253, 245, 234)) // Warm Cream Card Canvas (#fdf5ea)
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(230, 220, 205)))
                            .rounding(Rounding::same(8.0))
                            .inner_margin(egui::Margin::symmetric(24.0, 24.0))
                            .show(ui, |ui| {
                                let canvas_width = wrap_width.min(600.0).max(320.0);
                                ui.set_max_width(canvas_width);

                                for block in blocks {
                                    match block {
                                        HtmlBlock::Paragraph { spans, is_center } => {
                                            if is_center {
                                                ui.vertical_centered(|ui| {
                                                    render_spans(ui, &spans, canvas_width, true);
                                                });
                                            } else {
                                                render_spans(ui, &spans, canvas_width, true);
                                            }
                                            ui.add_space(10.0);
                                        }
                                        HtmlBlock::Heading { level, text, is_center, color } => {
                                            ui.add_space(8.0);
                                            let mut rt = RichText::new(text).strong();
                                            if level == 1 {
                                                rt = rt.size(24.0);
                                            } else {
                                                rt = rt.size(18.0);
                                            }

                                            if let Some((r, g, b)) = color {
                                                rt = rt.color(Color32::from_rgb(r, g, b));
                                            } else {
                                                rt = rt.color(Color32::from_rgb(33, 37, 41));
                                            }

                                            if is_center {
                                                ui.vertical_centered(|ui| {
                                                    ui.heading(rt);
                                                });
                                            } else {
                                                ui.scope(|ui| {
                                                    ui.set_max_width(canvas_width);
                                                    ui.heading(rt);
                                                });
                                            }
                                            ui.add_space(8.0);
                                        }
                                        HtmlBlock::Button { text, url, bg_color, text_color, is_center } => {
                                            ui.add_space(10.0);
                                            let bg = Color32::from_rgb(bg_color.0, bg_color.1, bg_color.2);
                                            let fg = Color32::from_rgb(text_color.0, text_color.1, text_color.2);

                                            let is_suspicious = email_html::is_suspicious_link(&text, &url);

                                            let btn_ui = |ui: &mut egui::Ui| {
                                                let btn_label = if is_suspicious {
                                                    format!("⚠️ {}", text)
                                                } else {
                                                    text.clone()
                                                };
                                                let btn = egui::Button::new(
                                                    RichText::new(&btn_label)
                                                        .size(14.5)
                                                        .strong()
                                                        .color(fg),
                                                )
                                                .fill(bg)
                                                .rounding(Rounding::same(6.0))
                                                .min_size(Vec2::new(160.0, 42.0));

                                                let response = ui.add(btn);
                                                if is_suspicious {
                                                    response.clone().on_hover_text(format!("⚠️ Suspicious Link Detected!\nText claims: '{}'\nActual destination: '{}'", text, url));
                                                } else {
                                                    response.clone().on_hover_text(&url);
                                                }

                                                if response.clicked() {
                                                    ui.ctx().open_url(egui::OpenUrl::new_tab(&url));
                                                }
                                            };

                                            if is_center {
                                                ui.vertical_centered(|ui| {
                                                    btn_ui(ui);
                                                });
                                            } else {
                                                btn_ui(ui);
                                            }
                                            ui.add_space(12.0);
                                        }
                                        HtmlBlock::ListItem(spans) => {
                                            ui.horizontal(|ui| {
                                                ui.label(RichText::new("•").size(14.0).color(Color32::from_rgb(234, 88, 12)));
                                                render_spans(ui, &spans, canvas_width - 20.0, true);
                                            });
                                            ui.add_space(4.0);
                                        }
                                        HtmlBlock::Blockquote(text) => {
                                            ui.horizontal(|ui| {
                                                let (bar_rect, _) = ui.allocate_exact_size(Vec2::new(3.0, 24.0), Sense::hover());
                                                ui.painter().rect_filled(bar_rect, Rounding::same(1.5), Color32::from_rgb(234, 88, 12));
                                                ui.add_space(8.0);
                                                ui.label(
                                                    RichText::new(text)
                                                        .italics()
                                                        .size(13.5)
                                                        .color(Color32::from_rgb(108, 117, 125)),
                                                );
                                            });
                                            ui.add_space(6.0);
                                        }
                                        HtmlBlock::Image { src, alt, is_center } => {
                                            ui.add_space(6.0);
                                            let is_external = src.starts_with("http://") || src.starts_with("https://");

                                            if is_external && !is_remote_allowed {
                                                // Remote image blocked placeholder
                                                let mut block_ui = |ui: &mut egui::Ui| {
                                                    egui::Frame::none()
                                                        .fill(Color32::from_rgb(245, 240, 230))
                                                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(220, 210, 195)))
                                                        .rounding(Rounding::same(6.0))
                                                        .inner_margin(egui::Margin::symmetric(14.0, 10.0))
                                                        .show(ui, |ui| {
                                                            ui.horizontal(|ui| {
                                                                ui.label(RichText::new("🛡️ Remote image blocked").size(12.0).color(Color32::from_rgb(120, 110, 100)));
                                                                if ui.small_button("Show").clicked() {
                                                                    allowed_remote_images.insert(msg.id.clone());
                                                                }
                                                            });
                                                        });
                                                };
                                                if is_center {
                                                    ui.vertical_centered(|ui| block_ui(ui));
                                                } else {
                                                    block_ui(ui);
                                                }
                                            } else {
                                                let resolved_uri = if src.starts_with("cid:") {
                                                    let cid_key = src.trim_start_matches("cid:").trim_matches(|c| c == '<' || c == '>');
                                                    if let Some(matching_att) = detail.attachments.iter().find(|a| {
                                                        a.content_id.as_deref() == Some(cid_key) || a.filename == cid_key
                                                    }) {
                                                        if let Some(ref path) = matching_att.local_cache_path {
                                                            format!("file://{}", path)
                                                        } else {
                                                            src.clone()
                                                        }
                                                    } else {
                                                        src.clone()
                                                    }
                                                } else {
                                                    src.clone()
                                                };

                                                let mut img_render = |ui: &mut egui::Ui| {
                                                    let img_widget = egui::Image::new(&resolved_uri)
                                                        .max_width(canvas_width)
                                                        .rounding(Rounding::same(6.0))
                                                        .sense(Sense::click());

                                                    let mut response = ui.add(img_widget);
                                                    if let Some(ref alt_text) = alt {
                                                        response = response.on_hover_text(alt_text);
                                                    }

                                                    // Right-click to Save Image As...
                                                    let save_uri = resolved_uri.clone();
                                                    let default_name = alt.clone().unwrap_or_else(|| "email_image.png".to_string());
                                                    response.context_menu(|ui| {
                                                        ui.label(RichText::new("Image Options").strong().size(11.0));
                                                        ui.separator();
                                                        if ui.button("💾 Save Image As...").clicked() {
                                                            std::thread::spawn(move || {
                                                                let dialog = rfd::FileDialog::new()
                                                                    .set_file_name(&default_name)
                                                                    .set_title("Save Image As...");

                                                                if let Some(dest_path) = dialog.save_file() {
                                                                    if save_uri.starts_with("file://") {
                                                                        let local_path = save_uri.trim_start_matches("file://");
                                                                        let _ = std::fs::copy(local_path, &dest_path);
                                                                    } else if save_uri.starts_with("data:") {
                                                                        if let Some(comma_pos) = save_uri.find(',') {
                                                                            let b64_data = &save_uri[comma_pos + 1..];
                                                                            if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(b64_data.trim()) {
                                                                                let _ = std::fs::write(&dest_path, decoded);
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            });
                                                            *status_toast = Some("Opening file picker...".to_string());
                                                            ui.close_menu();
                                                        }
                                                    });
                                                };

                                                if is_center {
                                                    ui.vertical_centered(|ui| {
                                                        img_render(ui);
                                                    });
                                                } else {
                                                    img_render(ui);
                                                }
                                            }

                                            ui.add_space(8.0);
                                        }
                                        HtmlBlock::CodeBlock(code) => {
                                            egui::Frame::none()
                                                .fill(Color32::from_rgb(240, 235, 225))
                                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(220, 210, 195)))
                                                .rounding(Rounding::same(6.0))
                                                .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                                                .show(ui, |ui| {
                                                    ui.monospace(RichText::new(code).size(12.0).color(Color32::from_rgb(33, 37, 41)));
                                                });
                                            ui.add_space(6.0);
                                        }
                                        HtmlBlock::HorizontalRule => {
                                            ui.add_space(6.0);
                                            ui.separator();
                                            ui.add_space(6.0);
                                        }
                                    }
                                }
                            });
                    });
                }
            } else if let Some(ref plain) = detail.body_plain {
                ui.label(RichText::new(plain).size(13.5).line_height(Some(20.0)));
            } else {
                ui.label(RichText::new("(Empty email body)").italics().color(AppTheme::TEXT_MUTED));
            }
        });
    }

    pub fn render_message_attachments(
        ui: &mut Ui,
        detail: &MessageDetail,
        status_toast: &mut Option<String>,
    ) {
        if detail.attachments.is_empty() {
            return;
        }

        ui.add_space(20.0);
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            Stroke::new(1.0_f32, AppTheme::BORDER_SUBTLE),
        );
        ui.add_space(14.0);

        ui.label(
            RichText::new(format!("📎 ATTACHMENTS ({}) — Click to Save", detail.attachments.len()))
                .size(11.5)
                .strong()
                .color(AppTheme::TEXT_MUTED),
        );
        ui.add_space(12.0);

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(10.0, 10.0);

            for att in &detail.attachments {
                let size_kb = att.size_bytes / 1024;
                let size_text = if size_kb > 1024 {
                    format!("{:.1} MB", size_kb as f64 / 1024.0)
                } else {
                    format!("{} KB", size_kb)
                };

                let card_width = if ui.available_width() < 480.0 {
                    (ui.available_width() - 8.0).max(180.0)
                } else {
                    230.0
                };

                let (rect, resp) = ui.allocate_exact_size(
                    Vec2::new(card_width, 56.0),
                    Sense::click(),
                );

                if resp.clicked() {
                    if let Some(ref cache_path) = att.local_cache_path {
                        let src_path = std::path::PathBuf::from(cache_path);
                        let filename = att.filename.clone();
                        if src_path.exists() {
                            std::thread::spawn(move || {
                                let dialog = rfd::FileDialog::new()
                                    .set_file_name(&filename)
                                    .set_title("Save Attachment As...");

                                if let Some(dest_path) = dialog.save_file() {
                                    let _ = std::fs::copy(&src_path, &dest_path);
                                }
                            });
                            *status_toast = Some(format!("Saving attachment: {}", att.filename));
                        }
                    }
                }

                let bg = if resp.hovered() {
                    AppTheme::BG_HOVER
                } else {
                    AppTheme::BG_CARD
                };

                let border_color = if resp.hovered() {
                    AppTheme::ACCENT_PRIMARY
                } else {
                    AppTheme::BORDER_SUBTLE
                };

                ui.painter().rect_filled(rect, Rounding::same(8.0), bg);
                ui.painter().rect_stroke(rect, Rounding::same(8.0), Stroke::new(1.0_f32, border_color));

                let inner_rect = rect.shrink2(Vec2::new(12.0, 8.0));
                let mut item_ui = ui.new_child(egui::UiBuilder::new().max_rect(inner_rect));
                item_ui.horizontal_centered(|ui| {
                    ui.label(RichText::new("📄").size(20.0));
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        let truncated_fn = if att.filename.len() > 22 {
                            format!("{}...", &att.filename[..19])
                        } else {
                            att.filename.clone()
                        };
                        ui.label(RichText::new(truncated_fn).size(12.0).strong().color(AppTheme::TEXT_PRIMARY));
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&size_text).size(10.5).color(AppTheme::TEXT_MUTED));
                            ui.label(RichText::new("• 💾 Save").size(10.5).color(AppTheme::ACCENT_PRIMARY));
                        });
                    });
                });

                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                resp.on_hover_text(format!("Click to choose where to save '{}'", att.filename));
            }
        });
    }
}


fn render_spans(ui: &mut Ui, spans: &[FormattedSpan], wrap_width: f32, is_light_canvas: bool) {
    ui.scope(|ui| {
        ui.set_max_width(wrap_width);
        ui.horizontal_wrapped(|ui| {
            for span in spans {
                let mut text = RichText::new(&span.text).size(14.5).line_height(Some(22.0));
                let default_color = if is_light_canvas {
                    Color32::from_rgb(33, 37, 41)
                } else {
                    AppTheme::TEXT_PRIMARY
                };
                let default_secondary = if is_light_canvas {
                    Color32::from_rgb(108, 117, 125)
                } else {
                    AppTheme::TEXT_SECONDARY
                };

                let col = if let Some((r, g, b)) = span.text_color {
                    Color32::from_rgb(r, g, b)
                } else {
                    default_color
                };

                match span.style {
                    TextStyle::Normal => {
                        text = text.color(col);
                    }
                    TextStyle::Bold => {
                        text = text.strong().color(col);
                    }
                    TextStyle::Italic => {
                        text = text.italics().color(if span.text_color.is_some() { col } else { default_secondary });
                    }
                    TextStyle::BoldItalic => {
                        text = text.strong().italics().color(col);
                    }
                    TextStyle::Code => {
                        text = text.monospace().background_color(Color32::from_rgb(235, 230, 220)).color(Color32::from_rgb(180, 80, 0));
                    }
                    TextStyle::Heading1 | TextStyle::Heading2 | TextStyle::Heading3 => {
                        text = text.strong().color(col);
                    }
                }

                if let Some(ref url) = span.link_url {
                    let is_suspicious = email_html::is_suspicious_link(&span.text, url);
                    if is_suspicious {
                        let warn_text = RichText::new(format!("⚠️ {} [Deceptive Link!]", span.text))
                            .size(13.5)
                            .color(Color32::from_rgb(220, 38, 38))
                            .strong()
                            .underline();
                        let link_resp = ui.hyperlink_to(warn_text, url)
                            .on_hover_text(format!("⚠️ Suspicious link! Display text is '{}' but target destination is '{}'", span.text, url));
                        if link_resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    } else {
                        let link_col = if is_light_canvas {
                            Color32::from_rgb(26, 115, 232)
                        } else {
                            AppTheme::ACCENT_HOVER
                        };
                        let link_resp = ui.hyperlink_to(text.color(link_col).underline(), url)
                            .on_hover_text(url);
                        if link_resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    }
                } else {
                    ui.add(egui::Label::new(text).selectable(true));
                }
            }
        });
    });
}

pub fn export_message_as_markdown(detail: &MessageDetail) -> String {
    let msg = &detail.header;
    let format_recipient = |r: &email_core::Recipient| {
        if let Some(ref n) = r.name {
            format!("{} <{}>", n, r.email)
        } else {
            r.email.clone()
        }
    };
    let to_str = msg.to_recipients.iter().map(format_recipient).collect::<Vec<_>>().join(", ");
    let cc_str = msg.cc_recipients.iter().map(format_recipient).collect::<Vec<_>>().join(", ");
    let body_text = detail.body_plain.clone().unwrap_or_else(|| {
        detail.body_html.as_deref().map(email_html::html_to_plain_text).unwrap_or_default()
    });

    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("Subject: {}\n", msg.subject));
    out.push_str(&format!("From: {}\n", msg.from_address));
    if !to_str.is_empty() {
        out.push_str(&format!("To: {}\n", to_str));
    }
    if !cc_str.is_empty() {
        out.push_str(&format!("Cc: {}\n", cc_str));
    }
    out.push_str(&format!("Date: {}\n", msg.formatted_date()));
    out.push_str("---\n\n");
    out.push_str(&body_text);
    out
}

pub fn export_message_as_html(detail: &MessageDetail) -> String {
    crate::webview::prepare_email_html(detail)
}

pub fn export_message_as_eml(detail: &MessageDetail) -> String {
    let msg = &detail.header;
    let format_recipient = |r: &email_core::Recipient| {
        if let Some(ref n) = r.name {
            format!("{} <{}>", n, r.email)
        } else {
            r.email.clone()
        }
    };
    let to_str = msg.to_recipients.iter().map(format_recipient).collect::<Vec<_>>().join(", ");
    let cc_str = msg.cc_recipients.iter().map(format_recipient).collect::<Vec<_>>().join(", ");

    let mut eml = String::new();
    eml.push_str(&format!("From: {}\r\n", msg.from_address));
    if !to_str.is_empty() {
        eml.push_str(&format!("To: {}\r\n", to_str));
    }
    if !cc_str.is_empty() {
        eml.push_str(&format!("Cc: {}\r\n", cc_str));
    }
    eml.push_str(&format!("Subject: {}\r\n", msg.subject));
    if let Some(ref mid) = msg.message_id {
        eml.push_str(&format!("Message-ID: <{}>\r\n", mid));
    }
    eml.push_str("MIME-Version: 1.0\r\n");

    if let Some(ref html) = detail.body_html {
        eml.push_str("Content-Type: text/html; charset=UTF-8\r\n\r\n");
        eml.push_str(html);
    } else if let Some(ref plain) = detail.body_plain {
        eml.push_str("Content-Type: text/plain; charset=UTF-8\r\n\r\n");
        eml.push_str(plain);
    } else {
        eml.push_str("Content-Type: text/plain; charset=UTF-8\r\n\r\n");
    }

    eml
}

pub fn sanitize_filename_for_export(subject: &str) -> String {
    let clean: String = subject
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let trimmed = clean.trim_matches('_');
    if trimmed.is_empty() {
        "email_export".to_string()
    } else if trimmed.len() > 50 {
        trimmed[..50].to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use email_core::models::{MessageHeader, Recipient};

    fn sample_detail() -> MessageDetail {
        MessageDetail {
            header: MessageHeader {
                id: "msg-99".to_string(),
                account_id: "acc-1".to_string(),
                folder_id: "inbox".to_string(),
                uid: 42,
                message_id: Some("unique-id-99@domain.com".to_string()),
                in_reply_to: None,
                subject: "Quarterly Financial Report Q3".to_string(),
                from_name: Some("Finance Lead".to_string()),
                from_address: "finance@company.com".to_string(),
                to_recipients: vec![Recipient::new(Some("Kunal".to_string()), "kunal@abhashtech.com".to_string())],
                cc_recipients: vec![Recipient::new(None, "audit@company.com".to_string())],
                date_epoch: 1700000000,
                snippet: "Attached is the quarterly report".to_string(),
                is_read: true,
                is_flagged: true,
                is_draft: false,
                is_deleted: false,
                body_fetched: true,
                size_bytes: 4096,
                snooze_until: None,
            },
            body_plain: Some("Hello team,\n\nHere is the financial summary for Q3.\nRevenue grew by 24%.\n\nBest,\nFinance".to_string()),
            body_html: Some("<p>Hello team,</p><p>Here is the financial summary for Q3.</p><p>Revenue grew by 24%.</p><p>Best,<br>Finance</p>".to_string()),
            attachments: Vec::new(),
        }
    }

    #[test]
    fn test_export_as_markdown() {
        let detail = sample_detail();
        let md = export_message_as_markdown(&detail);
        assert!(md.contains("Subject: Quarterly Financial Report Q3"));
        assert!(md.contains("From: finance@company.com"));
        assert!(md.contains("Revenue grew by 24%"));
    }

    #[test]
    fn test_export_as_eml() {
        let detail = sample_detail();
        let eml = export_message_as_eml(&detail);
        assert!(eml.contains("From: finance@company.com"));
        assert!(eml.contains("Subject: Quarterly Financial Report Q3"));
        assert!(eml.contains("Message-ID: <unique-id-99@domain.com>"));
        assert!(eml.contains("MIME-Version: 1.0"));
    }

    #[test]
    fn test_sanitize_filename_for_export() {
        assert_eq!(sanitize_filename_for_export("Hello / World: Test?"), "Hello___World__Test");
        assert_eq!(sanitize_filename_for_export(""), "email_export");
    }
}
