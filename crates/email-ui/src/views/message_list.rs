use crate::theme::AppTheme;
use egui::{Color32, FontId, Rect, RichText, Rounding, ScrollArea, Sense, Stroke, Ui, Vec2};
use email_core::models::{Folder, MessageHeader};
use std::collections::HashSet;

pub struct MessageListView;

impl MessageListView {
    pub const ROW_HEIGHT: f32 = 88.0;

    pub fn show(
        ui: &mut Ui,
        messages: &[MessageHeader],
        selected_message_id: &mut Option<String>,
        selected_ids: &mut HashSet<String>,
        last_clicked_idx: &mut Option<usize>,
        search_query: &mut String,
        focus_search_requested: &mut bool,
        available_folders: &[Folder],
        on_toggle_read: &mut Option<(String, bool)>,
        on_toggle_flag: &mut Option<(String, bool)>,
        on_batch_delete: &mut Option<Vec<String>>,
        on_batch_move: &mut Option<(Vec<String>, String)>,
        on_batch_toggle_read: &mut Option<(Vec<String>, bool)>,
        on_batch_toggle_flag: &mut Option<(Vec<String>, bool)>,
    ) {
        // Drag-and-drop preview cursor following mouse
        if let Some(payload) = egui::DragAndDrop::payload::<Vec<String>>(ui.ctx()) {
            if let Some(pointer_pos) = ui.ctx().pointer_latest_pos() {
                egui::Area::new(egui::Id::new("dnd_message_drag_preview"))
                    .order(egui::Order::Tooltip)
                    .fixed_pos(pointer_pos + Vec2::new(14.0, 14.0))
                    .interactable(false)
                    .show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style())
                            .fill(AppTheme::BG_CARD)
                            .stroke(Stroke::new(1.5_f32, AppTheme::ACCENT_PRIMARY))
                            .rounding(Rounding::same(8.0))
                            .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                            .show(ui, |ui| {
                                let count = payload.len();
                                let msg = if count == 1 {
                                    "📁 Moving 1 email...".to_string()
                                } else {
                                    format!("📁 Moving {} emails...", count)
                                };
                                ui.label(RichText::new(msg).strong().color(Color32::WHITE));
                            });
                    });
            }
        }

        // Header Section: Search bar OR Multi-Select Action Bar
        ui.add_space(6.0);
        if !selected_ids.is_empty() {
            // Multi-Select Action Bar (Multi-row Responsive Wrapped)
            egui::Frame::none()
                .fill(AppTheme::BG_SELECTED)
                .stroke(Stroke::new(1.0_f32, AppTheme::ACCENT_PRIMARY))
                .rounding(Rounding::same(8.0))
                .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
                        ui.spacing_mut().button_padding = Vec2::new(7.0, 3.5);

                        let all_selected = !messages.is_empty() && messages.iter().all(|m| selected_ids.contains(&m.id));
                        let select_all_icon = if all_selected { "☑" } else { "☐" };
                        if ui.button(RichText::new(select_all_icon).size(13.0).strong()).on_hover_text("Toggle Select All").clicked() {
                            if all_selected {
                                selected_ids.clear();
                            } else {
                                for m in messages {
                                    selected_ids.insert(m.id.clone());
                                }
                            }
                        }

                        ui.label(
                            RichText::new(format!("{} selected", selected_ids.len()))
                                .size(12.0)
                                .strong()
                                .color(Color32::WHITE),
                        );

                        ui.separator();

                        // Batch Delete
                        if ui
                            .button(RichText::new("🗑 Delete").size(11.5).color(AppTheme::ACCENT_DANGER))
                            .on_hover_text("Delete all selected emails")
                            .clicked()
                        {
                            *on_batch_delete = Some(selected_ids.iter().cloned().collect());
                        }

                        // Batch Move Dropdown
                        if !available_folders.is_empty() {
                            egui::ComboBox::from_id_salt("batch_move_combo")
                                .selected_text(RichText::new("📁 Move").size(11.5))
                                .show_ui(ui, |ui| {
                                    for folder in available_folders {
                                        if ui.button(&folder.display_name).clicked() {
                                            *on_batch_move = Some((
                                                selected_ids.iter().cloned().collect(),
                                                folder.id.clone(),
                                            ));
                                        }
                                    }
                                });
                        }

                        // Batch Mark Read/Unread
                        if ui.button(RichText::new("✉ Read").size(11.0)).on_hover_text("Mark selected as read").clicked() {
                            *on_batch_toggle_read = Some((selected_ids.iter().cloned().collect(), true));
                        }

                        if ui.button(RichText::new("✉ Unread").size(11.0)).on_hover_text("Mark selected as unread").clicked() {
                            *on_batch_toggle_read = Some((selected_ids.iter().cloned().collect(), false));
                        }

                        // Batch Star
                        if ui.button(RichText::new("★").size(12.0).color(AppTheme::ACCENT_STAR)).on_hover_text("Star selected").clicked() {
                            *on_batch_toggle_flag = Some((selected_ids.iter().cloned().collect(), true));
                        }

                        if ui.button(RichText::new("× Clear").size(11.0)).on_hover_text("Deselect all").clicked() {
                            selected_ids.clear();
                        }
                    });
                });
        } else {
            // Standard Search & Filter Header Bar
            ui.horizontal(|ui| {
                let search_width = ui.available_width() - 8.0;
                let (rect, _) = ui.allocate_exact_size(Vec2::new(search_width, 32.0), Sense::hover());
                ui.painter().rect_filled(rect, Rounding::same(8.0), AppTheme::BG_CARD);
                ui.painter().rect_stroke(rect, Rounding::same(8.0), Stroke::new(1.0_f32, AppTheme::BORDER_SUBTLE));

                let mut search_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
                search_ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(RichText::new("🔍").size(12.0).color(AppTheme::TEXT_MUTED));
                    let response = ui.add(
                        egui::TextEdit::singleline(search_query)
                            .hint_text("Search emails (subject, sender, content)... [ / ]")
                            .frame(false)
                            .desired_width(search_width - 70.0),
                    );
                    if *focus_search_requested {
                        response.request_focus();
                        *focus_search_requested = false;
                    }
                    if !search_query.is_empty() {
                        if ui.small_button("✖").clicked() {
                            search_query.clear();
                        }
                    }

                    if !messages.is_empty() {
                        if ui.small_button("☑").on_hover_text("Select all").clicked() {
                            for m in messages {
                                selected_ids.insert(m.id.clone());
                            }
                        }
                    }
                });
            });

            // Quick Filter Chips Bar
            ui.add_space(3.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 3.0);

                let is_unread_active = has_search_token(search_query, "is:unread");
                let is_starred_active = has_search_token(search_query, "is:starred") || has_search_token(search_query, "is:flagged");
                let has_att_active = has_search_token(search_query, "has:attachment") || has_search_token(search_query, "has:attachments");
                let is_all_active = !is_unread_active && !is_starred_active && !has_att_active;

                let all_btn = ui.selectable_label(is_all_active, RichText::new("All").size(10.5));
                if all_btn.clicked() {
                    clear_search_filter_tokens(search_query);
                }
                if all_btn.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }

                let unread_btn = ui.selectable_label(is_unread_active, RichText::new("✉ Unread").size(10.5));
                if unread_btn.clicked() {
                    toggle_search_token(search_query, "is:unread");
                }
                if unread_btn.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }

                let star_btn = ui.selectable_label(
                    is_starred_active,
                    RichText::new("★ Starred").size(10.5).color(if is_starred_active { AppTheme::ACCENT_STAR } else { AppTheme::TEXT_MUTED }),
                );
                if star_btn.clicked() {
                    toggle_search_token(search_query, "is:starred");
                }
                if star_btn.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }

                let att_btn = ui.selectable_label(has_att_active, RichText::new("📎 Files").size(10.5));
                if att_btn.clicked() {
                    toggle_search_token(search_query, "has:attachment");
                }
                if att_btn.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            });
        }

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
                    let is_active_view = selected_message_id.as_deref() == Some(&msg.id);
                    let is_in_selection = selected_ids.contains(&msg.id);

                    let (rect, response) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), Self::ROW_HEIGHT),
                        Sense::click_and_drag(),
                    );
                    if response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    // Drag-and-drop source
                    if response.drag_started() {
                        let payload: Vec<String> = if is_in_selection && selected_ids.len() > 1 {
                            selected_ids.iter().cloned().collect()
                        } else {
                            selected_ids.clear();
                            selected_ids.insert(msg.id.clone());
                            *selected_message_id = Some(msg.id.clone());
                            vec![msg.id.clone()]
                        };
                        egui::DragAndDrop::set_payload(ui.ctx(), payload);
                    }

                    // Row click handling (supporting Ctrl/Cmd toggle and Shift range selection)
                    if response.clicked() {
                        let is_ctrl = ui.input(|i| i.modifiers.command || i.modifiers.ctrl);
                        let is_shift = ui.input(|i| i.modifiers.shift);

                        if is_ctrl {
                            if selected_ids.contains(&msg.id) {
                                selected_ids.remove(&msg.id);
                            } else {
                                selected_ids.insert(msg.id.clone());
                            }
                            *selected_message_id = Some(msg.id.clone());
                            *last_clicked_idx = Some(idx);
                        } else if is_shift {
                            if let Some(prev) = *last_clicked_idx {
                                let start = prev.min(idx);
                                let end = prev.max(idx);
                                for i in start..=end {
                                    if let Some(m) = messages.get(i) {
                                        selected_ids.insert(m.id.clone());
                                    }
                                }
                            } else {
                                selected_ids.insert(msg.id.clone());
                            }
                            *selected_message_id = Some(msg.id.clone());
                            *last_clicked_idx = Some(idx);
                        } else {
                            selected_ids.clear();
                            selected_ids.insert(msg.id.clone());
                            *selected_message_id = Some(msg.id.clone());
                            *last_clicked_idx = Some(idx);
                        }
                    }

                    // Background highlight
                    let bg_color = if is_in_selection || is_active_view {
                        AppTheme::BG_SELECTED
                    } else if response.hovered() {
                        AppTheme::BG_HOVER
                    } else if !msg.is_read {
                        AppTheme::BG_UNREAD_ROW
                    } else {
                        AppTheme::BG_LIST
                    };

                    ui.painter().rect_filled(rect, Rounding::same(6.0), bg_color);

                    if is_active_view || is_in_selection {
                        // Left active indicator
                        let indicator = Rect::from_min_size(rect.min, Vec2::new(3.5, Self::ROW_HEIGHT));
                        ui.painter().rect_filled(indicator, Rounding::same(2.0), AppTheme::ACCENT_PRIMARY);
                    }

                    // Child UI layout with hard boundary clipping
                    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
                    child_ui.set_clip_rect(rect);
                    child_ui.horizontal(|ui| {
                        ui.add_space(8.0);

                        // 1. Selection Checkbox
                        let (cb_rect, cb_resp) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), Sense::click());
                        if cb_resp.clicked() {
                            if selected_ids.contains(&msg.id) {
                                selected_ids.remove(&msg.id);
                            } else {
                                selected_ids.insert(msg.id.clone());
                            }
                            *selected_message_id = Some(msg.id.clone());
                            *last_clicked_idx = Some(idx);
                        }
                        if cb_resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }

                        let cb_border = if is_in_selection { AppTheme::ACCENT_PRIMARY } else { AppTheme::BORDER_SUBTLE };
                        let cb_bg = if is_in_selection { AppTheme::ACCENT_PRIMARY } else { Color32::TRANSPARENT };
                        ui.painter().rect_filled(cb_rect, Rounding::same(4.0), cb_bg);
                        ui.painter().rect_stroke(cb_rect, Rounding::same(4.0), Stroke::new(1.0_f32, cb_border));
                        if is_in_selection {
                            ui.painter().text(
                                cb_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "✓",
                                FontId::proportional(11.0),
                                Color32::WHITE,
                            );
                        }

                        ui.add_space(2.0);

                        // 2. Unread Blue Dot
                        let (dot_rect, dot_resp) = ui.allocate_exact_size(Vec2::new(10.0, 10.0), Sense::click());
                        if dot_resp.clicked() {
                            *on_toggle_read = Some((msg.id.clone(), !msg.is_read));
                        }
                        if dot_resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        let dot_color = if !msg.is_read {
                            AppTheme::ACCENT_PRIMARY
                        } else {
                            Color32::TRANSPARENT
                        };
                        ui.painter().circle_filled(dot_rect.center(), 3.5, dot_color);

                        // 3. Avatar Circle with Initials
                        let avatar_size = 30.0;
                        let (avatar_rect, _) = ui.allocate_exact_size(Vec2::new(avatar_size, avatar_size), Sense::hover());
                        let avatar_bg = AppTheme::avatar_color(msg.sender_display());
                        ui.painter().circle_filled(avatar_rect.center(), avatar_size / 2.0, avatar_bg);

                        let initials = AppTheme::get_initials(msg.sender_display());
                        ui.painter().text(
                            avatar_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            initials,
                            FontId::proportional(11.5),
                            Color32::WHITE,
                        );

                        ui.add_space(6.0);

                        // 4. Message Content Columns
                        let content_avail_width = (ui.available_width() - 8.0).max(60.0);
                        ui.vertical(|ui| {
                            ui.set_max_width(content_avail_width);
                            ui.add_space(6.0);

                            // Row 1: Sender Name + Star + Date
                            ui.horizontal(|ui| {
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.add_space(4.0);
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
                                    let star_btn = ui.button(RichText::new(star_char).size(13.0).color(star_color));
                                    if star_btn.clicked() {
                                        *on_toggle_flag = Some((msg.id.clone(), !msg.is_flagged));
                                    }
                                    if star_btn.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }

                                    // Left: Sender Name (strictly single-line with ellipsis)
                                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
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
                                        ui.add(egui::Label::new(sender_style).truncate());
                                    });
                                });
                            });

                            ui.add_space(2.0);

                            // Row 2: Subject Line (strictly single-line with ellipsis)
                            let subject_display = if msg.subject.trim().is_empty() {
                                "(No Subject)".to_string()
                            } else {
                                msg.subject.replace('\n', " ").replace('\r', " ")
                            };

                            let subj_color = if is_active_view || is_in_selection {
                                Color32::WHITE
                            } else if !msg.is_read {
                                AppTheme::TEXT_PRIMARY
                            } else {
                                AppTheme::TEXT_SECONDARY
                            };

                            let subj_style = if !msg.is_read {
                                RichText::new(subject_display).strong().size(12.0).color(subj_color)
                            } else {
                                RichText::new(subject_display).size(12.0).color(subj_color)
                            };
                            ui.add(egui::Label::new(subj_style).truncate());

                            ui.add_space(2.0);

                            // Row 3: Snippet Preview (strictly single-line with ellipsis)
                            let snippet_display = msg.snippet.replace('\n', " ").replace('\r', " ");
                            ui.add(
                                egui::Label::new(
                                    RichText::new(snippet_display)
                                        .size(11.0)
                                        .color(AppTheme::TEXT_MUTED),
                                )
                                .truncate(),
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

fn toggle_search_token(query: &mut String, token: &str) {
    let mut tokens: Vec<String> = query.split_whitespace().map(|s| s.to_string()).collect();
    if let Some(pos) = tokens.iter().position(|t| t.eq_ignore_ascii_case(token)) {
        tokens.remove(pos);
    } else {
        tokens.push(token.to_string());
    }
    *query = tokens.join(" ");
}

fn has_search_token(query: &str, token: &str) -> bool {
    query.split_whitespace().any(|t| t.eq_ignore_ascii_case(token))
}

fn clear_search_filter_tokens(query: &mut String) {
    let filter_tokens = ["is:unread", "is:read", "is:starred", "is:flagged", "has:attachment", "has:attachments"];
    let tokens: Vec<String> = query
        .split_whitespace()
        .filter(|t| !filter_tokens.iter().any(|ft| t.eq_ignore_ascii_case(ft)))
        .map(|s| s.to_string())
        .collect();
    *query = tokens.join(" ");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_token_helpers() {
        let mut q = "meeting is:unread".to_string();
        assert!(has_search_token(&q, "is:unread"));
        assert!(!has_search_token(&q, "is:starred"));

        toggle_search_token(&mut q, "is:starred");
        assert!(has_search_token(&q, "is:starred"));
        assert_eq!(q, "meeting is:unread is:starred");

        toggle_search_token(&mut q, "is:unread");
        assert!(!has_search_token(&q, "is:unread"));
        assert_eq!(q, "meeting is:starred");

        clear_search_filter_tokens(&mut q);
        assert_eq!(q, "meeting");
    }
}

