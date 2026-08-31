use crate::theme::AppTheme;
use egui::{Color32, FontId, Key, RichText, Rounding, Stroke, Vec2};

#[derive(Clone, Debug)]
pub enum PaletteAction {
    Compose,
    SyncAll,
    OpenSettings,
    ToggleSidebar,
    ToggleMessageList,
    SelectFolder(String), // folder id or special tag like "unread", "starred", "all"
    MarkRead,
    MarkUnread,
    ToggleStar,
    DeleteSelected,
    Reply,
    ReplyAll,
    Forward,
    FocusSearch,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct PaletteItem {
    pub id: String,
    pub title: String,
    pub category: String,
    pub shortcut: Option<String>,
    pub action: PaletteAction,
}

pub struct CommandPalette {
    pub is_open: bool,
    pub search_query: String,
    pub selected_idx: usize,
    items: Vec<PaletteItem>,
    filtered: Vec<PaletteItem>,
    just_opened: bool,
}

impl CommandPalette {
    pub fn new() -> Self {
        Self {
            is_open: false,
            search_query: String::new(),
            selected_idx: 0,
            items: Vec::new(),
            filtered: Vec::new(),
            just_opened: false,
        }
    }

    pub fn open(&mut self) {
        self.is_open = true;
        self.search_query.clear();
        self.selected_idx = 0;
        self.just_opened = true;
        self.update_filtered();
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.search_query.clear();
    }

    pub fn set_items(&mut self, items: Vec<PaletteItem>) {
        self.items = items;
        self.update_filtered();
    }

    fn update_filtered(&mut self) {
        let q = self.search_query.trim().to_lowercase();
        if q.is_empty() {
            self.filtered = self.items.clone();
        } else {
            self.filtered = self
                .items
                .iter()
                .filter(|item| {
                    item.title.to_lowercase().contains(&q)
                        || item.category.to_lowercase().contains(&q)
                        || item
                            .shortcut
                            .as_ref()
                            .map(|s| s.to_lowercase().contains(&q))
                            .unwrap_or(false)
                })
                .cloned()
                .collect();
        }
        if self.selected_idx >= self.filtered.len() {
            self.selected_idx = 0;
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<PaletteAction> {
        if !self.is_open {
            return None;
        }

        let mut executed_action = None;

        // Handle Escape to close
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.close();
            return None;
        }

        // Handle Arrow navigation
        if ctx.input(|i| i.key_pressed(Key::ArrowDown)) {
            if !self.filtered.is_empty() {
                self.selected_idx = (self.selected_idx + 1) % self.filtered.len();
            }
        }
        if ctx.input(|i| i.key_pressed(Key::ArrowUp)) {
            if !self.filtered.is_empty() {
                self.selected_idx = if self.selected_idx == 0 {
                    self.filtered.len() - 1
                } else {
                    self.selected_idx - 1
                };
            }
        }

        // Handle Enter to execute
        if ctx.input(|i| i.key_pressed(Key::Enter)) {
            if let Some(item) = self.filtered.get(self.selected_idx) {
                executed_action = Some(item.action.clone());
                self.close();
                return executed_action;
            }
        }

        let modal_width = 540.0;
        let modal_height = 380.0;

        let window = egui::Window::new("command_palette_modal")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .fixed_size(Vec2::new(modal_width, modal_height))
            .anchor(egui::Align2::CENTER_CENTER, Vec2::new(0.0, -60.0))
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(Color32::from_rgb(18, 22, 34))
                    .stroke(Stroke::new(1.5_f32, AppTheme::ACCENT_PRIMARY))
                    .rounding(Rounding::same(12.0))
                    .inner_margin(16.0),
            );

        window.show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("🔍").size(16.0));
                let search_box = egui::TextEdit::singleline(&mut self.search_query)
                    .hint_text("Type a command, folder, or action... (↑/↓, Enter, Esc)")
                    .desired_width(modal_width - 50.0)
                    .font(FontId::proportional(14.0));

                let response = ui.add(search_box);
                if self.just_opened {
                    response.request_focus();
                    self.just_opened = false;
                }
                if response.changed() {
                    self.update_filtered();
                }
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);

            if self.filtered.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        RichText::new("No matching commands found")
                            .color(AppTheme::TEXT_MUTED)
                            .size(13.0),
                    );
                });
            } else {
                egui::ScrollArea::vertical()
                    .max_height(280.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (idx, item) in self.filtered.iter().enumerate() {
                            let is_selected = idx == self.selected_idx;
                            let bg_color = if is_selected {
                                AppTheme::ACCENT_PRIMARY.linear_multiply(0.3)
                            } else {
                                Color32::TRANSPARENT
                            };

                            let frame = egui::Frame::none()
                                .fill(bg_color)
                                .rounding(Rounding::same(6.0))
                                .inner_margin(Vec2::new(8.0, 6.0));

                            let res = frame.show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(&item.category)
                                            .size(10.5)
                                            .color(AppTheme::TEXT_MUTED),
                                    );
                                    ui.label(RichText::new("•").size(10.0).color(AppTheme::TEXT_MUTED));
                                    ui.label(
                                        RichText::new(&item.title)
                                            .size(13.0)
                                            .strong()
                                            .color(if is_selected {
                                                Color32::WHITE
                                            } else {
                                                AppTheme::TEXT_PRIMARY
                                            }),
                                    );

                                    if let Some(ref sc) = item.shortcut {
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.label(
                                                    RichText::new(sc)
                                                        .size(11.0)
                                                        .color(AppTheme::ACCENT_PRIMARY),
                                                );
                                            },
                                        );
                                    }
                                });
                            });

                            if res.response.interact(egui::Sense::click()).clicked() {
                                executed_action = Some(item.action.clone());
                            }
                            if res.response.hovered() {
                                self.selected_idx = idx;
                            }
                        }
                    });
            }
        });

        if executed_action.is_some() {
            self.close();
        }

        executed_action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_palette_filtering() {
        let mut palette = CommandPalette::new();
        palette.set_items(vec![
            PaletteItem {
                id: "compose".into(),
                title: "Compose New Email".into(),
                category: "Actions".into(),
                shortcut: Some("c".into()),
                action: PaletteAction::Compose,
            },
            PaletteItem {
                id: "sync".into(),
                title: "Sync All Mailboxes".into(),
                category: "Actions".into(),
                shortcut: Some("F5".into()),
                action: PaletteAction::SyncAll,
            },
            PaletteItem {
                id: "unread".into(),
                title: "Smart View: Unread".into(),
                category: "Folders".into(),
                shortcut: None,
                action: PaletteAction::SelectFolder("unified_unread".into()),
            },
        ]);

        assert_eq!(palette.filtered.len(), 3);

        palette.search_query = "compose".into();
        palette.update_filtered();
        assert_eq!(palette.filtered.len(), 1);
        assert_eq!(palette.filtered[0].id, "compose");

        palette.search_query = "folders".into();
        palette.update_filtered();
        assert_eq!(palette.filtered.len(), 1);
        assert_eq!(palette.filtered[0].id, "unread");

        palette.search_query = "xyznonexistent".into();
        palette.update_filtered();
        assert_eq!(palette.filtered.len(), 0);
    }

    #[test]
    fn test_command_palette_open_close() {
        let mut palette = CommandPalette::new();
        palette.open();
        assert!(palette.is_open);
        assert!(palette.just_opened);

        palette.close();
        assert!(!palette.is_open);
    }
}

