use crate::theme::AppTheme;
use egui::{Color32, Rect, RichText, Rounding, ScrollArea, Sense, Stroke, Ui, Vec2};
use email_core::models::{Account, Folder};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderSelection {
    UnifiedFlagged,
    UnifiedUnread,
    Folder { account_id: String, folder_id: String },
}

pub struct SidebarView;

impl SidebarView {
    pub fn show(
        ui: &mut Ui,
        accounts: &[Account],
        folders_by_account: &HashMap<String, Vec<Folder>>,
        selected: &mut FolderSelection,
        on_add_account: &mut bool,
        on_open_settings: &mut bool,
        on_sync_all: &mut bool,
    ) {
        ui.vertical(|ui| {
            // App Brand Header
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(28.0, 28.0), Sense::hover());
                ui.painter().rect_filled(rect, Rounding::same(8.0), AppTheme::ACCENT_PRIMARY);
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "✉",
                    egui::FontId::proportional(16.0),
                    Color32::WHITE,
                );

                ui.vertical(|ui| {
                    ui.label(RichText::new("AT-mail-rs").strong().size(15.0).color(AppTheme::TEXT_PRIMARY));
                    ui.label(RichText::new("Fast Native Email").size(10.0).color(AppTheme::TEXT_MUTED));
                });


                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(RichText::new("🔄").size(13.0)).on_hover_text("Sync all mailboxes").clicked() {
                        *on_sync_all = true;
                    }
                });
            });

            ui.add_space(8.0);
            ui.painter().hline(
                ui.available_rect_before_wrap().x_range(),
                ui.cursor().top(),
                Stroke::new(1.0_f32, AppTheme::BORDER_SUBTLE),
            );
            ui.add_space(8.0);

            // Sidebar Navigation Scrollable Area
            ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                ui.vertical(|ui| {
                    // 1. Smart Views Section
                    ui.label(
                        RichText::new("SMART VIEWS")
                            .size(10.5)
                            .strong()
                            .color(AppTheme::TEXT_MUTED),
                    );
                    ui.add_space(4.0);

                    Self::render_folder_item(
                        ui,
                        "⭐",
                        "Starred",
                        0,
                        matches!(selected, FolderSelection::UnifiedFlagged),
                        || *selected = FolderSelection::UnifiedFlagged,
                    );

                    let total_unread_count: u32 = folders_by_account
                        .values()
                        .flatten()
                        .map(|f| f.unread_messages)
                        .sum();

                    Self::render_folder_item(
                        ui,
                        "🔵",
                        "Unread",
                        total_unread_count,
                        matches!(selected, FolderSelection::UnifiedUnread),
                        || *selected = FolderSelection::UnifiedUnread,
                    );

                    ui.add_space(14.0);
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        Stroke::new(1.0_f32, AppTheme::BORDER_SUBTLE),
                    );
                    ui.add_space(8.0);

                    // 2. Accounts Section
                    ui.label(
                        RichText::new("ACCOUNTS")
                            .size(10.5)
                            .strong()
                            .color(AppTheme::TEXT_MUTED),
                    );
                    ui.add_space(4.0);

                    if accounts.is_empty() {
                        ui.vertical_centered(|ui| {
                            ui.add_space(8.0);
                            ui.label(RichText::new("No accounts setup").size(12.0).color(AppTheme::TEXT_MUTED));
                            if ui.button(RichText::new("+ Add Account").size(11.0)).clicked() {
                                *on_add_account = true;
                            }
                        });
                    } else {
                        for account in accounts {
                            let header_text = format!("📂 {}", account.name);
                            egui::CollapsingHeader::new(
                                RichText::new(header_text)
                                    .size(12.5)
                                    .strong()
                                    .color(AppTheme::TEXT_PRIMARY),
                            )
                            .id_salt(format!("acc_collapse_{}", account.id))
                            .default_open(true)
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    if let Some(folders) = folders_by_account.get(&account.id) {
                                        for folder in folders {
                                            let is_sel = match selected {
                                                FolderSelection::Folder {
                                                    account_id,
                                                    folder_id,
                                                } => account_id == &account.id && folder_id == &folder.id,
                                                _ => false,
                                            };

                                            let icon = if folder.is_inbox() {
                                                "📥"
                                            } else if folder.is_sent() {
                                                "📤"
                                            } else if folder.is_drafts() {
                                                "📄"
                                            } else if folder.is_trash() {
                                                "🗑"
                                            } else {
                                                "📁"
                                            };

                                            Self::render_folder_item(
                                                ui,
                                                icon,
                                                &folder.display_name,
                                                folder.unread_messages,
                                                is_sel,
                                                || {
                                                    *selected = FolderSelection::Folder {
                                                        account_id: account.id.clone(),
                                                        folder_id: folder.id.clone(),
                                                    }
                                                },
                                            );
                                        }
                                    } else {
                                        ui.label(RichText::new("No folders discovered").size(11.0).color(AppTheme::TEXT_MUTED));
                                    }
                                });
                            });
                            ui.add_space(4.0);
                        }
                    }

                    ui.add_space(16.0);
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        Stroke::new(1.0_f32, AppTheme::BORDER_SUBTLE),
                    );
                    ui.add_space(8.0);

                    // 3. Management Section
                    ui.label(
                        RichText::new("MANAGEMENT")
                            .size(10.5)
                            .strong()
                            .color(AppTheme::TEXT_MUTED),
                    );
                    ui.add_space(6.0);

                    if ui.button(RichText::new("➕ Add Account").size(12.0)).clicked() {
                        *on_add_account = true;
                    }
                    if ui.button(RichText::new("⚙ Settings & Signatures").size(12.0)).clicked() {
                        *on_open_settings = true;
                    }
                });
            });
        });
    }

    fn render_folder_item(
        ui: &mut Ui,
        icon: &str,
        name: &str,
        unread: u32,
        is_selected: bool,
        mut on_click: impl FnMut(),
    ) {
        let height = 28.0;
        let width = ui.available_width().max(160.0);
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(width, height),
            Sense::click(),
        );

        if response.clicked() {
            on_click();
        }

        // Draw background
        let bg = if is_selected {
            AppTheme::BG_SELECTED
        } else if response.hovered() {
            AppTheme::BG_HOVER
        } else {
            Color32::TRANSPARENT
        };

        ui.painter().rect_filled(rect, Rounding::same(6.0), bg);

        if is_selected {
            // Accent bar on the left
            let indicator = Rect::from_min_size(rect.min, Vec2::new(3.0, height));
            ui.painter().rect_filled(indicator, Rounding::same(2.0), AppTheme::ACCENT_PRIMARY);
        }

        // Content
        let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        child_ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(RichText::new(icon).size(13.0));
            ui.add_space(2.0);

            let text_color = if is_selected {
                Color32::WHITE
            } else if unread > 0 {
                AppTheme::TEXT_PRIMARY
            } else {
                AppTheme::TEXT_SECONDARY
            };

            let text_style = if unread > 0 {
                RichText::new(name).size(12.5).strong().color(text_color)
            } else {
                RichText::new(name).size(12.5).color(text_color)
            };

            ui.label(text_style);

            // Badge Pill for unread messages
            if unread > 0 {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    let badge_text = if unread > 999 {
                        "999+".to_string()
                    } else {
                        unread.to_string()
                    };
                    let (badge_rect, _) = ui.allocate_exact_size(
                        Vec2::new(20.0 + (badge_text.len() as f32 * 2.0), 16.0),
                        Sense::hover(),
                    );
                    let badge_bg = if is_selected {
                        AppTheme::ACCENT_PRIMARY
                    } else {
                        AppTheme::BG_CARD
                    };
                    ui.painter().rect_filled(badge_rect, Rounding::same(8.0), badge_bg);
                    ui.painter().text(
                        badge_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        badge_text,
                        egui::FontId::proportional(10.0),
                        if is_selected { Color32::WHITE } else { AppTheme::TEXT_PRIMARY },
                    );
                });
            }
        });
    }
}
