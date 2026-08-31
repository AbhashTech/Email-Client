use egui::{epaint::Shadow, Color32, Margin, Rounding, Stroke, Visuals};
use email_core::models::CustomTheme;
use log::info;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemePreset {
    #[default]
    DarkSlate,
    SystemAuto,
    CatppuccinMocha,
    Nord,
    SolarizedDark,
    GruvboxDark,
    GruvboxLight,
    GruvboxAuto,
    OledBlack,
    LightClean,
}

impl ThemePreset {
    pub fn all() -> &'static [ThemePreset] {
        &[
            ThemePreset::DarkSlate,
            ThemePreset::SystemAuto,
            ThemePreset::CatppuccinMocha,
            ThemePreset::Nord,
            ThemePreset::SolarizedDark,
            ThemePreset::GruvboxDark,
            ThemePreset::GruvboxLight,
            ThemePreset::GruvboxAuto,
            ThemePreset::OledBlack,
            ThemePreset::LightClean,
        ]
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ThemePreset::DarkSlate => "Dark Slate (Default)",
            ThemePreset::SystemAuto => "System Auto (Follow OS)",
            ThemePreset::CatppuccinMocha => "Catppuccin Mocha",
            ThemePreset::Nord => "Nord Arctic",
            ThemePreset::SolarizedDark => "Solarized Dark",
            ThemePreset::GruvboxDark => "Gruvbox Retro Dark",
            ThemePreset::GruvboxLight => "Gruvbox Retro Light",
            ThemePreset::GruvboxAuto => "Gruvbox Auto (System)",
            ThemePreset::OledBlack => "OLED Pure Black",
            ThemePreset::LightClean => "Clean Daylight",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ThemePreset::DarkSlate => "Modern dark slate with Google Blue accents",
            ThemePreset::SystemAuto => "Automatically switches between Dark Slate & Clean Daylight based on OS theme",
            ThemePreset::CatppuccinMocha => "Soothing pastel dark aesthetic with lavender accents",
            ThemePreset::Nord => "Arctic bluish dark theme inspired by Nordic colors",
            ThemePreset::SolarizedDark => "Precision low-contrast warm green-dark palette",
            ThemePreset::GruvboxDark => "Warm retro groove dark palette with amber and gold accents",
            ThemePreset::GruvboxLight => "Warm retro groove light parchment palette with ochre accents",
            ThemePreset::GruvboxAuto => "Automatically switches between Gruvbox Dark & Light based on OS theme",
            ThemePreset::OledBlack => "Deep #000000 true black for OLED screens and power saving",
            ThemePreset::LightClean => "Bright, crisp daylight theme for well-lit environments",
        }
    }

    pub fn to_key(&self) -> &'static str {
        match self {
            ThemePreset::DarkSlate => "dark_slate",
            ThemePreset::SystemAuto => "system_auto",
            ThemePreset::CatppuccinMocha => "catppuccin_mocha",
            ThemePreset::Nord => "nord",
            ThemePreset::SolarizedDark => "solarized_dark",
            ThemePreset::GruvboxDark => "gruvbox_dark",
            ThemePreset::GruvboxLight => "gruvbox_light",
            ThemePreset::GruvboxAuto => "gruvbox_auto",
            ThemePreset::OledBlack => "oled_black",
            ThemePreset::LightClean => "light_clean",
        }
    }

    pub fn from_key(key: &str) -> Self {
        match key {
            "system_auto" => ThemePreset::SystemAuto,
            "catppuccin_mocha" => ThemePreset::CatppuccinMocha,
            "nord" => ThemePreset::Nord,
            "solarized_dark" => ThemePreset::SolarizedDark,
            "gruvbox_dark" => ThemePreset::GruvboxDark,
            "gruvbox_light" => ThemePreset::GruvboxLight,
            "gruvbox_auto" => ThemePreset::GruvboxAuto,
            "oled_black" => ThemePreset::OledBlack,
            "light_clean" => ThemePreset::LightClean,
            _ => ThemePreset::DarkSlate,
        }
    }
}

/// Filesystem and config path helpers for OS standard config folder
pub fn get_config_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        let mut p = PathBuf::from(home);
        p.push(".config");
        p.push("at-mail-rs");
        let _ = fs::create_dir_all(&p);
        p
    } else if let Ok(appdata) = std::env::var("APPDATA") {
        let mut p = PathBuf::from(appdata);
        p.push("at-mail-rs");
        let _ = fs::create_dir_all(&p);
        p
    } else {
        PathBuf::from(".config/at-mail-rs")
    }
}

pub fn get_themes_dir() -> PathBuf {
    let mut dir = get_config_dir();
    dir.push("themes");
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn load_custom_themes() -> Vec<CustomTheme> {
    let themes_dir = get_themes_dir();
    let mut list = Vec::new();
    if let Ok(entries) = fs::read_dir(&themes_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(theme) = serde_json::from_str::<CustomTheme>(&content) {
                        list.push(theme);
                    }
                }
            }
        }
    }
    list.sort_by(|a, b| a.name.cmp(&b.name));
    list
}

pub fn save_custom_theme(theme: &CustomTheme) -> Result<PathBuf, String> {
    let themes_dir = get_themes_dir();
    let file_path = themes_dir.join(format!("{}.json", theme.id));
    let json = serde_json::to_string_pretty(theme)
        .map_err(|e| format!("Failed to serialize theme: {}", e))?;
    fs::write(&file_path, json).map_err(|e| format!("Failed to write theme file: {}", e))?;
    info!("Saved custom theme to {:?}", file_path);
    Ok(file_path)
}

pub fn delete_custom_theme(theme_id: &str) -> Result<(), String> {
    let themes_dir = get_themes_dir();
    let file_path = themes_dir.join(format!("{}.json", theme_id));
    if file_path.exists() {
        fs::remove_file(&file_path).map_err(|e| format!("Failed to delete theme file: {}", e))?;
        info!("Deleted custom theme {:?}", file_path);
    }
    Ok(())
}

use std::sync::atomic::{AtomicI64, AtomicU8, Ordering};

static LAST_SYSTEM_THEME_CHECK: AtomicI64 = AtomicI64::new(0);
static CACHED_SYSTEM_THEME: AtomicU8 = AtomicU8::new(0); // 0: Dark, 1: Light

pub fn detect_system_theme(ctx: &egui::Context) -> egui::Theme {
    // 1. Try egui native window system theme query (instant zero-cost memory read)
    if let Some(st) = ctx.input(|i| i.raw.system_theme) {
        return st;
    }

    // Throttle external CLI execution to at most once every 5 seconds
    let now = chrono::Utc::now().timestamp();
    let last = LAST_SYSTEM_THEME_CHECK.load(Ordering::Relaxed);
    if now - last < 5 && last > 0 {
        return if CACHED_SYSTEM_THEME.load(Ordering::Relaxed) == 1 {
            egui::Theme::Light
        } else {
            egui::Theme::Dark
        };
    }

    LAST_SYSTEM_THEME_CHECK.store(now, Ordering::Relaxed);

    let detected = detect_system_theme_uncached();
    CACHED_SYSTEM_THEME.store(
        if detected == egui::Theme::Light { 1 } else { 0 },
        Ordering::Relaxed,
    );
    detected
}

fn detect_system_theme_uncached() -> egui::Theme {
    // 2. Linux GNOME / Freedesktop Portal / XDG gsettings check
    #[cfg(target_os = "linux")]
    {
        // Check standard freedesktop color-scheme via gsettings
        if let Ok(output) = std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "color-scheme"])
            .output()
        {
            let s = String::from_utf8_lossy(&output.stdout).to_lowercase();
            if s.contains("prefer-light") {
                return egui::Theme::Light;
            } else if s.contains("prefer-dark") {
                return egui::Theme::Dark;
            }
        }

        // Check GTK theme name
        if let Ok(output) = std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "gtk-theme"])
            .output()
        {
            let s = String::from_utf8_lossy(&output.stdout).to_lowercase();
            if s.contains("dark") {
                return egui::Theme::Dark;
            } else if s.contains("light") {
                return egui::Theme::Light;
            }
        }

        if let Ok(gtk_theme) = std::env::var("GTK_THEME") {
            let s = gtk_theme.to_lowercase();
            if s.contains("dark") {
                return egui::Theme::Dark;
            } else if s.contains("light") {
                return egui::Theme::Light;
            }
        }
    }

    // 3. macOS AppleInterfaceStyle check
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("defaults")
            .args(["read", "-g", "AppleInterfaceStyle"])
            .output()
        {
            let s = String::from_utf8_lossy(&output.stdout);
            if s.trim().eq_ignore_ascii_case("dark") {
                return egui::Theme::Dark;
            } else {
                return egui::Theme::Light;
            }
        }
    }

    egui::Theme::Dark
}

pub struct AppTheme;

#[allow(dead_code)]
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

    // Dynamic theme accessors
    #[inline]
    pub fn bg_app(ui: &egui::Ui) -> Color32 {
        ui.visuals().panel_fill
    }

    #[inline]
    pub fn bg_app_ctx(ctx: &egui::Context) -> Color32 {
        ctx.style().visuals.panel_fill
    }

    #[inline]
    pub fn bg_list(ui: &egui::Ui) -> Color32 {
        ui.visuals().panel_fill
    }

    #[inline]
    pub fn bg_view(ui: &egui::Ui) -> Color32 {
        ui.visuals().window_fill
    }

    #[inline]
    pub fn bg_view_ctx(ctx: &egui::Context) -> Color32 {
        ctx.style().visuals.window_fill
    }

    #[inline]
    pub fn bg_card(ui: &egui::Ui) -> Color32 {
        ui.visuals().widgets.noninteractive.bg_fill
    }

    #[inline]
    pub fn bg_card_ctx(ctx: &egui::Context) -> Color32 {
        ctx.style().visuals.widgets.noninteractive.bg_fill
    }

    #[inline]
    pub fn bg_hover(ui: &egui::Ui) -> Color32 {
        ui.visuals().widgets.hovered.bg_fill
    }

    #[inline]
    pub fn bg_selected(ui: &egui::Ui) -> Color32 {
        ui.visuals().selection.bg_fill
    }

    #[inline]
    pub fn bg_unread_row(ui: &egui::Ui) -> Color32 {
        if ui.visuals().dark_mode {
            Color32::from_rgb(28, 33, 44)
        } else {
            let base = ui.visuals().panel_fill;
            Color32::from_rgb(
                base.r().saturating_sub(10),
                base.g().saturating_sub(10),
                base.b().saturating_sub(15),
            )
        }
    }

    #[inline]
    pub fn accent(ui: &egui::Ui) -> Color32 {
        ui.visuals().widgets.active.bg_fill
    }

    #[inline]
    pub fn accent_ctx(ctx: &egui::Context) -> Color32 {
        ctx.style().visuals.widgets.active.bg_fill
    }

    #[inline]
    pub fn accent_hover(ui: &egui::Ui) -> Color32 {
        ui.visuals().widgets.active.bg_stroke.color
    }

    #[inline]
    pub fn border_subtle(ui: &egui::Ui) -> Color32 {
        ui.visuals().widgets.noninteractive.bg_stroke.color
    }

    #[inline]
    pub fn border_subtle_ctx(ctx: &egui::Context) -> Color32 {
        ctx.style().visuals.widgets.noninteractive.bg_stroke.color
    }

    #[inline]
    pub fn text_primary(ui: &egui::Ui) -> Color32 {
        ui.visuals().text_color()
    }

    #[inline]
    pub fn text_primary_ctx(ctx: &egui::Context) -> Color32 {
        ctx.style().visuals.text_color()
    }

    #[inline]
    pub fn text_secondary(ui: &egui::Ui) -> Color32 {
        if ui.visuals().dark_mode {
            Color32::from_rgb(165, 172, 185)
        } else {
            Color32::from_rgb(100, 105, 115)
        }
    }

    #[inline]
    pub fn text_secondary_ctx(ctx: &egui::Context) -> Color32 {
        if ctx.style().visuals.dark_mode {
            Color32::from_rgb(165, 172, 185)
        } else {
            Color32::from_rgb(100, 105, 115)
        }
    }

    #[inline]
    pub fn text_muted(ui: &egui::Ui) -> Color32 {
        ui.visuals().weak_text_color()
    }

    #[inline]
    pub fn text_muted_ctx(ctx: &egui::Context) -> Color32 {
        ctx.style().visuals.weak_text_color()
    }

    pub fn apply(ctx: &egui::Context) {
        Self::apply_preset(ctx, ThemePreset::DarkSlate);
    }

    pub fn apply_preset(ctx: &egui::Context, preset: ThemePreset) {
        let active_preset = match preset {
            ThemePreset::SystemAuto => {
                let sys_theme = detect_system_theme(ctx);
                if sys_theme == egui::Theme::Light {
                    ThemePreset::LightClean
                } else {
                    ThemePreset::DarkSlate
                }
            }
            ThemePreset::GruvboxAuto => {
                let sys_theme = detect_system_theme(ctx);
                if sys_theme == egui::Theme::Light {
                    ThemePreset::GruvboxLight
                } else {
                    ThemePreset::GruvboxDark
                }
            }
            other => other,
        };

        let (mut visuals, bg_app, bg_view, bg_card, bg_hover, accent, accent_hover, border, is_light) = match active_preset {
            ThemePreset::DarkSlate => (
                Visuals::dark(),
                Color32::from_rgb(18, 20, 24),
                Color32::from_rgb(30, 34, 42),
                Color32::from_rgb(34, 38, 48),
                Color32::from_rgb(42, 47, 60),
                Color32::from_rgb(66, 133, 244),
                Color32::from_rgb(90, 150, 255),
                Color32::from_rgb(45, 50, 62),
                false,
            ),
            ThemePreset::CatppuccinMocha => (
                Visuals::dark(),
                Color32::from_rgb(30, 30, 46),   // #1e1e2e
                Color32::from_rgb(24, 24, 37),   // #181825
                Color32::from_rgb(49, 50, 68),   // #313244
                Color32::from_rgb(69, 71, 90),   // #45475a
                Color32::from_rgb(137, 180, 250), // #89b4fa
                Color32::from_rgb(180, 190, 254), // #b4befe
                Color32::from_rgb(58, 60, 80),
                false,
            ),
            ThemePreset::Nord => (
                Visuals::dark(),
                Color32::from_rgb(46, 52, 64),   // #2e3440
                Color32::from_rgb(59, 66, 82),   // #3b4252
                Color32::from_rgb(67, 76, 94),   // #434c5e
                Color32::from_rgb(76, 86, 106),  // #4c566a
                Color32::from_rgb(136, 192, 208), // #88c0d0
                Color32::from_rgb(129, 161, 193), // #81a1c1
                Color32::from_rgb(76, 86, 106),
                false,
            ),
            ThemePreset::SolarizedDark => (
                Visuals::dark(),
                Color32::from_rgb(0, 43, 54),     // #002b36
                Color32::from_rgb(7, 54, 66),     // #073642
                Color32::from_rgb(14, 68, 82),
                Color32::from_rgb(20, 80, 96),
                Color32::from_rgb(42, 161, 152),  // #2aa198
                Color32::from_rgb(38, 139, 210),  // #268bd2
                Color32::from_rgb(15, 80, 96),
                false,
            ),
            ThemePreset::GruvboxDark => (
                Visuals::dark(),
                Color32::from_rgb(40, 40, 40),    // #282828 Gruvbox Dark 0
                Color32::from_rgb(60, 56, 54),    // #3c3836 Gruvbox Dark 1
                Color32::from_rgb(80, 73, 69),    // #504945 Gruvbox Dark 2
                Color32::from_rgb(102, 92, 84),   // #665c54 Gruvbox Dark 3
                Color32::from_rgb(250, 189, 47),  // #fabd2f Gruvbox Yellow
                Color32::from_rgb(254, 128, 25),  // #fe8019 Gruvbox Orange
                Color32::from_rgb(80, 73, 69),
                false,
            ),
            ThemePreset::GruvboxLight => (
                Visuals::light(),
                Color32::from_rgb(251, 241, 199), // #fbf1c7 Gruvbox Light 0 (bg_app)
                Color32::from_rgb(242, 229, 188), // #f2e5bc Gruvbox Light 1 (bg_view)
                Color32::from_rgb(235, 219, 178), // #ebdbb2 Gruvbox Light 2 (bg_card)
                Color32::from_rgb(213, 196, 161), // #d5c4a1 Gruvbox Light 3 (bg_hover)
                Color32::from_rgb(175, 58, 3),    // #af3a03 Gruvbox Rust/Orange
                Color32::from_rgb(215, 153, 33),  // #d79921 Gruvbox Dark Yellow
                Color32::from_rgb(213, 196, 161),
                true,
            ),
            ThemePreset::GruvboxAuto => unreachable!(),
            ThemePreset::SystemAuto => unreachable!(),
            ThemePreset::OledBlack => (
                Visuals::dark(),
                Color32::from_rgb(0, 0, 0),       // #000000
                Color32::from_rgb(10, 10, 10),
                Color32::from_rgb(20, 20, 20),
                Color32::from_rgb(34, 34, 34),
                Color32::from_rgb(59, 130, 246),  // #3b82f6
                Color32::from_rgb(96, 165, 250),
                Color32::from_rgb(38, 38, 38),
                false,
            ),
            ThemePreset::LightClean => (
                Visuals::light(),
                Color32::from_rgb(245, 247, 250), // #f5f7fa
                Color32::from_rgb(255, 255, 255), // #ffffff
                Color32::from_rgb(241, 245, 249), // #f1f5f9
                Color32::from_rgb(226, 232, 240), // #e2e8f0
                Color32::from_rgb(37, 99, 235),   // #2563eb
                Color32::from_rgb(29, 78, 216),
                Color32::from_rgb(203, 213, 225),
                true,
            ),
        };

        visuals.panel_fill = bg_app;
        visuals.window_fill = bg_view;
        visuals.window_stroke = Stroke::new(1.0_f32, border);
        visuals.window_rounding = Rounding::same(10.0);
        visuals.window_shadow = Shadow {
            offset: egui::vec2(0.0, 6.0),
            blur: 16.0_f32,
            spread: 0.0_f32,
            color: if is_light {
                Color32::from_black_alpha(40)
            } else {
                Color32::from_black_alpha(140)
            },
        };

        // Widgets styling (Buttons, inputs, toggles)
        visuals.widgets.noninteractive.bg_fill = bg_card;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, border);
        visuals.widgets.noninteractive.rounding = Rounding::same(6.0);

        visuals.widgets.inactive.bg_fill = bg_card;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, border);
        visuals.widgets.inactive.rounding = Rounding::same(6.0);

        visuals.widgets.hovered.bg_fill = bg_hover;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, accent);
        visuals.widgets.hovered.rounding = Rounding::same(6.0);

        visuals.widgets.active.bg_fill = accent;
        visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, accent_hover);
        visuals.widgets.active.rounding = Rounding::same(6.0);

        visuals.widgets.open.bg_fill = bg_card;
        visuals.widgets.open.rounding = Rounding::same(6.0);

        if is_light {
            visuals.widgets.noninteractive.fg_stroke.color = Color32::from_rgb(40, 40, 40);
            visuals.widgets.inactive.fg_stroke.color = Color32::from_rgb(55, 50, 45);
            visuals.widgets.hovered.fg_stroke.color = Color32::from_rgb(20, 20, 20);
            visuals.widgets.active.fg_stroke.color = Color32::WHITE;
            visuals.widgets.open.fg_stroke.color = Color32::from_rgb(40, 40, 40);
            visuals.extreme_bg_color = bg_app;
        }

        visuals.selection.bg_fill = if is_light {
            if active_preset == ThemePreset::GruvboxLight {
                Color32::from_rgb(235, 219, 178)
            } else {
                Color32::from_rgb(219, 234, 254)
            }
        } else {
            Color32::from_rgb(38, 62, 105)
        };
        visuals.selection.stroke = Stroke::new(1.0_f32, accent);

        let mut style = (*ctx.style()).clone();
        style.visuals = visuals;
        style.spacing.item_spacing = egui::Vec2::new(6.0, 4.0);
        style.spacing.button_padding = egui::Vec2::new(8.0, 4.5);
        style.spacing.interact_size.y = 26.0;
        style.spacing.window_margin = Margin::same(16.0);
        style.interaction.selectable_labels = false;

        ctx.set_style(style);
    }

    pub fn apply_custom(ctx: &egui::Context, theme: &CustomTheme) {
        let mut visuals = if theme.is_dark {
            Visuals::dark()
        } else {
            Visuals::light()
        };

        let bg_app = Color32::from_rgb(theme.bg_app[0], theme.bg_app[1], theme.bg_app[2]);
        let bg_view = Color32::from_rgb(theme.bg_view[0], theme.bg_view[1], theme.bg_view[2]);
        let bg_card = Color32::from_rgb(theme.bg_card[0], theme.bg_card[1], theme.bg_card[2]);
        let bg_hover = Color32::from_rgb(theme.bg_hover[0], theme.bg_hover[1], theme.bg_hover[2]);
        let bg_selected = Color32::from_rgb(theme.bg_selected[0], theme.bg_selected[1], theme.bg_selected[2]);
        let accent = Color32::from_rgb(theme.accent_primary[0], theme.accent_primary[1], theme.accent_primary[2]);
        let accent_hover = Color32::from_rgb(theme.accent_hover[0], theme.accent_hover[1], theme.accent_hover[2]);
        let border = Color32::from_rgb(theme.border[0], theme.border[1], theme.border[2]);
        let text_primary = Color32::from_rgb(theme.text_primary[0], theme.text_primary[1], theme.text_primary[2]);
        let text_secondary = Color32::from_rgb(theme.text_secondary[0], theme.text_secondary[1], theme.text_secondary[2]);

        visuals.panel_fill = bg_app;
        visuals.window_fill = bg_view;
        visuals.window_stroke = Stroke::new(1.0_f32, border);
        visuals.window_rounding = Rounding::same(10.0);
        visuals.window_shadow = Shadow {
            offset: egui::vec2(0.0, 6.0),
            blur: 16.0_f32,
            spread: 0.0_f32,
            color: if theme.is_dark {
                Color32::from_black_alpha(140)
            } else {
                Color32::from_black_alpha(40)
            },
        };

        visuals.widgets.noninteractive.bg_fill = bg_card;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, border);
        visuals.widgets.noninteractive.fg_stroke.color = text_primary;
        visuals.widgets.noninteractive.rounding = Rounding::same(6.0);

        visuals.widgets.inactive.bg_fill = bg_card;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, border);
        visuals.widgets.inactive.fg_stroke.color = text_secondary;
        visuals.widgets.inactive.rounding = Rounding::same(6.0);

        visuals.widgets.hovered.bg_fill = bg_hover;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, accent);
        visuals.widgets.hovered.fg_stroke.color = text_primary;
        visuals.widgets.hovered.rounding = Rounding::same(6.0);

        visuals.widgets.active.bg_fill = accent;
        visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, accent_hover);
        visuals.widgets.active.fg_stroke.color = Color32::WHITE;
        visuals.widgets.active.rounding = Rounding::same(6.0);

        visuals.widgets.open.bg_fill = bg_card;
        visuals.widgets.open.fg_stroke.color = text_primary;
        visuals.widgets.open.rounding = Rounding::same(6.0);

        visuals.selection.bg_fill = bg_selected;
        visuals.selection.stroke = Stroke::new(1.0_f32, accent);
        visuals.extreme_bg_color = bg_app;

        let mut style = (*ctx.style()).clone();
        style.visuals = visuals;
        style.spacing.item_spacing = egui::Vec2::new(6.0, 4.0);
        style.spacing.button_padding = egui::Vec2::new(8.0, 4.5);
        style.spacing.interact_size.y = 26.0;
        style.spacing.window_margin = Margin::same(16.0);
        style.interaction.selectable_labels = false;

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
