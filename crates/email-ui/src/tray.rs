use email_core::events::SyncCommand;
use ksni::{menu::StandardItem, MenuItem, Tray, TrayMethods};
use log::{info, warn};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct EmailTray {
    pub unread_count: Arc<AtomicU32>,
    pub cmd_tx: mpsc::UnboundedSender<SyncCommand>,
}

impl Tray for EmailTray {
    fn id(&self) -> String {
        "com.atmail.emailapp".to_string()
    }

    fn title(&self) -> String {
        let count = self.unread_count.load(Ordering::Relaxed);
        if count > 0 {
            format!("AT-mail-rs ({})", count)
        } else {
            "AT-mail-rs".to_string()
        }
    }

    fn icon_name(&self) -> String {
        "mail-unread".to_string()
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let cmd_tx_sync = self.cmd_tx.clone();
        vec![
            StandardItem {
                label: "Sync All Mail".into(),
                activate: Box::new(move |_| {
                    let _ = cmd_tx_sync.send(SyncCommand::SyncAll);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit AT-mail-rs".into(),
                activate: Box::new(|_| {
                    std::process::exit(0);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }

}

pub struct AppTray {
    pub unread_counter: Arc<AtomicU32>,
    rt_handle: tokio::runtime::Handle,
    handle: Arc<std::sync::RwLock<Option<ksni::Handle<EmailTray>>>>,
}

impl AppTray {
    pub fn new(cmd_tx: mpsc::UnboundedSender<SyncCommand>, rt_handle: tokio::runtime::Handle) -> Self {
        let unread_counter = Arc::new(AtomicU32::new(0));
        let tray = EmailTray {
            unread_count: unread_counter.clone(),
            cmd_tx,
        };

        let handle_store = Arc::new(std::sync::RwLock::new(None));
        let handle_store_clone = handle_store.clone();

        rt_handle.spawn(async move {
            match tray.spawn().await {
                Ok(h) => {
                    info!("System tray (StatusNotifierItem) spawned successfully.");
                    if let Ok(mut lock) = handle_store_clone.write() {
                        *lock = Some(h);
                    }
                }
                Err(e) => {
                    warn!("Could not spawn system tray service: {}. Running without tray icon.", e);
                }
            }
        });

        Self {
            unread_counter,
            rt_handle,
            handle: handle_store,
        }
    }

    pub fn update_unread_count(&self, count: u32) {
        self.unread_counter.store(count, Ordering::Relaxed);
        if let Ok(lock) = self.handle.read() {
            if let Some(ref handle) = *lock {
                let handle_clone = handle.clone();
                self.rt_handle.spawn(async move {
                    let _ = handle_clone
                        .update(|tray| {
                            tray.unread_count.store(count, Ordering::Relaxed);
                        })
                        .await;
                });
            }
        }
    }
}




