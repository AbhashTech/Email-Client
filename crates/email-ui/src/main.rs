mod app;
mod theme;
mod tray;
mod views;
pub mod webview;

use app::EmailApp;
use eframe::NativeOptions;
use email_core::events::{SyncCommand, SyncEvent};
use email_core::models::{Signature, Template};
use email_keychain::{CredentialStore, NativeKeyringStore};
use email_storage::Storage;
use email_sync::SyncWorker;
use log::info;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

fn main() -> Result<(), eframe::Error> {
    // Check if invoked as in-app WebKit webview subprocess
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "--webview" {
        let file_path = std::path::Path::new(&args[2]);
        let title = args.get(3).map(|s| s.as_str()).unwrap_or("Email Preview");
        crate::webview::run_standalone_webview(file_path, title);
        return Ok(());
    }

    // Install default Rustls cryptographic provider (Ring)
    let _ = rustls::crypto::ring::default_provider().install_default();

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("Starting AT-mail-rs Native Email Client...");

    // Determine local SQLite database path
    let db_path = get_database_path();
    info!("Database path: {:?}", db_path);

    let storage = Storage::new(&db_path).expect("Failed to initialize SQLite storage");
    let keyring: Arc<dyn CredentialStore> = Arc::new(NativeKeyringStore::new());

    // Seed initial templates and default signatures if empty
    seed_defaults(&storage);

    // Channels for Tokio background sync worker <-> egui UI thread
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<SyncCommand>();
    let (event_tx, event_rx) = broadcast::channel::<SyncEvent>(256);

    // Initialize multi-threaded Tokio runtime
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(4)
        .build()
        .expect("Failed to create Tokio runtime");

    let rt_handle = rt.handle().clone();

    // Enter Tokio runtime context for the main UI thread
    let _guard = rt.enter();

    // Spawn the background SyncWorker actor on the Tokio runtime
    let worker_storage = storage.clone();
    let worker_keyring = keyring.clone();
    let worker_event_tx = event_tx.clone();

    rt_handle.spawn(async move {
        let worker = SyncWorker::new(worker_storage, worker_keyring, cmd_rx, worker_event_tx);
        worker.run().await;
    });

    // Start background IMAP IDLE real-time push listeners for all accounts
    let idle_storage = storage.clone();
    let idle_keyring = keyring.clone();
    let idle_event_tx = event_tx.clone();
    email_sync::IdleWorker::start_for_all_accounts(idle_storage, idle_keyring, idle_event_tx);

    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("AT-mail-rs")
            .with_inner_size([1200.0, 780.0])
            .with_min_inner_size([880.0, 540.0]),
        ..Default::default()
    };

    let app_rt_handle = rt_handle.clone();
    eframe::run_native(
        "AT-mail-rs",
        options,
        Box::new(move |cc| {
            // Install GPU-accelerated image loaders (PNG, JPEG, WebP, URI)
            egui_extras::install_image_loaders(&cc.egui_ctx);

            // Apply clean modern dark theme
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(EmailApp::new(
                cc, storage, keyring, cmd_tx, event_rx, app_rt_handle,
            )))
        }),

    )
}


fn get_database_path() -> PathBuf {
    if let Some(mut dir) = dirs_next().or_else(dirs_fallback) {
        dir.push("at-mail-rs");
        let _ = std::fs::create_dir_all(&dir);
        dir.push("email_client.db");
        dir

    } else {
        PathBuf::from("email_client.db")
    }
}

fn dirs_next() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("HOME") {
        let mut p = PathBuf::from(home);
        p.push(".local");
        p.push("share");
        Some(p)
    } else {
        None
    }
}

fn dirs_fallback() -> Option<PathBuf> {
    std::env::current_dir().ok()
}

fn seed_defaults(storage: &Storage) {
    if let Ok(templates) = storage.get_templates() {
        if templates.is_empty() {
            let _ = storage.save_template(&Template::new(
                "Quick Follow-up".to_string(),
                "Following up on our conversation".to_string(),
                "Hi,\n\nJust wanted to quickly follow up on our previous discussion and see if you have any questions.\n\nBest,\n".to_string(),
                Some("/followup".to_string()),
            ));

            let _ = storage.save_template(&Template::new(
                "Meeting Confirmation".to_string(),
                "Meeting Confirmation".to_string(),
                "Hi,\n\nConfirming our scheduled meeting. Looking forward to speaking with you!\n\nBest regards,\n".to_string(),
                Some("/meet".to_string()),
            ));
        }
    }

    if let Ok(sigs) = storage.get_signatures(None) {
        if sigs.is_empty() {
            let _ = storage.save_signature(&Signature::new(
                None,
                "Professional Signature".to_string(),
                "<p><b>Kind regards,</b><br/>Software Engineering Team<br/><i>Sent with AT-mail-rs</i></p>".to_string(),
                true,
            ));

        }
    }
}
