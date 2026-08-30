use crate::theme::AppTheme;
use egui::{Color32, FontId, RichText, Rounding, ScrollArea, Sense, Stroke, Ui, Vec2};
use email_core::events::SyncCommand;
use email_core::models::MessageDetail;
use email_html::{parse_html_to_blocks, FormattedSpan, HtmlBlock, TextStyle};
use tokio::sync::mpsc;

pub struct MessageViewPane;

impl MessageViewPane {
    pub fn show(
        ui: &mut Ui,
        detail_opt: Option<&MessageDetail>,
        cmd_tx: &mpsc::UnboundedSender<SyncCommand>,
        on_reply: &mut Option<MessageDetail>,
        on_reply_all: &mut Option<MessageDetail>,
        on_forward: &mut Option<MessageDetail>,
        on_delete: &mut Option<String>,
        on_mark_unread: &mut Option<String>,
    ) {
        let Some(detail) = detail_opt else {
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
        };

        let msg = &detail.header;

        // 1. Action Toolbar
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            if ui.button(RichText::new("↩ Reply").size(12.5)).clicked() {
                *on_reply = Some(detail.clone());
            }
            if ui.button(RichText::new("👥 Reply All").size(12.5)).clicked() {
                *on_reply_all = Some(detail.clone());
            }
            if ui.button(RichText::new("➡ Forward").size(12.5)).clicked() {
                *on_forward = Some(detail.clone());
            }

            ui.add_space(12.0);

            if ui.button(RichText::new("✉ Unread").size(12.5)).clicked() {
                *on_mark_unread = Some(msg.id.clone());
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(RichText::new("🗑 Delete").size(12.5).color(AppTheme::ACCENT_DANGER))
                    .clicked()
                {
                    *on_delete = Some(msg.id.clone());
                }
            });
        });

        ui.add_space(6.0);
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            Stroke::new(1.0_f32, AppTheme::BORDER_SUBTLE),
        );
        ui.add_space(12.0);

        ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
            // 2. Email Subject Title
            let subj = if msg.subject.is_empty() {
                "(No Subject)"
            } else {
                &msg.subject
            };
            ui.heading(
                RichText::new(subj)
                    .size(20.0)
                    .strong()
                    .color(AppTheme::TEXT_PRIMARY),
            );
            ui.add_space(12.0);

            // 3. Sender Header Card
            let header_card_rect = ui.available_rect_before_wrap();
            let (card_rect, _) = ui.allocate_exact_size(Vec2::new(header_card_rect.width(), 64.0), Sense::hover());
            ui.painter().rect_filled(card_rect, Rounding::same(8.0), AppTheme::BG_CARD);
            ui.painter().rect_stroke(card_rect, Rounding::same(8.0), Stroke::new(1.0_f32, AppTheme::BORDER_SUBTLE));


            let mut card_ui = ui.new_child(egui::UiBuilder::new().max_rect(card_rect));
            card_ui.horizontal(|ui| {
                ui.add_space(12.0);

                // Avatar Circle
                let avatar_size = 40.0;
                let (avatar_rect, _) = ui.allocate_exact_size(Vec2::new(avatar_size, avatar_size), Sense::hover());
                let avatar_bg = AppTheme::avatar_color(msg.sender_display());
                ui.painter().circle_filled(avatar_rect.center(), avatar_size / 2.0, avatar_bg);

                let initials = AppTheme::get_initials(msg.sender_display());
                ui.painter().text(
                    avatar_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    initials,
                    FontId::proportional(14.0),
                    Color32::WHITE,
                );

                ui.add_space(8.0);

                // Sender Info & Recipients
                ui.vertical(|ui| {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(msg.sender_display())
                                .strong()
                                .size(13.5)
                                .color(AppTheme::TEXT_PRIMARY),
                        );
                        if !msg.from_address.trim().is_empty() {
                            ui.label(
                                RichText::new(format!("<{}>", msg.from_address))
                                    .size(12.0)
                                    .color(AppTheme::TEXT_MUTED),
                            );
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(12.0);
                            let full_date = msg.formatted_full_date();
                            if !full_date.is_empty() {
                                ui.label(
                                    RichText::new(full_date)
                                        .size(11.5)
                                        .color(AppTheme::TEXT_SECONDARY),
                                );
                            }
                        });
                    });


                    // To / Cc line
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("to").size(11.5).color(AppTheme::TEXT_MUTED));
                        let to_str = if msg.to_recipients.is_empty() {
                            "me".to_string()
                        } else {
                            msg.to_recipients.iter().map(|r| r.display()).collect::<Vec<_>>().join(", ")
                        };
                        ui.label(RichText::new(to_str).size(11.5).color(AppTheme::TEXT_SECONDARY));
                    });
                });
            });

            ui.add_space(16.0);

            // 4. Body download check
            let has_body = detail.body_html.as_ref().map(|b| !b.trim().is_empty()).unwrap_or(false)
                || detail.body_plain.as_ref().map(|b| !b.trim().is_empty()).unwrap_or(false);

            if !has_body && !msg.body_fetched {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.spinner();
                    ui.add_space(8.0);
                    ui.label(RichText::new("Downloading complete email...").size(13.0).color(AppTheme::TEXT_MUTED));
                });

                let _ = cmd_tx.send(SyncCommand::FetchBody {
                    account_id: msg.account_id.clone(),
                    folder_id: msg.folder_id.clone(),
                    uid: msg.uid,
                    message_id: msg.id.clone(),
                });
                return;
            }


            // 5. Message Body Content
            let body_container_rect = ui.available_rect_before_wrap();
            let (_body_rect, _) = ui.allocate_exact_size(Vec2::new(body_container_rect.width(), 0.0), Sense::hover());

            
            ui.scope(|ui| {
                if let Some(ref html) = detail.body_html {
                    let blocks = parse_html_to_blocks(html);
                    if blocks.is_empty() {
                        if let Some(ref plain) = detail.body_plain {
                            ui.label(RichText::new(plain).size(13.5).line_height(Some(20.0)));
                        }
                    } else {
                        for block in blocks {
                            match block {
                                HtmlBlock::Paragraph(spans) => {
                                    render_spans(ui, &spans);
                                    ui.add_space(8.0);
                                }
                                HtmlBlock::Heading { level: _, text } => {
                                    ui.add_space(4.0);
                                    ui.heading(RichText::new(text).strong().color(AppTheme::TEXT_PRIMARY));
                                    ui.add_space(4.0);
                                }
                                HtmlBlock::ListItem(spans) => {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("•").size(14.0).color(AppTheme::ACCENT_PRIMARY));
                                        render_spans(ui, &spans);
                                    });
                                    ui.add_space(4.0);
                                }
                                HtmlBlock::Blockquote(text) => {
                                    ui.horizontal(|ui| {
                                        let (bar_rect, _) = ui.allocate_exact_size(Vec2::new(3.0, 24.0), Sense::hover());
                                        ui.painter().rect_filled(bar_rect, Rounding::same(1.5), AppTheme::ACCENT_PRIMARY);
                                        ui.add_space(8.0);
                                        ui.label(
                                            RichText::new(text)
                                                .italics()
                                                .size(13.0)
                                                .color(AppTheme::TEXT_SECONDARY),
                                        );
                                    });
                                    ui.add_space(6.0);
                                }
                                HtmlBlock::Image { src, alt } => {
                                    ui.add_space(6.0);
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

                                    let img_widget = egui::Image::new(&resolved_uri)
                                        .max_width(ui.available_width().min(680.0))
                                        .rounding(Rounding::same(6.0));

                                    let response = ui.add(img_widget);
                                    if let Some(ref alt_text) = alt {
                                        response.on_hover_text(alt_text);
                                    }
                                    ui.add_space(6.0);
                                }
                                HtmlBlock::CodeBlock(code) => {
                                    let (code_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::hover());
                                    ui.painter().rect_filled(code_rect, Rounding::same(6.0), AppTheme::BG_CARD);
                                    ui.monospace(RichText::new(code).size(12.0).color(AppTheme::TEXT_PRIMARY));
                                    ui.add_space(6.0);
                                }
                                HtmlBlock::HorizontalRule => {
                                    ui.add_space(4.0);
                                    ui.separator();
                                    ui.add_space(4.0);
                                }
                            }
                        }
                    }
                } else if let Some(ref plain) = detail.body_plain {
                    ui.label(RichText::new(plain).size(13.5).line_height(Some(20.0)));
                } else {
                    ui.label(RichText::new("(Empty email body)").italics().color(AppTheme::TEXT_MUTED));
                }
            });

            // 6. Attachments Card Section
            if !detail.attachments.is_empty() {
                ui.add_space(24.0);
                ui.painter().hline(
                    ui.available_rect_before_wrap().x_range(),
                    ui.cursor().top(),
                    Stroke::new(1.0_f32, AppTheme::BORDER_SUBTLE),
                );
                ui.add_space(12.0);

                ui.label(
                    RichText::new(format!("📎 ATTACHMENTS ({}) — Click to Save to Downloads", detail.attachments.len()))
                        .size(11.0)
                        .strong()
                        .color(AppTheme::TEXT_MUTED),
                );
                ui.add_space(8.0);

                ui.horizontal_wrapped(|ui| {
                    for att in &detail.attachments {
                        let size_kb = att.size_bytes / 1024;
                        let (rect, resp) = ui.allocate_exact_size(
                            Vec2::new(220.0, 40.0),
                            Sense::click(),
                        );

                        if resp.clicked() {
                            if let Some(ref cache_path) = att.local_cache_path {
                                let src_path = std::path::Path::new(cache_path);
                                if src_path.exists() {
                                    let download_dir = dirs_download();
                                    let dest_path = download_dir.join(&att.filename);
                                    let _ = std::fs::copy(src_path, &dest_path);
                                    let _ = std::process::Command::new("xdg-open")
                                        .arg(&dest_path)
                                        .spawn();
                                }
                            }
                        }

                        let bg = if resp.hovered() {
                            AppTheme::BG_HOVER
                        } else {
                            AppTheme::BG_CARD
                        };
                        ui.painter().rect_filled(rect, Rounding::same(6.0), bg);
                        ui.painter().rect_stroke(rect, Rounding::same(6.0), Stroke::new(1.0_f32, if resp.hovered() { AppTheme::ACCENT_PRIMARY } else { AppTheme::BORDER_SUBTLE }));

                        let mut pill_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
                        pill_ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            ui.label(RichText::new("📄").size(15.0));
                            ui.vertical(|ui| {
                                ui.add_space(4.0);
                                ui.label(RichText::new(&att.filename).size(11.5).strong().color(AppTheme::TEXT_PRIMARY));
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(format!("{} KB", size_kb)).size(10.0).color(AppTheme::TEXT_MUTED));
                                    ui.label(RichText::new("• ⬇ Download").size(10.0).color(AppTheme::ACCENT_PRIMARY));
                                });
                            });
                        });
                        resp.on_hover_text(format!("Click to download '{}' to ~/Downloads and open", att.filename));
                    }
                });
            }

            ui.add_space(30.0);
        });
    }
}

fn dirs_download() -> std::path::PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        let p = std::path::PathBuf::from(home).join("Downloads");
        let _ = std::fs::create_dir_all(&p);
        p
    } else {
        std::path::PathBuf::from(".")
    }
}


fn render_spans(ui: &mut Ui, spans: &[FormattedSpan]) {
    ui.horizontal_wrapped(|ui| {
        for span in spans {
            let mut text = RichText::new(&span.text).size(13.5);
            match span.style {
                TextStyle::Normal => {
                    text = text.color(AppTheme::TEXT_PRIMARY);
                }
                TextStyle::Bold => {
                    text = text.strong().color(AppTheme::TEXT_PRIMARY);
                }
                TextStyle::Italic => {
                    text = text.italics().color(AppTheme::TEXT_SECONDARY);
                }
                TextStyle::BoldItalic => {
                    text = text.strong().italics().color(AppTheme::TEXT_PRIMARY);
                }
                TextStyle::Code => {
                    text = text.monospace().background_color(AppTheme::BG_CARD).color(Color32::from_rgb(255, 180, 100));
                }
                TextStyle::Heading1 => {
                    text = text.size(18.0).strong().color(AppTheme::TEXT_PRIMARY);
                }
                TextStyle::Heading2 => {
                    text = text.size(15.0).strong().color(AppTheme::TEXT_PRIMARY);
                }
                TextStyle::Heading3 => {
                    text = text.size(14.0).strong().color(AppTheme::TEXT_PRIMARY);
                }
            }

            if let Some(ref url) = span.link_url {
                ui.hyperlink_to(text.color(AppTheme::ACCENT_HOVER).underline(), url);
            } else {
                ui.label(text);
            }
        }
    });
}
