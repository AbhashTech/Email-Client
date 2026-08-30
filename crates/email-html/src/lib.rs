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
}

#[derive(Debug, Clone)]
pub enum HtmlBlock {
    Paragraph(Vec<FormattedSpan>),
    Heading { level: u8, text: String },
    ListItem(Vec<FormattedSpan>),
    Blockquote(String),
    CodeBlock(String),
    Image { src: String, alt: Option<String> },
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

fn sanitize_raw_html(html: &str) -> String {
    let s0 = COMMENT_RE.replace_all(html, "");
    let s1 = SCRIPT_RE.replace_all(&s0, "");
    let s2 = STYLE_RE.replace_all(&s1, "");
    let s3 = HEAD_RE.replace_all(&s2, "");
    let s4 = XML_RE.replace_all(&s3, "");
    META_LINK_RE.replace_all(&s4, "").to_string()
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
    let mut current_link: Option<String> = None;

    let mut last_idx = 0;

    for mat in TAG_RE.find_iter(&with_newlines) {
        let text_before = &with_newlines[last_idx..mat.start()];
        if !text_before.is_empty() {
            let decoded = html_escape::decode_html_entities(text_before).to_string();
            let clean_text = clean_whitespace(&decoded);
            if !clean_text.is_empty() {
                let style = if is_code {
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
                });
            }
        }

        let tag_match = TAG_RE.captures(mat.as_str()).unwrap();
        let tag_name = tag_match.get(1).unwrap().as_str().to_lowercase();
        let tag_attrs = tag_match.get(2).map(|m| m.as_str()).unwrap_or("");

        match tag_name.as_str() {
            "b" | "strong" => is_bold = true,
            "/b" | "/strong" => is_bold = false,
            "i" | "em" => is_italic = true,
            "/i" | "/em" => is_italic = false,
            "code" => is_code = true,
            "/code" => is_code = false,
            "a" => {
                if let Some(src) = extract_attribute(tag_attrs, "href") {
                    current_link = Some(src);
                }
            }
            "/a" => current_link = None,
            "img" => {
                if !current_spans.is_empty() {
                    blocks.push(HtmlBlock::Paragraph(std::mem::take(&mut current_spans)));
                }
                if let Some(src) = extract_attribute(tag_attrs, "src") {
                    let alt = extract_attribute(tag_attrs, "alt");
                    blocks.push(HtmlBlock::Image { src, alt });
                }
            }
            "p" | "div" | "tr" | "table" => {
                if !current_spans.is_empty() {
                    blocks.push(HtmlBlock::Paragraph(std::mem::take(&mut current_spans)));
                }
            }
            "/p" | "/div" | "/tr" | "/table" => {
                if !current_spans.is_empty() {
                    blocks.push(HtmlBlock::Paragraph(std::mem::take(&mut current_spans)));
                }
            }
            "li" => {
                if !current_spans.is_empty() {
                    blocks.push(HtmlBlock::Paragraph(std::mem::take(&mut current_spans)));
                }
            }
            "/li" => {
                if !current_spans.is_empty() {
                    blocks.push(HtmlBlock::ListItem(std::mem::take(&mut current_spans)));
                }
            }
            "hr" | "hr/" => {
                if !current_spans.is_empty() {
                    blocks.push(HtmlBlock::Paragraph(std::mem::take(&mut current_spans)));
                }
                blocks.push(HtmlBlock::HorizontalRule);
            }
            "h1" => {
                if !current_spans.is_empty() {
                    blocks.push(HtmlBlock::Paragraph(std::mem::take(&mut current_spans)));
                }
                is_bold = true;
            }
            "/h1" => {
                is_bold = false;
                if !current_spans.is_empty() {
                    let text = current_spans.iter().map(|s| s.text.as_str()).collect::<String>();
                    current_spans.clear();
                    blocks.push(HtmlBlock::Heading { level: 1, text });
                }
            }
            "h2" | "h3" | "h4" => {
                if !current_spans.is_empty() {
                    blocks.push(HtmlBlock::Paragraph(std::mem::take(&mut current_spans)));
                }
                is_bold = true;
            }
            "/h2" | "/h3" | "/h4" => {
                is_bold = false;
                if !current_spans.is_empty() {
                    let text = current_spans.iter().map(|s| s.text.as_str()).collect::<String>();
                    current_spans.clear();
                    blocks.push(HtmlBlock::Heading { level: 2, text });
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
            });
        }
    }

    if !current_spans.is_empty() {
        blocks.push(HtmlBlock::Paragraph(current_spans));
    }

    // Filter out completely empty paragraphs
    blocks.into_iter().filter(|b| match b {
        HtmlBlock::Paragraph(spans) => spans.iter().any(|s| !s.text.trim().is_empty()),
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
    }

    #[test]
    fn test_plain_text_extract() {
        let html = "<!-- comments --><p>Hi John,<br>Let's meet <b>tomorrow</b> at 10 AM.</p>";
        let plain = html_to_plain_text(html);
        assert!(plain.contains("Hi John,"));
        assert!(plain.contains("tomorrow at 10 AM."));
        assert!(!plain.contains("comments"));
    }
}
