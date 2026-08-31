use email_core::events::SyncCommand;
use ksni::{menu::StandardItem, MenuItem, Tray, TrayMethods};
use log::{info, warn};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    ToggleVisibility,
    ShowApp,
    HideApp,
    ComposeEmail,
    SyncAll,
    Quit,
}

pub struct EmailTray {
    pub unread_count: Arc<AtomicU32>,
    pub is_visible: Arc<AtomicBool>,
    pub cmd_tx: mpsc::UnboundedSender<SyncCommand>,
    pub action_tx: mpsc::UnboundedSender<TrayAction>,
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

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.action_tx.send(TrayAction::ToggleVisibility);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let cmd_tx_sync = self.cmd_tx.clone();
        let action_tx_toggle = self.action_tx.clone();
        let action_tx_compose = self.action_tx.clone();
        let action_tx_quit = self.action_tx.clone();
        let is_visible = self.is_visible.load(Ordering::Relaxed);

        let toggle_label = if is_visible {
            "👁 Hide Window"
        } else {
            "👁 Show Window"
        };

        vec![
            StandardItem {
                label: toggle_label.into(),
                activate: Box::new(move |_| {
                    let _ = action_tx_toggle.send(TrayAction::ToggleVisibility);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "✉ Compose Email".into(),
                activate: Box::new(move |_| {
                    let _ = action_tx_compose.send(TrayAction::ComposeEmail);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "🔄 Sync All Mail".into(),
                activate: Box::new(move |_| {
                    let _ = cmd_tx_sync.send(SyncCommand::SyncAll);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit AT-mail-rs".into(),
                activate: Box::new(move |_| {
                    let _ = action_tx_quit.send(TrayAction::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub struct AppTray {
    pub unread_counter: Arc<AtomicU32>,
    pub is_visible: Arc<AtomicBool>,
    pub action_rx: mpsc::UnboundedReceiver<TrayAction>,
    rt_handle: tokio::runtime::Handle,
    handle: Arc<std::sync::RwLock<Option<ksni::Handle<EmailTray>>>>,
}

impl AppTray {
    pub fn new(cmd_tx: mpsc::UnboundedSender<SyncCommand>, rt_handle: tokio::runtime::Handle) -> Self {
        let unread_counter = Arc::new(AtomicU32::new(0));
        let is_visible = Arc::new(AtomicBool::new(true));
        let (action_tx, action_rx) = mpsc::unbounded_channel::<TrayAction>();

        let tray = EmailTray {
            unread_count: unread_counter.clone(),
            is_visible: is_visible.clone(),
            cmd_tx,
            action_tx,
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
            is_visible,
            action_rx,
            rt_handle,
            handle: handle_store,
        }
    }

    pub fn set_visible(&self, visible: bool) {
        self.is_visible.store(visible, Ordering::Relaxed);
        if let Ok(lock) = self.handle.read() {
            if let Some(ref handle) = *lock {
                let handle_clone = handle.clone();
                self.rt_handle.spawn(async move {
                    let _ = handle_clone
                        .update(move |tray| {
                            tray.is_visible.store(visible, Ordering::Relaxed);
                        })
                        .await;
                });
            }
        }
    }

    pub fn update_unread_count(&self, count: u32) {
        self.unread_counter.store(count, Ordering::Relaxed);
        if let Ok(lock) = self.handle.read() {
            if let Some(ref handle) = *lock {
                let handle_clone = handle.clone();
                self.rt_handle.spawn(async move {
                    let _ = handle_clone
                        .update(move |tray| {
                            tray.unread_count.store(count, Ordering::Relaxed);
                        })
                        .await;
                });
            }
        }
    }

    pub fn try_recv_action(&mut self) -> Option<TrayAction> {
        self.action_rx.try_recv().ok()
    }
}




