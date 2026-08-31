use egui::{
    epaint::Shadow, Color32, Margin, Rounding, Stroke, Visuals,
};

pub struct AppTheme;

impl AppTheme {
    // Primary Color Palette (Modern Dark Slate / Charcoal)
    pub const BG_APP: Color32 = Color32::from_rgb(18, 20, 24);         // #121418 - Deep background
    pub const BG_LIST: Color32 = Color32::from_rgb(26, 29, 35);        // #1a1d23 - Message list surface
    pub const BG_VIEW: Color32 = Color32::from_rgb(30, 34, 42);        // #1e222a - Reading pane surface
    pub const BG_CARD: Color32 = Color32::from_rgb(34, 38, 48);        // #222630 - Card surface
    pub const BG_HOVER: Color32 = Color32::from_rgb(42, 47, 60);       // #2a2f3c - Hover state
    pub const BG_SELECTED: Color32 = Color32::from_rgb(38, 62, 105);   // #263e69 - Selected message state
    pub const BG_UNREAD_ROW: Color32 = Color32::from_rgb(28, 33, 44);  // #1c212c - Unread row tint

    // Accents & State Colors
    pub const ACCENT_PRIMARY: Color32 = Color32::from_rgb(66, 133, 244);   // Vibrant Blue #4285f4
    pub const ACCENT_HOVER: Color32 = Color32::from_rgb(90, 150, 255);
    pub const ACCENT_STAR: Color32 = Color32::from_rgb(255, 193, 7);       // Star Gold #ffc107
    pub const ACCENT_SUCCESS: Color32 = Color32::from_rgb(52, 168, 83);    // Green #34a853
    pub const ACCENT_WARNING: Color32 = Color32::from_rgb(251, 146, 60);    // Amber Orange #fb923c
    pub const ACCENT_DANGER: Color32 = Color32::from_rgb(234, 67, 53);     // Red #ea4335


    // Text Hierarchy
    pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(240, 242, 245);
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(165, 172, 185);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(115, 122, 135);
    pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(45, 50, 62);

    pub fn apply(ctx: &egui::Context) {
        let mut visuals = Visuals::dark();

        visuals.override_text_color = Some(Self::TEXT_PRIMARY);
        visuals.panel_fill = Self::BG_APP;
        visuals.window_fill = Self::BG_VIEW;
        visuals.window_stroke = Stroke::new(1.0_f32, Self::BORDER_SUBTLE);
        visuals.window_rounding = Rounding::same(10.0);
        visuals.window_shadow = Shadow {
            offset: egui::vec2(0.0, 6.0),
            blur: 16.0_f32,
            spread: 0.0_f32,
            color: Color32::from_black_alpha(140),
        };


        // Widgets styling (Buttons, inputs, toggles)
        visuals.widgets.noninteractive.bg_fill = Self::BG_CARD;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Self::BORDER_SUBTLE);
        visuals.widgets.noninteractive.rounding = Rounding::same(6.0);

        visuals.widgets.inactive.bg_fill = Self::BG_CARD;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Self::BORDER_SUBTLE);
        visuals.widgets.inactive.rounding = Rounding::same(6.0);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Self::TEXT_PRIMARY);

        visuals.widgets.hovered.bg_fill = Self::BG_HOVER;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, Self::ACCENT_PRIMARY);
        visuals.widgets.hovered.rounding = Rounding::same(6.0);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);

        visuals.widgets.active.bg_fill = Self::ACCENT_PRIMARY;
        visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, Self::ACCENT_HOVER);
        visuals.widgets.active.rounding = Rounding::same(6.0);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);

        visuals.widgets.open.bg_fill = Self::BG_CARD;
        visuals.widgets.open.rounding = Rounding::same(6.0);

        visuals.selection.bg_fill = Self::BG_SELECTED;
        visuals.selection.stroke = Stroke::new(1.0_f32, Self::ACCENT_PRIMARY);

        let mut style = (*ctx.style()).clone();
        style.visuals = visuals;
        style.spacing.item_spacing = egui::Vec2::new(8.0, 6.0);
        style.spacing.button_padding = egui::Vec2::new(10.0, 6.0);
        style.spacing.window_margin = Margin::same(16.0);

        ctx.set_style(style);
    }

    /// Generates a consistent, attractive avatar color based on a sender string
    pub fn avatar_color(name: &str) -> Color32 {
        let mut hash: u32 = 0;
        for b in name.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(b as u32);
        }
        let palette = [
            Color32::from_rgb(233, 30, 99),   // Pink
            Color32::from_rgb(156, 39, 176),  // Purple
            Color32::from_rgb(103, 58, 183),  // Deep Purple
            Color32::from_rgb(63, 81, 181),   // Indigo
            Color32::from_rgb(33, 150, 243),  // Blue
            Color32::from_rgb(0, 150, 136),   // Teal
            Color32::from_rgb(76, 175, 80),   // Green
            Color32::from_rgb(255, 152, 0),   // Orange
            Color32::from_rgb(244, 67, 54),   // Red
            Color32::from_rgb(0, 188, 212),   // Cyan
        ];
        palette[(hash as usize) % palette.len()]
    }

    /// Extracts initials from name or email
    pub fn get_initials(name_or_email: &str) -> String {
        let clean = name_or_email.trim();
        if clean.is_empty() {
            return "?".to_string();
        }

        let parts: Vec<&str> = clean
            .split(|c: char| c.is_whitespace() || c == '.' || c == '@' || c == '_' || c == '-')
            .filter(|s| !s.is_empty())
            .collect();

        if parts.len() >= 2 {
            let first = parts[0].chars().next().unwrap_or('?');
            let second = parts[1].chars().next().unwrap_or('?');
            format!("{}{}", first.to_ascii_uppercase(), second.to_ascii_uppercase())
        } else if let Some(first_char) = clean.chars().next() {
            first_char.to_ascii_uppercase().to_string()
        } else {
            "?".to_string()
        }
    }
}
