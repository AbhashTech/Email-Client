pub fn configure_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // 1. Embedded font suite: all major scripts bundled into the binary
    // Guaranteed to render on any operating system without external dependencies
    let embedded_fonts: &[(&str, &[u8])] = &[
        // European, Cyrillic, Greek, IPA, math symbols, extended Latin
        ("DejaVuSans", include_bytes!("../assets/fonts/DejaVuSans.ttf")),
        // Indic languages
        ("NotoSansDevanagari", include_bytes!("../assets/fonts/NotoSansDevanagari-Regular.ttf")),
        ("NotoSansBengali", include_bytes!("../assets/fonts/NotoSansBengali-Regular.ttf")),
        ("NotoSansTamil", include_bytes!("../assets/fonts/NotoSansTamil-Regular.ttf")),
        ("NotoSansTelugu", include_bytes!("../assets/fonts/NotoSansTelugu-Regular.ttf")),
        ("NotoSansGujarati", include_bytes!("../assets/fonts/NotoSansGujarati-Regular.ttf")),
        ("NotoSansKannada", include_bytes!("../assets/fonts/NotoSansKannada-Regular.ttf")),
        ("NotoSansMalayalam", include_bytes!("../assets/fonts/NotoSansMalayalam-Regular.ttf")),
        ("NotoSansGurmukhi", include_bytes!("../assets/fonts/NotoSansGurmukhi-Regular.ttf")),
        ("NotoSansOriya", include_bytes!("../assets/fonts/NotoSansOriya-Regular.ttf")),
        ("NotoSansSinhala", include_bytes!("../assets/fonts/NotoSansSinhala-Regular.ttf")),
        // Middle Eastern
        ("NotoSansArabic", include_bytes!("../assets/fonts/NotoSansArabic-Regular.ttf")),
        ("NotoSansHebrew", include_bytes!("../assets/fonts/NotoSansHebrew-Regular.ttf")),
        // Southeast Asian
        ("NotoSansThai", include_bytes!("../assets/fonts/NotoSansThai-Regular.ttf")),
        ("NotoSansKhmer", include_bytes!("../assets/fonts/NotoSansKhmer-Regular.ttf")),
        ("NotoSansMyanmar", include_bytes!("../assets/fonts/NotoSansMyanmar-Regular.ttf")),
        // East Asian (Chinese, Japanese, Korean)
        ("NotoSansCJK", include_bytes!("../assets/fonts/NotoSansCJK-Regular.ttc")),
    ];

    for (name, bytes) in embedded_fonts {
        fonts.font_data.insert(
            name.to_string(),
            egui::FontData::from_static(bytes),
        );
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .push(name.to_string());
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push(name.to_string());
    }

    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_devanagari_font_loaded_and_renders() {
        let ctx = egui::Context::default();
        configure_fonts(&ctx);

        let marathi_sample = "प्रिय विद्यार्थ्यांनो, 'स्टुडंट प्रोफाईल सिस्टम' (SPS) मध्ये तुमचा ABC ID/APAAR ID";

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            ctx.fonts(|fonts| {
                let font_id = egui::FontId::proportional(14.0);
                let galley = fonts.layout_no_wrap(
                    marathi_sample.to_string(),
                    font_id,
                    egui::Color32::WHITE,
                );
                assert!(galley.size().x > 0.0);
                assert_eq!(galley.rows.len(), 1);
            });
        });
    }

    #[test]
    fn test_multilingual_fonts_render() {
        let ctx = egui::Context::default();
        configure_fonts(&ctx);

        // Samples across all embedded language families
        let samples = [
            ("Devanagari / Marathi", "प्रिय विद्यार्थ्यांनो (SPS)"),
            ("Cyrillic / Russian", "Здравствуйте, это тестовое письмо"),
            ("Greek", "Γειά σας, δοκιμαστικό μήνυμα"),
            ("Arabic", "مرحبا بكم في البريد الإلكتروني"),
            ("Hebrew", "שלום עולם"),
            ("Bengali", "সবাইকে স্বাগতম"),
            ("Tamil", "வணக்கம்"),
            ("Telugu", "నమస్కారం"),
            ("Gujarati", "નમસ્તે"),
            ("Kannada", "ನಮಸ್ಕಾರ"),
            ("Malayalam", "നമസ്കാരം"),
            ("Punjabi / Gurmukhi", "ਸਤਿ ਸ੍ਰੀ ਅਕਾਲ"),
            ("Odia", "ନମସ୍କାର"),
            ("Sinhala", "ආයුබෝවන්"),
            ("Thai", "สวัสดี"),
            ("Khmer", "ជំរាបសួរ"),
            ("Myanmar / Burmese", "မင်္ဂလာပါ"),
            ("Chinese", "你好世界"),
            ("Japanese", "こんにちは"),
            ("Korean", "안녕하세요"),
        ];

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            ctx.fonts(|fonts| {
                let font_id = egui::FontId::proportional(14.0);
                for (name, text) in samples {
                    let galley = fonts.layout_no_wrap(
                        text.to_string(),
                        font_id.clone(),
                        egui::Color32::WHITE,
                    );
                    assert!(galley.size().x > 0.0, "Failed to layout for {}", name);
                }
            });
        });
    }
}
