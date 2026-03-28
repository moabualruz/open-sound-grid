//! Plugin manager — discovers, initializes, and bridges plugins to the UI.
//!
//! The plugin thread is fully event-driven: it blocks on a unified channel
//! and wakes only when a UI command or PA subscribe event arrives. No timers,
//! no polling, no sleep loops.

use std::sync::mpsc as std_mpsc;

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

/// Messages the plugin thread can receive from any source.
pub enum PluginThreadMsg {
    /// A command from the UI.
    Command(PluginCommand),
    /// A PA subscribe event from the background reader thread.
    PaEvent(PaSubscribeKind),
}

/// Categories of PA subscribe events.
#[derive(Debug, Clone, Copy)]
pub enum PaSubscribeKind {
    /// sink-input new/remove/change → app list may have changed
    SinkInput,
    /// sink new/remove/change → device list may have changed
    Sink,
    /// source new/remove/change → device list may have changed
    Source,
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
    ///
    /// The plugin thread blocks on a unified `std_mpsc::Receiver<PluginThreadMsg>`
    /// and wakes only when a command or PA event arrives — no timers.
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

        // UI ↔ plugin bridge (tokio channels for async iced subscription)
        let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<PluginCommand>();
        let (evt_tx, evt_rx) = mpsc::unbounded_channel();

        // Unified blocking channel for the plugin thread
        let (unified_tx, unified_rx) = std_mpsc::channel::<PluginThreadMsg>();

        // Forward UI commands into the unified channel via a bridging thread
        let cmd_unified_tx = unified_tx.clone();
        std::thread::Builder::new()
            .name("osg-cmd-bridge".into())
            .spawn(move || {
                // Block on the tokio receiver using a mini runtime
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("cmd-bridge runtime");
                rt.block_on(async move {
                    while let Some(cmd) = cmd_rx.recv().await {
                        if cmd_unified_tx.send(PluginThreadMsg::Command(cmd)).is_err() {
                            break;
                        }
                    }
                    tracing::debug!("command bridge thread exiting");
                });
            })
            .map_err(|e| {
                tracing::error!(error = %e, "failed to spawn command bridge thread");
                crate::error::OsgError::PulseAudio(format!("Thread spawn: {e}"))
            })?;

        self.plugin_info = Some(info);

        // Spawn the plugin thread — receives the unified_rx and pa_event sender
        let pa_event_tx = unified_tx;
        std::thread::Builder::new()
            .name("osg-plugin".into())
            .spawn(move || {
                run_plugin_thread(&mut *plugin, unified_rx, pa_event_tx, evt_tx);
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

/// Plugin thread main loop — fully event-driven, no polling.
///
/// Blocks on `unified_rx.recv()` and wakes only when:
/// - A UI command arrives (forwarded by the cmd-bridge thread)
/// - A PA subscribe event arrives (sent by the pactl subscribe reader thread)
fn run_plugin_thread(
    plugin: &mut dyn AudioPlugin,
    unified_rx: std_mpsc::Receiver<PluginThreadMsg>,
    pa_event_tx: std_mpsc::Sender<PluginThreadMsg>,
    event_tx: mpsc::UnboundedSender<PluginEvent>,
) {
    let _span = tracing::info_span!("plugin_thread").entered();
    tracing::info!("plugin thread started (event-driven, no polling)");

    // Initialize plugin — this spawns `pactl subscribe` internally
    if let Err(e) = plugin.init() {
        tracing::error!(error = %e, "plugin init failed");
        let _ = event_tx.send(PluginEvent::Error(format!("Init failed: {e}")));
        return;
    }
    tracing::info!("plugin initialized successfully");

    // Give the plugin access to the unified channel so its background threads
    // (e.g., pactl subscribe) can push events directly — zero latency, no polling.
    plugin.set_event_sender(pa_event_tx);

    // Main event loop — blocks until a message arrives
    loop {
        let msg = match unified_rx.recv() {
            Ok(msg) => msg,
            Err(_) => {
                tracing::info!("unified channel closed — plugin thread shutting down");
                let _ = plugin.cleanup();
                return;
            }
        };

        match msg {
            PluginThreadMsg::Command(cmd) => {
                tracing::debug!(command = %cmd, "plugin thread received command");
                match cmd {
                    PluginCommand::GetState => {
                        match plugin.handle_command(PluginCommand::GetState) {
                            Ok(PluginResponse::State(snapshot)) => {
                                let _ = event_tx.send(PluginEvent::StateRefreshed(snapshot));
                            }
                            Ok(_) => {}
                            Err(e) => {
                                tracing::warn!(error = %e, "GetState command failed");
                                let _ = event_tx.send(PluginEvent::Error(format!("GetState: {e}")));
                            }
                        }
                    }
                    other => {
                        match plugin.handle_command(other) {
                            Ok(_) => {
                                // Mutation succeeded — refresh state so the UI updates
                                match plugin.handle_command(PluginCommand::GetState) {
                                    Ok(PluginResponse::State(snapshot)) => {
                                        let _ = event_tx.send(PluginEvent::StateRefreshed(snapshot));
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        tracing::warn!(error = %e, "post-mutation GetState failed");
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "command produced error");
                                let _ = event_tx.send(PluginEvent::Error(format!("{e}")));
                            }
                        }
                    }
                }
            }

            PluginThreadMsg::PaEvent(kind) => {
                tracing::debug!(kind = ?kind, "PA subscribe event received");
                match kind {
                    PaSubscribeKind::SinkInput => {
                        // App list may have changed — emit ApplicationsChanged first so
                        // AppResolver runs before StateRefreshed triggers a UI redraw.
                        match plugin.handle_command(PluginCommand::GetState) {
                            Ok(PluginResponse::State(snapshot)) => {
                                let apps = snapshot.applications.clone();
                                tracing::debug!(app_count = apps.len(), "emitting ApplicationsChanged from PA subscribe");
                                let _ = event_tx.send(PluginEvent::ApplicationsChanged(apps));
                                let _ = event_tx.send(PluginEvent::StateRefreshed(snapshot));
                            }
                            _ => {}
                        }
                    }
                    PaSubscribeKind::Sink | PaSubscribeKind::Source => {
                        // Device list may have changed — refresh full state
                        let _ = event_tx.send(PluginEvent::DevicesChanged);
                        match plugin.handle_command(PluginCommand::GetState) {
                            Ok(PluginResponse::State(snapshot)) => {
                                let _ = event_tx.send(PluginEvent::StateRefreshed(snapshot));
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // Check if bridge is still alive
        if event_tx.is_closed() {
            tracing::info!(reason = "bridge closed", "plugin thread shutting down");
            let _ = plugin.cleanup();
            return;
        }
    }
}
