//! System tray via ksni (StatusNotifierItem).
//! Communicates with the iced app via an mpsc channel.

use ksni::menu::{MenuItem, StandardItem};
use ksni::{Tray, TrayMethods};
use tokio::sync::mpsc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayCommand { Show, Hide, Quit, MuteAll }

struct OsgTray { tx: mpsc::UnboundedSender<TrayCommand> }

impl OsgTray {
    fn send(&self, cmd: TrayCommand) { let _ = self.tx.send(cmd); }
}

impl Tray for OsgTray {
    fn id(&self) -> String { "open-sound-grid".into() }
    fn title(&self) -> String { "OpenSoundGrid".into() }
    fn icon_name(&self) -> String { "audio-volume-high".into() }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.send(TrayCommand::Show);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Show".into(),
                activate: Box::new(|t: &mut Self| t.send(TrayCommand::Show)),
                ..Default::default()
            }.into(),
            StandardItem {
                label: "Mute All".into(),
                icon_name: "audio-volume-muted".into(),
                activate: Box::new(|t: &mut Self| t.send(TrayCommand::MuteAll)),
                ..Default::default()
            }.into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|t: &mut Self| t.send(TrayCommand::Quit)),
                ..Default::default()
            }.into(),
        ]
    }
}

/// Spawn the system tray and return a receiver for user actions.
///
/// If the tray fails to start (no SNI host), logs a warning and
/// returns a receiver from a dummy channel (never yields).
pub fn spawn_tray() -> mpsc::UnboundedReceiver<TrayCommand> {
    let (tx, rx) = mpsc::unbounded_channel();
    let tray = OsgTray { tx };

    tokio::spawn(async move {
        match tray.spawn().await {
            Ok(handle) => {
                tracing::info!("System tray started");
                handle.shutdown().await;
            }
            Err(e) => tracing::warn!("System tray unavailable: {e}"),
        }
    });

    rx
}
