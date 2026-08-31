use base64::Engine;
use email_core::models::MessageDetail;
use std::process::Command;

/// Prepares the full HTML payload with all inline cid: attachments resolved to base64 data URIs
pub fn prepare_email_html(detail: &MessageDetail) -> String {
    let raw_body = detail.body_html.as_deref().unwrap_or(detail.body_plain.as_deref().unwrap_or(""));
    let mut full_html = if detail.body_html.is_some() {
        raw_body.to_string()
    } else {
        format!(
            "<!DOCTYPE html><html><head><meta charset=\"utf-8\"></head><body style=\"font-family: monospace; white-space: pre-wrap; padding: 24px; background: #fff;\">{}</body></html>",
            html_escape::encode_text(raw_body)
        )
    };

    // Replace all cid: references with base64 data URIs
    for att in &detail.attachments {
        if let Some(ref cache_path) = att.local_cache_path {
            if let Ok(bytes) = std::fs::read(cache_path) {
                let mime = if !att.mime_type.is_empty() {
                    att.mime_type.as_str()
                } else if att.filename.ends_with(".png") {
                    "image/png"
                } else if att.filename.ends_with(".jpg") || att.filename.ends_with(".jpeg") {
                    "image/jpeg"
                } else if att.filename.ends_with(".gif") {
                    "image/gif"
                } else if att.filename.ends_with(".webp") {
                    "image/webp"
                } else {
                    "application/octet-stream"
                };

                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                let data_uri = format!("data:{};base64,{}", mime, b64);

                if let Some(ref cid) = att.content_id {
                    let clean_cid = cid.trim_matches(|c| c == '<' || c == '>');
                    full_html = full_html.replace(&format!("cid:{}", clean_cid), &data_uri);
                    full_html = full_html.replace(&format!("cid:<{}>", clean_cid), &data_uri);
                    full_html = full_html.replace(&format!("cid:{}", cid), &data_uri);
                }
                if !att.filename.is_empty() {
                    full_html = full_html.replace(&format!("cid:{}", att.filename), &data_uri);
                }
            }
        }
    }

    // Ensure standard HTML structure if missing
    if !full_html.to_lowercase().contains("<!doctype") && !full_html.to_lowercase().contains("<html") {
        full_html = format!(
            r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<style>
  html, body {{
    margin: 0;
    padding: 16px;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
    background-color: #ffffff;
    color: #222222;
  }}
</style>
</head>
<body>
{}
</body>
</html>"#,
            full_html
        );
    }

    full_html
}

/// Launches the in-app WebKit webview window process
pub fn open_webview_window(title: String, detail: &MessageDetail) {
    let html_content = prepare_email_html(detail);
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join(format!("email_webview_{}.html", uuid::Uuid::new_v4()));
    if let Err(e) = std::fs::write(&file_path, &html_content) {
        log::error!("Failed to write HTML content for webview: {}", e);
        return;
    }

    if let Ok(exe) = std::env::current_exe() {
        let _ = Command::new(exe)
            .arg("--webview")
            .arg(&file_path)
            .arg(&title)
            .spawn();
    }
}

/// Runs the standalone WebKit window on the main thread of the webview subprocess
pub fn run_standalone_webview(file_path: &std::path::Path, title: &str) {
    use tao::{
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoopBuilder},
        window::WindowBuilder,
    };
    use wry::WebViewBuilder;

    let event_loop = EventLoopBuilder::new().build();
    let window = match WindowBuilder::new()
        .with_title(title)
        .with_inner_size(tao::dpi::LogicalSize::new(960.0, 800.0))
        .build(&event_loop)
    {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Failed to create webview window: {}. Opening in default browser instead.", e);
            open_in_browser(file_path);
            return;
        }
    };

    let html_string = std::fs::read_to_string(file_path).unwrap_or_default();

    let _webview = match WebViewBuilder::new()
        .with_html(&html_string)
        .build(&window)
    {
        Ok(wv) => wv,
        Err(e) => {
            eprintln!("Failed to build webview: {}. Opening in default browser instead.", e);
            open_in_browser(file_path);
            return;
        }
    };

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            *control_flow = ControlFlow::Exit;
        }
    });
}

fn open_in_browser(path: &std::path::Path) {
    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("xdg-open").arg(path).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(path).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("cmd").args(["/C", "start", "", &path.to_string_lossy()]).spawn();
    }
}
