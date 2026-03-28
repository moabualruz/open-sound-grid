//! Plugin manager — discovers, initializes, and bridges plugins to the UI.

use tokio::sync::mpsc;

use crate::error::Result;
use crate::plugin::{AudioPlugin, PluginCommand, PluginEvent, PluginInfo, PluginResponse};

/// Bridges the plugin thread with the iced UI thread.
pub struct PluginBridge {
    /// Send commands from UI → plugin thread.
    pub command_tx: mpsc::UnboundedSender<PluginCommand>,
    /// Receive events from plugin thread → UI.
    pub event_rx: mpsc::UnboundedReceiver<PluginEvent>,
}

/// Handles held by the plugin thread.
struct PluginThreadHandle {
    command_rx: mpsc::UnboundedReceiver<PluginCommand>,
    event_tx: mpsc::UnboundedSender<PluginEvent>,
}

/// Manages the active audio plugin.
pub struct PluginManager {
    /// Info about the loaded plugin (available after start).
    pub plugin_info: Option<PluginInfo>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self { plugin_info: None }
    }

    /// Start a plugin in its own thread. Returns a bridge for communication.
    pub fn start(&mut self, mut plugin: Box<dyn AudioPlugin>) -> Result<PluginBridge> {
        let info = plugin.info();
        tracing::info!(
            "Starting plugin: {} v{} (API v{})",
            info.name,
            info.version,
            info.api_version
        );

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (evt_tx, evt_rx) = mpsc::unbounded_channel();

        let handle = PluginThreadHandle {
            command_rx: cmd_rx,
            event_tx: evt_tx,
        };

        self.plugin_info = Some(info);

        // Spawn the plugin thread
        std::thread::Builder::new()
            .name("osg-plugin".into())
            .spawn(move || {
                run_plugin_thread(&mut *plugin, handle);
            })
            .map_err(|e| crate::error::OsgError::PulseAudio(format!("Thread spawn: {e}")))?;

        Ok(PluginBridge {
            command_tx: cmd_tx,
            event_rx: evt_rx,
        })
    }
}

/// Plugin thread main loop.
fn run_plugin_thread(plugin: &mut dyn AudioPlugin, mut handle: PluginThreadHandle) {
    // Initialize
    if let Err(e) = plugin.init() {
        let _ = handle.event_tx.send(PluginEvent::Error(format!("Init failed: {e}")));
        return;
    }

    loop {
        // Process commands (non-blocking batch)
        while let Ok(cmd) = handle.command_rx.try_recv() {
            match cmd {
                PluginCommand::GetState => {
                    // For state queries, we send the response as an event
                    // since the bridge is async
                    match plugin.handle_command(PluginCommand::GetState) {
                        Ok(PluginResponse::State(snapshot)) => {
                            // State is delivered via the event channel
                            // The engine will handle this
                            let _ = handle.event_tx.send(PluginEvent::DevicesChanged);
                            // TODO: dedicated StateRefreshed event
                        }
                        Ok(_) => {}
                        Err(e) => {
                            let _ = handle
                                .event_tx
                                .send(PluginEvent::Error(format!("Command error: {e}")));
                        }
                    }
                }
                other => {
                    if let Err(e) = plugin.handle_command(other) {
                        let _ = handle
                            .event_tx
                            .send(PluginEvent::Error(format!("Command error: {e}")));
                    }
                }
            }
        }

        // Poll events from the plugin
        for event in plugin.poll_events() {
            if handle.event_tx.send(event).is_err() {
                // UI side dropped — shut down
                tracing::info!("Plugin bridge closed, shutting down");
                let _ = plugin.cleanup();
                return;
            }
        }

        // Sleep briefly to avoid busy-spin (plugin thread is not latency-critical)
        std::thread::sleep(std::time::Duration::from_millis(16)); // ~60Hz poll
    }
}
