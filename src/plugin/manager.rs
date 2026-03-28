//! Plugin manager — discovers, initializes, and bridges plugins to the UI.

use tracing::instrument;
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
    #[instrument(skip(self, plugin), fields(plugin_name = tracing::field::Empty, plugin_version = tracing::field::Empty))]
    pub fn start(&mut self, mut plugin: Box<dyn AudioPlugin>) -> Result<PluginBridge> {
        let info = plugin.info();
        tracing::Span::current().record("plugin_name", info.name);
        tracing::Span::current().record("plugin_version", info.version);
        tracing::info!(
            plugin.name = info.name,
            plugin.version = info.version,
            plugin.api_version = info.api_version,
            "starting plugin"
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
            .map_err(|e| {
                tracing::error!(error = %e, "failed to spawn plugin thread");
                crate::error::OsgError::PulseAudio(format!("Thread spawn: {e}"))
            })?;

        Ok(PluginBridge {
            command_tx: cmd_tx,
            event_rx: evt_rx,
        })
    }
}

/// Plugin thread main loop.
fn run_plugin_thread(plugin: &mut dyn AudioPlugin, mut handle: PluginThreadHandle) {
    tracing::info!("plugin thread started");
    // Initialize
    if let Err(e) = plugin.init() {
        tracing::error!(error = %e, "plugin init failed");
        let _ = handle.event_tx.send(PluginEvent::Error(format!("Init failed: {e}")));
        return;
    }
    tracing::info!("plugin initialized successfully");

    loop {
        // Process commands (non-blocking batch)
        let mut queue_depth: usize = 0;
        while let Ok(cmd) = handle.command_rx.try_recv() {
            queue_depth += 1;
            tracing::debug!(command = %cmd, "plugin thread received command");
            match cmd {
                PluginCommand::GetState => {
                    match plugin.handle_command(PluginCommand::GetState) {
                        Ok(PluginResponse::State(snapshot)) => {
                            let _ = handle
                                .event_tx
                                .send(PluginEvent::StateRefreshed(snapshot));
                        }
                        Ok(_) => {}
                        Err(e) => {
                            tracing::warn!(error = %e, "GetState command failed");
                            if handle
                                .event_tx
                                .send(PluginEvent::Error(format!("GetState error: {e}")))
                                .is_err()
                            {
                                tracing::error!("failed to send GetState error event: bridge closed");
                            }
                        }
                    }
                }
                other => {
                    match plugin.handle_command(other) {
                        Ok(_) => {
                            // Mutation succeeded — refresh state so the UI updates
                            match plugin.handle_command(PluginCommand::GetState) {
                                Ok(PluginResponse::State(snapshot)) => {
                                    let _ = handle
                                        .event_tx
                                        .send(PluginEvent::StateRefreshed(snapshot));
                                }
                                Ok(_) => {}
                                Err(e) => {
                                    tracing::warn!(error = %e, "post-mutation GetState failed");
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "command produced error response");
                            if handle
                                .event_tx
                                .send(PluginEvent::Error(format!("Command error: {e}")))
                                .is_err()
                            {
                                tracing::error!("failed to send command error event: bridge closed");
                            }
                        }
                    }
                }
            }
        }

        // Poll events from the plugin
        let events = plugin.poll_events();
        let event_count = events.len();
        for event in events {
            tracing::debug!(event = %event, "plugin thread forwarding event");
            if handle.event_tx.send(event).is_err() {
                // UI side dropped — shut down
                tracing::error!("plugin event channel closed unexpectedly; bridge receiver dropped");
                tracing::info!(reason = "bridge closed", "plugin thread shutting down");
                let _ = plugin.cleanup();
                return;
            }
        }

        tracing::trace!(commands_processed = queue_depth, events_forwarded = event_count, "plugin poll loop tick");
        // Sleep briefly to avoid busy-spin (plugin thread is not latency-critical)
        std::thread::sleep(std::time::Duration::from_millis(16)); // ~60Hz poll
    }
}
