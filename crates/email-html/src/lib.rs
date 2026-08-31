use regex::Regex;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextStyle {
    Normal,
    Bold,
    Italic,
    BoldItalic,
    Code,
    Heading1,
    Heading2,
    Heading3,
}

#[derive(Debug, Clone)]
pub struct FormattedSpan {
    pub text: String,
    pub style: TextStyle,
    pub link_url: Option<String>,
    pub text_color: Option<(u8, u8, u8)>,
}

#[derive(Debug, Clone)]
pub enum HtmlBlock {
    Paragraph {
        spans: Vec<FormattedSpan>,
        is_center: bool,
    },
    Heading {
        level: u8,
        text: String,
        is_center: bool,
        color: Option<(u8, u8, u8)>,
    },
    Button {
        text: String,
        url: String,
        bg_color: (u8, u8, u8),
        text_color: (u8, u8, u8),
        is_center: bool,
    },
    ListItem(Vec<FormattedSpan>),
    Blockquote(String),
    CodeBlock(String),
    Image {
        src: String,
        alt: Option<String>,
        is_center: bool,
    },
    HorizontalRule,
}

static SCRIPT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap()
});

static STYLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap()
});

static HEAD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<head[^>]*>.*?</head>").unwrap()
});

static XML_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<\?xml[^>]*>|<!DOCTYPE[^>]*>|<xml[^>]*>.*?</xml>").unwrap()
});

static COMMENT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<!--.*?-->|<!\[if[^\]]*\]>.*?<!\[endif\]>|<!\[endif\]>|<!\[if[^\]]*\]>").unwrap()
});

static META_LINK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<meta[^>]*>|<link[^>]*>|<title[^>]*>.*?</title>").unwrap()
});

static TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)<(/?[a-z0-9]+)([^>]*)>").unwrap()
});

static BR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)<br\s*/?>").unwrap()
});

pub fn sanitize_raw_html(html: &str) -> String {
    let s0 = COMMENT_RE.replace_all(html, "");
    let s1 = SCRIPT_RE.replace_all(&s0, "");
    let s2 = STYLE_RE.replace_all(&s1, "");
    let s3 = HEAD_RE.replace_all(&s2, "");
    let s4 = XML_RE.replace_all(&s3, "");
    META_LINK_RE.replace_all(&s4, "").to_string()
}

pub fn parse_color_string(s: &str) -> Option<(u8, u8, u8)> {
    let clean = s.trim().to_lowercase();
    if clean.starts_with('#') {
        let hex = clean.trim_start_matches('#');
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some((r, g, b));
        } else if hex.len() == 3 {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            return Some((r, g, b));
        }
    } else if clean.starts_with("rgb(") && clean.ends_with(')') {
        let inner = &clean[4..clean.len() - 1];
        let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
        if parts.len() >= 3 {
            let r = parts[0].parse::<u8>().ok()?;
            let g = parts[1].parse::<u8>().ok()?;
            let b = parts[2].parse::<u8>().ok()?;
            return Some((r, g, b));
        }
    } else {
        match clean.as_str() {
            "black" => return Some((0, 0, 0)),
            "white" => return Some((255, 255, 255)),
            "red" => return Some((220, 38, 38)),
            "orange" => return Some((234, 88, 12)),
            "blue" => return Some((37, 99, 235)),
            "green" => return Some((22, 163, 74)),
            "gray" | "grey" => return Some((107, 114, 128)),
            _ => {}
        }
    }
    None
}

fn parse_inline_styles(style_attr: &str) -> (Option<(u8, u8, u8)>, Option<(u8, u8, u8)>, Option<TextStyle>, bool) {
    let mut text_color = None;
    let mut bg_color = None;
    let mut text_style = None;
    let mut is_center = false;

    for decl in style_attr.split(';') {
        let mut parts = decl.splitn(2, ':');
        if let (Some(prop), Some(val)) = (parts.next(), parts.next()) {
            let prop_clean = prop.trim().to_lowercase();
            let val_clean = val.trim();

            if prop_clean == "color" {
                text_color = parse_color_string(val_clean);
            } else if prop_clean == "background-color" || prop_clean == "background" {
                bg_color = parse_color_string(val_clean);
            } else if prop_clean == "font-size" {
                if let Some(px_str) = val_clean.strip_suffix("px") {
                    if let Ok(px) = px_str.trim().parse::<f32>() {
                        if px >= 22.0 {
                            text_style = Some(TextStyle::Heading1);
                        } else if px >= 17.0 {
                            text_style = Some(TextStyle::Heading2);
                        } else if px >= 15.0 {
                            text_style = Some(TextStyle::Heading3);
                        }
                    }
                } else if val_clean.contains("large") || val_clean.contains("x-large") || val_clean.contains("xx-large") {
                    text_style = Some(TextStyle::Heading1);
                }
            } else if prop_clean == "font-weight" {
                if val_clean == "bold" || val_clean == "700" || val_clean == "800" || val_clean == "900" || val_clean == "bolder" {
                    if text_style.is_none() {
                        text_style = Some(TextStyle::Bold);
                    }
                }
            } else if prop_clean == "text-align" && val_clean.to_lowercase().contains("center") {
                is_center = true;
            }
        }
    }

    (text_color, bg_color, text_style, is_center)
}

/// Sanitize and parse HTML into structured blocks suitable for lightweight native rendering
pub fn parse_html_to_blocks(html: &str) -> Vec<HtmlBlock> {
    if html.trim().is_empty() {
        return Vec::new();
    }

    // 1. Sanitize HTML comments, head, style, script, xml, meta tags
    let cleaned = sanitize_raw_html(html);

    // 2. Replace <br> with newlines
    let with_newlines = BR_RE.replace_all(&cleaned, "\n");

    let mut blocks = Vec::new();
    let mut current_spans: Vec<FormattedSpan> = Vec::new();
    let mut is_bold = false;
    let mut is_italic = false;
    let mut is_code = false;
    let mut current_color: Option<(u8, u8, u8)> = None;
    let mut current_bg: Option<(u8, u8, u8)> = None;
    let mut current_override_style: Option<TextStyle> = None;
    let mut current_is_center = false;
    let mut current_link: Option<String> = None;

    let mut last_idx = 0;

    for mat in TAG_RE.find_iter(&with_newlines) {
        let text_before = &with_newlines[last_idx..mat.start()];
        if !text_before.is_empty() {
            let decoded = html_escape::decode_html_entities(text_before).to_string();
            let clean_text = clean_whitespace(&decoded);
            if !clean_text.is_empty() {
                let style = if let Some(ref st) = current_override_style {
                    st.clone()
                } else if is_code {
                    TextStyle::Code
                } else if is_bold && is_italic {
                    TextStyle::BoldItalic
                } else if is_bold {
                    TextStyle::Bold
                } else if is_italic {
                    TextStyle::Italic
                } else {
                    TextStyle::Normal
                };

                current_spans.push(FormattedSpan {
                    text: clean_text,
                    style,
                    link_url: current_link.clone(),
                    text_color: current_color,
                });
            }
        }

        let tag_match = TAG_RE.captures(mat.as_str()).unwrap();
        let tag_name = tag_match.get(1).unwrap().as_str().to_lowercase();
        let tag_attrs = tag_match.get(2).map(|m| m.as_str()).unwrap_or("");

        // Check align attribute
        if let Some(align) = extract_attribute(tag_attrs, "align") {
            if align.to_lowercase() == "center" {
                current_is_center = true;
            }
        }

        // Check inline style
        if let Some(style_str) = extract_attribute(tag_attrs, "style") {
            let (col, bg, st_opt, center) = parse_inline_styles(&style_str);
            if col.is_some() {
                current_color = col;
            }
            if bg.is_some() {
                current_bg = bg;
            }
            if st_opt.is_some() {
                current_override_style = st_opt;
            }
            if center {
                current_is_center = true;
            }
        }

        // Check font color
        if let Some(col_attr) = extract_attribute(tag_attrs, "color") {
            if let Some(c) = parse_color_string(&col_attr) {
                current_color = Some(c);
            }
        }

        match tag_name.as_str() {
            "b" | "strong" => is_bold = true,
            "/b" | "/strong" => is_bold = false,
            "i" | "em" => is_italic = true,
            "/i" | "/em" => is_italic = false,
            "code" => is_code = true,
            "/code" => is_code = false,
            "/span" | "/font" => {
                current_color = None;
                current_override_style = None;
                current_bg = None;
            }
            "center" => current_is_center = true,
            "/center" => current_is_center = false,
            "a" => {
                if let Some(src) = extract_attribute(tag_attrs, "href") {
                    current_link = Some(src);
                }
            }
            "/a" => {
                // If this link was styled as a button with a background color
                if let (Some(url), Some(bg)) = (current_link.take(), current_bg.take()) {
                    if !current_spans.is_empty() {
                        let btn_text = current_spans.iter().map(|s| s.text.as_str()).collect::<String>();
                        if btn_text.len() <= 35 && !btn_text.contains('\n') {
                            current_spans.clear();
                            let fg = current_color.unwrap_or((255, 255, 255));
                            blocks.push(HtmlBlock::Button {
                                text: btn_text,
                                url,
                                bg_color: bg,
                                text_color: fg,
                                is_center: current_is_center,
                            });
                        }
                    }
                }
                current_link = None;
            }
            "img" => {
                if !current_spans.is_empty() {
                    blocks.push(HtmlBlock::Paragraph {
                        spans: std::mem::take(&mut current_spans),
                        is_center: current_is_center,
                    });
                }
                if let Some(src) = extract_attribute(tag_attrs, "src") {
                    let alt = extract_attribute(tag_attrs, "alt");
                    blocks.push(HtmlBlock::Image {
                        src,
                        alt,
                        is_center: current_is_center,
                    });
                }
            }
            "p" | "div" | "tr" | "table" => {
                if !current_spans.is_empty() {
                    blocks.push(HtmlBlock::Paragraph {
                        spans: std::mem::take(&mut current_spans),
                        is_center: current_is_center,
                    });
                }
            }
            "/p" | "/div" | "/tr" | "/table" => {
                if !current_spans.is_empty() {
                    // Check if this paragraph is actually a single large heading
                    if current_spans.len() == 1 && matches!(current_spans[0].style, TextStyle::Heading1 | TextStyle::Heading2) {
                        let span = current_spans.remove(0);
                        let level = if span.style == TextStyle::Heading1 { 1 } else { 2 };
                        blocks.push(HtmlBlock::Heading {
                            level,
                            text: span.text,
                            is_center: current_is_center,
                            color: span.text_color,
                        });
                    } else {
                        blocks.push(HtmlBlock::Paragraph {
                            spans: std::mem::take(&mut current_spans),
                            is_center: current_is_center,
                        });
                    }
                }
                current_is_center = false;
                current_color = None;
                current_bg = None;
                current_override_style = None;
            }
            "li" => {
                if !current_spans.is_empty() {
                    blocks.push(HtmlBlock::Paragraph {
                        spans: std::mem::take(&mut current_spans),
                        is_center: current_is_center,
                    });
                }
            }
            "/li" => {
                if !current_spans.is_empty() {
                    blocks.push(HtmlBlock::ListItem(std::mem::take(&mut current_spans)));
                }
            }
            "hr" | "hr/" => {
                if !current_spans.is_empty() {
                    blocks.push(HtmlBlock::Paragraph {
                        spans: std::mem::take(&mut current_spans),
                        is_center: current_is_center,
                    });
                }
                blocks.push(HtmlBlock::HorizontalRule);
            }
            "h1" => {
                if !current_spans.is_empty() {
                    blocks.push(HtmlBlock::Paragraph {
                        spans: std::mem::take(&mut current_spans),
                        is_center: current_is_center,
                    });
                }
                is_bold = true;
            }
            "/h1" => {
                is_bold = false;
                if !current_spans.is_empty() {
                    let text = current_spans.iter().map(|s| s.text.as_str()).collect::<String>();
                    current_spans.clear();
                    blocks.push(HtmlBlock::Heading {
                        level: 1,
                        text,
                        is_center: current_is_center,
                        color: current_color,
                    });
                }
            }
            "h2" | "h3" | "h4" => {
                if !current_spans.is_empty() {
                    blocks.push(HtmlBlock::Paragraph {
                        spans: std::mem::take(&mut current_spans),
                        is_center: current_is_center,
                    });
                }
                is_bold = true;
            }
            "/h2" | "/h3" | "/h4" => {
                is_bold = false;
                if !current_spans.is_empty() {
                    let text = current_spans.iter().map(|s| s.text.as_str()).collect::<String>();
                    current_spans.clear();
                    blocks.push(HtmlBlock::Heading {
                        level: 2,
                        text,
                        is_center: current_is_center,
                        color: current_color,
                    });
                }
            }
            _ => {}
        }

        last_idx = mat.end();
    }

    if last_idx < with_newlines.len() {
        let remaining = &with_newlines[last_idx..];
        let decoded = html_escape::decode_html_entities(remaining).to_string();
        let clean_text = clean_whitespace(&decoded);
        if !clean_text.is_empty() {
            current_spans.push(FormattedSpan {
                text: clean_text,
                style: TextStyle::Normal,
                link_url: None,
                text_color: None,
            });
        }
    }

    if !current_spans.is_empty() {
        blocks.push(HtmlBlock::Paragraph {
            spans: current_spans,
            is_center: current_is_center,
        });
    }

    // Filter out completely empty paragraphs
    blocks.into_iter().filter(|b| match b {
        HtmlBlock::Paragraph { spans, .. } => spans.iter().any(|s| !s.text.trim().is_empty()),
        HtmlBlock::ListItem(spans) => spans.iter().any(|s| !s.text.trim().is_empty()),
        HtmlBlock::Heading { text, .. } => !text.trim().is_empty(),
        _ => true,
    }).collect()
}

fn extract_attribute(attrs: &str, attr_name: &str) -> Option<String> {
    let lower_attrs = attrs.to_lowercase();
    let name_lower = attr_name.to_lowercase();

    let mut search_from = 0;
    while let Some(pos) = lower_attrs[search_from..].find(&name_lower) {
        let actual_pos = search_from + pos;
        if actual_pos > 0 {
            let prev_char = attrs[..actual_pos].chars().last().unwrap();
            if !prev_char.is_whitespace() && prev_char != '<' {
                search_from = actual_pos + name_lower.len();
                continue;
            }
        }

        let after_name = attrs[actual_pos + name_lower.len()..].trim_start();
        if after_name.starts_with('=') {
            let rest = after_name[1..].trim_start();
            let quote_char = rest.chars().next()?;
            if quote_char == '"' || quote_char == '\'' {
                let end_quote = rest[1..].find(quote_char)?;
                let extracted = rest[1..=end_quote].trim().to_string();
                if !extracted.is_empty() {
                    return Some(extracted);
                }
            } else {
                let end = rest.find(|c: char| c.is_whitespace() || c == '>').unwrap_or(rest.len());
                let extracted = rest[..end].trim().to_string();
                if !extracted.is_empty() {
                    return Some(extracted);
                }
            }
        }
        search_from = actual_pos + name_lower.len();
    }
    None
}

fn clean_whitespace(s: &str) -> String {
    s.replace('\u{00A0}', " ")
}

/// Convert HTML string into a clean, unformatted plain-text snippet
pub fn html_to_plain_text(html: &str) -> String {
    let cleaned = sanitize_raw_html(html);
    let with_newlines = BR_RE.replace_all(&cleaned, "\n");
    let no_tags = TAG_RE.replace_all(&with_newlines, " ");
    let decoded = html_escape::decode_html_entities(&no_tags);
    let normalized = decoded
        .lines()
        .map(|l| {
            let words: Vec<&str> = l.split_whitespace().collect();
            words.join(" ")
        })
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    normalized
}

/// Convert Markdown text into clean, email-safe HTML with inline styling
pub fn markdown_to_html(md: &str) -> String {
    let mut html = String::new();
    let lines: Vec<&str> = md.lines().collect();
    let mut in_code_block = false;
    let mut code_block_buf = String::new();
    let mut in_ul = false;
    let mut in_ol = false;

    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        // Code block toggle
        if trimmed.starts_with("```") {
            if in_code_block {
                html.push_str("<pre style=\"background-color: #1e2433; color: #e2e8f0; padding: 12px; border-radius: 6px; font-family: monospace; font-size: 13px; overflow-x: auto; margin: 10px 0;\"><code>");
                html.push_str(&html_escape::encode_text(&code_block_buf));
                html.push_str("</code></pre>\n");
                code_block_buf.clear();
                in_code_block = false;
            } else {
                if in_ul {
                    html.push_str("</ul>\n");
                    in_ul = false;
                }
                if in_ol {
                    html.push_str("</ol>\n");
                    in_ol = false;
                }
                in_code_block = true;
            }
            i += 1;
            continue;
        }

        if in_code_block {
            if !code_block_buf.is_empty() {
                code_block_buf.push('\n');
            }
            code_block_buf.push_str(line);
            i += 1;
            continue;
        }

        // Horizontal rule
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            if in_ul {
                html.push_str("</ul>\n");
                in_ul = false;
            }
            if in_ol {
                html.push_str("</ol>\n");
                in_ol = false;
            }
            html.push_str("<hr style=\"border: none; border-top: 1px solid #cbd5e1; margin: 16px 0;\" />\n");
            i += 1;
            continue;
        }

        // Headings
        if trimmed.starts_with("# ") {
            if in_ul { html.push_str("</ul>\n"); in_ul = false; }
            if in_ol { html.push_str("</ol>\n"); in_ol = false; }
            let content = inline_markdown_to_html(&trimmed[2..]);
            html.push_str(&format!("<h1 style=\"font-size: 22px; font-weight: bold; margin: 16px 0 8px 0; color: #1e293b;\">{}</h1>\n", content));
            i += 1;
            continue;
        }
        if trimmed.starts_with("## ") {
            if in_ul { html.push_str("</ul>\n"); in_ul = false; }
            if in_ol { html.push_str("</ol>\n"); in_ol = false; }
            let content = inline_markdown_to_html(&trimmed[3..]);
            html.push_str(&format!("<h2 style=\"font-size: 18px; font-weight: bold; margin: 14px 0 6px 0; color: #1e293b;\">{}</h2>\n", content));
            i += 1;
            continue;
        }
        if trimmed.starts_with("### ") {
            if in_ul { html.push_str("</ul>\n"); in_ul = false; }
            if in_ol { html.push_str("</ol>\n"); in_ol = false; }
            let content = inline_markdown_to_html(&trimmed[4..]);
            html.push_str(&format!("<h3 style=\"font-size: 15px; font-weight: bold; margin: 12px 0 4px 0; color: #1e293b;\">{}</h3>\n", content));
            i += 1;
            continue;
        }

        // Blockquotes
        if trimmed.starts_with("> ") || trimmed == ">" {
            if in_ul { html.push_str("</ul>\n"); in_ul = false; }
            if in_ol { html.push_str("</ol>\n"); in_ol = false; }
            let quote_text = if trimmed.len() > 2 { &trimmed[2..] } else { "" };
            let content = inline_markdown_to_html(quote_text);
            html.push_str(&format!("<blockquote style=\"border-left: 3px solid #3b82f6; margin: 10px 0; padding-left: 12px; color: #475569; font-style: italic;\">{}</blockquote>\n", content));
            i += 1;
            continue;
        }

        // Unordered lists
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
            if in_ol {
                html.push_str("</ol>\n");
                in_ol = false;
            }
            if !in_ul {
                html.push_str("<ul style=\"margin: 8px 0; padding-left: 24px;\">\n");
                in_ul = true;
            }
            let item_text = inline_markdown_to_html(&trimmed[2..]);
            html.push_str(&format!("<li style=\"margin: 4px 0;\">{}</li>\n", item_text));
            i += 1;
            continue;
        }

        // Ordered lists
        if let Some(pos) = trimmed.find(". ") {
            if pos > 0 && trimmed[..pos].chars().all(|c| c.is_ascii_digit()) {
                if in_ul {
                    html.push_str("</ul>\n");
                    in_ul = false;
                }
                if !in_ol {
                    html.push_str("<ol style=\"margin: 8px 0; padding-left: 24px;\">\n");
                    in_ol = true;
                }
                let item_text = inline_markdown_to_html(&trimmed[pos + 2..]);
                html.push_str(&format!("<li style=\"margin: 4px 0;\">{}</li>\n", item_text));
                i += 1;
                continue;
            }
        }

        // Blank line
        if trimmed.is_empty() {
            if in_ul {
                html.push_str("</ul>\n");
                in_ul = false;
            }
            if in_ol {
                html.push_str("</ol>\n");
                in_ol = false;
            }
            i += 1;
            continue;
        }

        // Regular paragraph
        if in_ul {
            html.push_str("</ul>\n");
            in_ul = false;
        }
        if in_ol {
            html.push_str("</ol>\n");
            in_ol = false;
        }
        let content = inline_markdown_to_html(trimmed);
        html.push_str(&format!("<p style=\"margin: 8px 0; line-height: 1.5;\">{}</p>\n", content));
        i += 1;
    }

    if in_code_block {
        html.push_str("<pre style=\"background-color: #1e2433; color: #e2e8f0; padding: 12px; border-radius: 6px; font-family: monospace; font-size: 13px; overflow-x: auto; margin: 10px 0;\"><code>");
        html.push_str(&html_escape::encode_text(&code_block_buf));
        html.push_str("</code></pre>\n");
    }
    if in_ul {
        html.push_str("</ul>\n");
    }
    if in_ol {
        html.push_str("</ol>\n");
    }

    html
}

fn inline_markdown_to_html(text: &str) -> String {
    let mut res = text.to_string();

    // 1. Links: [title](url)
    static LINK_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap());
    res = LINK_RE.replace_all(&res, "<a href=\"$2\" style=\"color: #2563eb; text-decoration: underline;\">$1</a>").to_string();

    // 2. Bold+Italic: ***text***
    static BOLD_ITALIC_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\*\*\*([^*]+)\*\*\*").unwrap());
    res = BOLD_ITALIC_RE.replace_all(&res, "<strong><em>$1</em></strong>").to_string();

    // 3. Bold: **text** or __text__
    static BOLD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\*\*([^*]+)\*\*|__([^_]+)__").unwrap());
    res = BOLD_RE.replace_all(&res, |caps: &regex::Captures| {
        let val = caps.get(1).or_else(|| caps.get(2)).map(|m| m.as_str()).unwrap_or("");
        format!("<strong>{}</strong>", val)
    }).to_string();

    // 4. Italic: *text* or _text_
    static ITALIC_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\*([^*]+)\*|_([^_]+)_").unwrap());
    res = ITALIC_RE.replace_all(&res, |caps: &regex::Captures| {
        let val = caps.get(1).or_else(|| caps.get(2)).map(|m| m.as_str()).unwrap_or("");
        format!("<em>{}</em>", val)
    }).to_string();

    // 5. Inline code: `code`
    static CODE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`([^`]+)`").unwrap());
    res = CODE_RE.replace_all(&res, "<code style=\"background-color: #f1f5f9; padding: 2px 5px; border-radius: 4px; font-family: monospace; font-size: 90%; color: #e11d48;\">$1</code>").to_string();

    res
}

/// Helper to extract host domain from a string or URL
pub fn extract_domain(input: &str) -> Option<String> {
    let clean = input.trim().to_lowercase();
    let without_scheme = clean
        .strip_prefix("https://")
        .or_else(|| clean.strip_prefix("http://"))
        .unwrap_or(&clean);

    let host_part = without_scheme.split(&['/', ':', '?', '#'][..]).next()?;
    let host = host_part.trim_start_matches("www.");
    if host.contains('.') && !host.contains(' ') && host.len() > 3 {
        Some(host.to_string())
    } else {
        None
    }
}

/// Detects potentially deceptive/phishing links where the display text
/// claims to be one domain (e.g. "paypal.com" or "https://bank.com") but the href
/// points to a completely different domain (e.g. "http://phish-site.com").
pub fn is_suspicious_link(display_text: &str, target_url: &str) -> bool {
    let display_domain = extract_domain(display_text);
    let target_domain = extract_domain(target_url);

    match (display_domain, target_domain) {
        (Some(disp), Some(target)) => {
            disp != target && !target.ends_with(&format!(".{}", disp))
        }
        _ => false,
    }
}

/// Checks if an HTML block list contains external remote images (http/https)
pub fn has_remote_images(blocks: &[HtmlBlock]) -> bool {
    blocks.iter().any(|b| match b {
        HtmlBlock::Image { src, .. } => src.starts_with("http://") || src.starts_with("https://"),
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_parsing() {
        let html = "<!--[if gte mso 9]> <![endif]--><div><h2>Meeting Notes</h2><p>Hello <b>World</b>! Check <a href=\"https://example.com\">this link</a>.</p><img src=\"https://example.com/logo.png\" alt=\"Logo\" /></div>";
        let blocks = parse_html_to_blocks(html);
        assert!(!blocks.is_empty());
        let has_img = blocks.iter().any(|b| matches!(b, HtmlBlock::Image { .. }));
        assert!(has_img);
        assert!(has_remote_images(&blocks));
    }

    #[test]
    fn test_plain_text_extract() {
        let html = "<!-- comments --><p>Hi John,<br>Let's meet <b>tomorrow</b> at 10 AM.</p>";
        let plain = html_to_plain_text(html);
        assert!(plain.contains("Hi John,"));
        assert!(plain.contains("tomorrow at 10 AM."));
        assert!(!plain.contains("comments"));
    }

    #[test]
    fn test_markdown_to_html_converter() {
        let md = "# Weekly Sprint\n\nHello **Team**,\n\nPlease review `crates/email-ui` and check [our roadmap](https://example.com).\n\n- Task 1\n- Task 2\n\n```rust\nfn main() {}\n```\n\n> Keep shipping!";
        let html = markdown_to_html(md);
        assert!(html.contains("<h1"));
        assert!(html.contains("<strong>Team</strong>"));
        assert!(html.contains("<code"));
        assert!(html.contains("<a href=\"https://example.com\""));
        assert!(html.contains("<li"));
        assert!(html.contains("<blockquote"));
    }

    #[test]
    fn test_suspicious_phishing_link_detector() {
        // Deceptive: Text says paypal.com but points to attacker site
        assert!(is_suspicious_link("https://paypal.com/login", "http://evil-phishing.com/steal"));
        assert!(is_suspicious_link("paypal.com", "https://paypal-fake-login.com"));

        // Safe: Text is normal words
        assert!(!is_suspicious_link("Click here to view dashboard", "https://app.company.com/dashboard"));

        // Safe: Text domain matches target domain
        assert!(!is_suspicious_link("google.com", "https://www.google.com/search"));
        assert!(!is_suspicious_link("https://github.com", "https://github.com/rust-lang/rust"));
    }
}
