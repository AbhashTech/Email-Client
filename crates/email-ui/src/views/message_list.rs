use crate::theme::AppTheme;
use egui::{Color32, FontId, Rect, RichText, Rounding, ScrollArea, Sense, Stroke, Ui, Vec2};
use email_core::models::MessageHeader;

pub struct MessageListView;

impl MessageListView {
    pub const ROW_HEIGHT: f32 = 88.0;

    pub fn show(
        ui: &mut Ui,
        messages: &[MessageHeader],
        selected_message_id: &mut Option<String>,
        search_query: &mut String,
        on_toggle_read: &mut Option<(String, bool)>,
        on_toggle_flag: &mut Option<(String, bool)>,
    ) {
        // Search & Filter Header Bar
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let search_width = ui.available_width() - 8.0;
            let (rect, _) = ui.allocate_exact_size(Vec2::new(search_width, 32.0), Sense::hover());
            ui.painter().rect_filled(rect, Rounding::same(8.0), AppTheme::BG_CARD);
            ui.painter().rect_stroke(rect, Rounding::same(8.0), Stroke::new(1.0_f32, AppTheme::BORDER_SUBTLE));

            let mut search_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
            search_ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(RichText::new("🔍").size(12.0).color(AppTheme::TEXT_MUTED));
                let _response = ui.add(
                    egui::TextEdit::singleline(search_query)
                        .hint_text("Search emails (subject, sender, content)...")
                        .frame(false)
                        .desired_width(search_width - 50.0),
                );
                if !search_query.is_empty() {
                    if ui.small_button("✖").clicked() {
                        search_query.clear();
                    }
                }
            });
        });

        ui.add_space(6.0);
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            Stroke::new(1.0_f32, AppTheme::BORDER_SUBTLE),
        );

        ui.add_space(2.0);

        if messages.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);
                ui.label(RichText::new("📭").size(32.0));
                ui.add_space(8.0);
                ui.label(
                    RichText::new("No messages in this folder")
                        .size(13.0)
                        .color(AppTheme::TEXT_MUTED),
                );
            });
            return;
        }

        // Virtualized ScrollArea
        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show_rows(ui, Self::ROW_HEIGHT, messages.len(), |ui, row_range| {
                for idx in row_range {
                    let msg = &messages[idx];
                    let is_selected = selected_message_id.as_deref() == Some(&msg.id);

                    let (rect, response) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), Self::ROW_HEIGHT),
                        Sense::click(),
                    );

                    if response.clicked() {
                        *selected_message_id = Some(msg.id.clone());
                    }

                    // Background highlight
                    let bg_color = if is_selected {
                        AppTheme::BG_SELECTED
                    } else if response.hovered() {
                        AppTheme::BG_HOVER
                    } else if !msg.is_read {
                        AppTheme::BG_UNREAD_ROW
                    } else {
                        AppTheme::BG_LIST
                    };

                    ui.painter().rect_filled(rect, Rounding::same(6.0), bg_color);

                    if is_selected {
                        // Left active indicator
                        let indicator = Rect::from_min_size(rect.min, Vec2::new(3.5, Self::ROW_HEIGHT));
                        ui.painter().rect_filled(indicator, Rounding::same(2.0), AppTheme::ACCENT_PRIMARY);
                    }

                    // Child UI layout
                    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
                    child_ui.horizontal(|ui| {
                        ui.add_space(8.0);

                        // 1. Unread Blue Dot
                        let (dot_rect, dot_resp) = ui.allocate_exact_size(Vec2::new(12.0, 12.0), Sense::click());
                        if dot_resp.clicked() {
                            *on_toggle_read = Some((msg.id.clone(), !msg.is_read));
                        }
                        let dot_color = if !msg.is_read {
                            AppTheme::ACCENT_PRIMARY
                        } else {
                            Color32::TRANSPARENT
                        };
                        ui.painter().circle_filled(dot_rect.center(), 4.0, dot_color);

                        // 2. Avatar Circle with Initials
                        let avatar_size = 32.0;
                        let (avatar_rect, _) = ui.allocate_exact_size(Vec2::new(avatar_size, avatar_size), Sense::hover());
                        let avatar_bg = AppTheme::avatar_color(msg.sender_display());
                        ui.painter().circle_filled(avatar_rect.center(), avatar_size / 2.0, avatar_bg);

                        let initials = AppTheme::get_initials(msg.sender_display());
                        ui.painter().text(
                            avatar_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            initials,
                            FontId::proportional(12.0),
                            Color32::WHITE,
                        );

                        ui.add_space(6.0);

                        // 3. Message Content Columns
                        ui.vertical(|ui| {
                            ui.add_space(6.0);

                            // Row 1: Sender Name + Star + Date
                            ui.horizontal(|ui| {
                                let sender_color = if !msg.is_read {
                                    AppTheme::TEXT_PRIMARY
                                } else {
                                    AppTheme::TEXT_SECONDARY
                                };
                                let sender_style = if !msg.is_read {
                                    RichText::new(msg.sender_display()).strong().size(13.0).color(sender_color)
                                } else {
                                    RichText::new(msg.sender_display()).size(13.0).color(sender_color)
                                };
                                ui.label(sender_style);

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new(msg.formatted_date())
                                            .size(11.0)
                                            .color(AppTheme::TEXT_MUTED),
                                    );

                                    // Star Toggle button
                                    let star_char = if msg.is_flagged { "★" } else { "☆" };
                                    let star_color = if msg.is_flagged {
                                        AppTheme::ACCENT_STAR
                                    } else {
                                        AppTheme::TEXT_MUTED
                                    };
                                    if ui.button(RichText::new(star_char).size(13.0).color(star_color)).clicked() {
                                        *on_toggle_flag = Some((msg.id.clone(), !msg.is_flagged));
                                    }
                                });
                            });

                            ui.add_space(1.0);

                            // Row 2: Subject Line
                            let truncated_subj = if msg.subject.is_empty() {
                                "(No Subject)".to_string()
                            } else if msg.subject.len() > 65 {
                                format!("{}...", &msg.subject[..62])
                            } else {
                                msg.subject.clone()
                            };

                            let subj_color = if is_selected {
                                Color32::WHITE
                            } else if !msg.is_read {
                                AppTheme::TEXT_PRIMARY
                            } else {
                                AppTheme::TEXT_SECONDARY
                            };

                            let subj_style = if !msg.is_read {
                                RichText::new(truncated_subj).strong().size(12.0).color(subj_color)
                            } else {
                                RichText::new(truncated_subj).size(12.0).color(subj_color)
                            };
                            ui.label(subj_style);

                            ui.add_space(1.0);

                            // Row 3: Snippet Preview
                            let snippet_text = if msg.snippet.len() > 75 {
                                format!("{}...", &msg.snippet[..72])
                            } else {
                                msg.snippet.clone()
                            };
                            ui.label(
                                RichText::new(snippet_text)
                                    .size(11.0)
                                    .color(AppTheme::TEXT_MUTED),
                            );

                            ui.add_space(4.0);
                        });
                    });

                    // Separator line between rows
                    ui.painter().hline(
                        rect.x_range(),
                        rect.bottom(),
                        Stroke::new(1.0_f32, AppTheme::BORDER_SUBTLE),
                    );

                }
            });
    }
}
