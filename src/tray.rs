//! System tray via ksni (StatusNotifierItem).
//! Runs in its own thread with a dedicated tokio runtime.

use ksni::menu::{MenuItem, StandardItem};
use ksni::{Tray, TrayMethods};
use tokio::sync::mpsc;

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
        let _ = self.tx.send(cmd);
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
        self.send(TrayCommand::Show);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Show".into(),
                activate: Box::new(|t: &mut Self| t.send(TrayCommand::Show)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Mute All".into(),
                icon_name: "audio-volume-muted".into(),
                activate: Box::new(|t: &mut Self| t.send(TrayCommand::MuteAll)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|t: &mut Self| t.send(TrayCommand::Quit)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Spawn the system tray in its own thread with a dedicated tokio runtime.
///
/// Returns a receiver for tray user actions.
/// If the tray fails to start, logs a warning — the app still works without it.
pub fn spawn_tray() -> mpsc::UnboundedReceiver<TrayCommand> {
    let (tx, rx) = mpsc::unbounded_channel();
    let tray = OsgTray { tx };

    std::thread::Builder::new()
        .name("osg-tray".into())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::warn!("Failed to create tray runtime: {e}");
                    return;
                }
            };

            rt.block_on(async move {
                match tray.spawn().await {
                    Ok(handle) => {
                        tracing::info!("System tray started");
                        handle.shutdown().await;
                    }
                    Err(e) => tracing::warn!("System tray unavailable: {e}"),
                }
            });
        })
        .ok();

    rx
}
