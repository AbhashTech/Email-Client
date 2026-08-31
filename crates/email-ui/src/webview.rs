use std::thread;

/// Spawns a dedicated native WebKit webview window rendering pixel-perfect HTML
pub fn open_webview_window(title: String, html_content: String) {
    thread::spawn(move || {
        use tao::{
            event::{Event, WindowEvent},
            event_loop::{ControlFlow, EventLoopBuilder},
            window::WindowBuilder,
        };
        use wry::WebViewBuilder;

        let event_loop = EventLoopBuilder::new().build();
        
        let window = match WindowBuilder::new()
            .with_title(&title)
            .with_inner_size(tao::dpi::LogicalSize::new(900.0, 750.0))
            .build(&event_loop)
        {
            Ok(w) => w,
            Err(e) => {
                log::error!("Failed to create webview window: {}", e);
                return;
            }
        };

        let _webview = match WebViewBuilder::new()
            .with_html(&html_content)
            .build(&window)
        {
            Ok(wv) => wv,
            Err(e) => {
                log::error!("Failed to build webview: {}", e);
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
    });
}
