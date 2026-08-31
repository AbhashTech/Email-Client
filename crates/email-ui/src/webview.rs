use std::process::Command;

/// Launches the in-app WebKit webview window process
pub fn open_webview_window(title: String, html_content: String) {
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
        .with_inner_size(tao::dpi::LogicalSize::new(920.0, 780.0))
        .build(&event_loop)
    {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Failed to create webview window: {}", e);
            return;
        }
    };

    let url = format!("file://{}", file_path.display());
    let _webview = match WebViewBuilder::new()
        .with_url(&url)
        .build(&window)
    {
        Ok(wv) => wv,
        Err(e) => {
            eprintln!("Failed to build webview: {}", e);
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
