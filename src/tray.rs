//! System tray via ksni (StatusNotifierItem).
//! Runs in its own thread with a dedicated tokio runtime.

use std::sync::{Mutex, OnceLock};

use ksni::menu::{MenuItem, StandardItem};
use ksni::{Tray, TrayMethods};
use tokio::sync::mpsc;

/// Global slot for the tray command receiver.
/// Set once during boot in `spawn_tray`, consumed once by the subscription stream.
pub static TRAY_RX: OnceLock<Mutex<Option<mpsc::UnboundedReceiver<TrayCommand>>>> =
    OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayCommand {
    Show,
    #[allow(dead_code)]
    Hide,
    Quit,
    MuteAll,
}

struct OsgTray {
    tx: mpsc::UnboundedSender<TrayCommand>,
}

impl OsgTray {
    fn send(&self, cmd: TrayCommand) {
        tracing::debug!("Tray command sent: {:?}", cmd);
        if let Err(e) = self.tx.send(cmd) {
            tracing::warn!("Tray command dropped — receiver closed: {e}");
        }
    }
}

impl Tray for OsgTray {
    fn id(&self) -> String {
        "open-sound-grid".into()
    }
    fn title(&self) -> String {
        "OpenSoundGrid".into()
    }
    fn icon_name(&self) -> String {
        "audio-volume-high".into()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        tracing::debug!("tray icon activated (clicked)");
        self.send(TrayCommand::Show);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Show".into(),
                activate: Box::new(|t: &mut Self| {
                    tracing::debug!("tray menu: Show clicked");
                    t.send(TrayCommand::Show);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Mute All".into(),
                icon_name: "audio-volume-muted".into(),
                activate: Box::new(|t: &mut Self| {
                    tracing::debug!("tray menu: Mute All clicked");
                    t.send(TrayCommand::MuteAll);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|t: &mut Self| {
                    tracing::debug!("tray menu: Quit clicked");
                    t.send(TrayCommand::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Spawn the system tray in its own thread with a dedicated tokio runtime.
///
/// Stores the tray command receiver in the global `TRAY_RX` slot so the iced
/// subscription can consume it on first tick.  The tray stays alive for the
/// entire lifetime of the process — `handle.shutdown()` is intentionally
/// **not** called (BUG-004 fix).
///
/// If the tray fails to start the app continues without it (logs a warning).
pub fn spawn_tray() {
    tracing::info!("Spawning system tray");
    let (tx, rx) = mpsc::unbounded_channel::<TrayCommand>();

    // Store the receiver globally before spawning so the subscription can
    // pick it up even if it initialises before the thread finishes booting.
    let _ = TRAY_RX.set(Mutex::new(Some(rx)));
    tracing::debug!("Tray command receiver stored in TRAY_RX");

    let tray = OsgTray { tx };

    match std::thread::Builder::new()
        .name("osg-tray".into())
        .spawn(move || {
            tracing::debug!("Tray thread started");
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::error!("Failed to create tray tokio runtime: {e}");
                    return;
                }
            };

            rt.block_on(async move {
                match tray.spawn().await {
                    Ok(_handle) => {
                        // BUG-004 fix: do NOT call handle.shutdown() here.
                        // Block forever so the tray remains registered with the
                        // StatusNotifierWatcher for the lifetime of the process.
                        tracing::info!("System tray started — running indefinitely");
                        std::future::pending::<()>().await;
                    }
                    Err(e) => tracing::warn!("System tray unavailable (D-Bus/SNI): {e}"),
                }
            });

            tracing::debug!("Tray thread exiting");
        }) {
        Ok(_) => tracing::debug!("Tray thread spawned"),
        Err(e) => tracing::warn!("Failed to spawn tray thread: {e}"),
    }
}
