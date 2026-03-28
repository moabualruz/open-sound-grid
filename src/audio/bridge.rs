use tokio::sync::mpsc;

use crate::audio::types::{AudioEvent, BackendCommand};

/// Channel pair for communication between the audio backend thread and the UI.
pub struct AudioBridge {
    /// Send commands from UI → backend
    pub command_tx: mpsc::UnboundedSender<BackendCommand>,
    /// Receive events from backend → UI
    pub event_rx: mpsc::UnboundedReceiver<AudioEvent>,
}

/// Backend-side handles (held by the audio thread).
pub struct BackendHandle {
    /// Receive commands from UI
    pub command_rx: mpsc::UnboundedReceiver<BackendCommand>,
    /// Send events to UI
    pub event_tx: mpsc::UnboundedSender<AudioEvent>,
}

/// Create a connected bridge pair.
pub fn create_bridge() -> (AudioBridge, BackendHandle) {
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let (evt_tx, evt_rx) = mpsc::unbounded_channel();

    (
        AudioBridge {
            command_tx: cmd_tx,
            event_rx: evt_rx,
        },
        BackendHandle {
            command_rx: cmd_rx,
            event_tx: evt_tx,
        },
    )
}
