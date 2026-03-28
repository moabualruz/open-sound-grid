//! Mixer engine — owns mixer state and delegates to the active plugin.
//!
//! The engine is the single source of truth for the mixer's state.
//! It sends `PluginCommand`s and processes `PluginEvent`s.
//! The UI reads state from the engine and sends user actions to it.

pub mod state;

pub use state::*;

use tokio::sync::mpsc;

use crate::plugin::api::{MixerSnapshot, PluginCommand, PluginEvent};
use crate::plugin::manager::PluginBridge;

/// The mixer engine sits between the UI and the plugin.
pub struct MixerEngine {
    pub state: MixerState,
    command_tx: Option<mpsc::UnboundedSender<PluginCommand>>,
}

impl MixerEngine {
    pub fn new() -> Self {
        Self {
            state: MixerState::default(),
            command_tx: None,
        }
    }

    /// Attach a plugin bridge. Returns the event receiver for use in subscriptions.
    pub fn attach(&mut self, bridge: PluginBridge) -> mpsc::UnboundedReceiver<PluginEvent> {
        tracing::info!(
            command_tx_capacity = "unbounded",
            "plugin bridge attached"
        );
        self.command_tx = Some(bridge.command_tx);
        // Request initial state
        self.send_command(PluginCommand::GetState);
        bridge.event_rx
    }

    /// Send a command to the plugin.
    pub fn send_command(&self, cmd: PluginCommand) {
        tracing::debug!(command = ?cmd, "sending plugin command");
        if let Some(tx) = &self.command_tx {
            if tx.send(cmd).is_err() {
                tracing::warn!("plugin bridge disconnected; command dropped");
            }
        } else {
            tracing::warn!("send_command called with no bridge attached");
        }
    }

    /// Apply a snapshot from the plugin to the engine state.
    pub fn apply_snapshot(&mut self, snapshot: MixerSnapshot) {
        tracing::debug!(
            channels = snapshot.channels.len(),
            mixes = snapshot.mixes.len(),
            routes = snapshot.routes.len(),
            hardware_inputs = snapshot.hardware_inputs.len(),
            hardware_outputs = snapshot.hardware_outputs.len(),
            applications = snapshot.applications.len(),
            peaks = snapshot.peak_levels.len(),
            "applying snapshot to engine"
        );
        self.state.apply_snapshot(snapshot);
    }

    /// Check if plugin is connected.
    /// Uses state.connected which is set true by apply_snapshot and false by ConnectionLost.
    pub fn is_connected(&self) -> bool {
        let connected = self.command_tx.is_some() && self.state.connected;
        tracing::trace!(connected, has_bridge = self.command_tx.is_some(), state_connected = self.state.connected, "is_connected check");
        connected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_not_connected_by_default() {
        let engine = MixerEngine::new();
        assert!(!engine.is_connected());
    }

    #[test]
    fn test_send_command_without_bridge_does_not_panic() {
        let engine = MixerEngine::new();
        engine.send_command(PluginCommand::GetState);
        // Should not panic — just logs a warning
    }
}
