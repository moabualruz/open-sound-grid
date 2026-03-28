//! Mixer engine — owns mixer state and delegates to the active plugin.
//!
//! The engine is the single source of truth for the mixer's state.
//! It sends `PluginCommand`s and processes `PluginEvent`s.
//! The UI reads state from the engine and sends user actions to it.

pub mod state;

pub use state::*;

use crate::plugin::api::{MixerSnapshot, PluginCommand};
use crate::plugin::manager::PluginBridge;

/// The mixer engine sits between the UI and the plugin.
pub struct MixerEngine {
    pub state: MixerState,
    bridge: Option<PluginBridge>,
}

impl MixerEngine {
    pub fn new() -> Self {
        Self {
            state: MixerState::default(),
            bridge: None,
        }
    }

    /// Attach a plugin bridge (called after PluginManager::start).
    pub fn attach(&mut self, bridge: PluginBridge) {
        tracing::info!("plugin bridge attached");
        self.bridge = Some(bridge);
        // Request initial state
        self.send_command(PluginCommand::GetState);
    }

    /// Send a command to the plugin.
    pub fn send_command(&self, cmd: PluginCommand) {
        tracing::debug!(command = ?cmd, "sending plugin command");
        if let Some(bridge) = &self.bridge {
            if bridge.command_tx.send(cmd).is_err() {
                tracing::error!("Plugin bridge disconnected");
            }
        } else {
            tracing::warn!("send_command called with no bridge attached");
        }
    }

    /// Apply a snapshot from the plugin to the engine state.
    pub fn apply_snapshot(&mut self, snapshot: MixerSnapshot) {
        tracing::info!(
            channels = snapshot.channels.len(),
            mixes = snapshot.mixes.len(),
            routes = snapshot.routes.len(),
            "applying snapshot to engine"
        );
        self.state.apply_snapshot(snapshot);
    }

    /// Check if plugin is connected.
    pub fn is_connected(&self) -> bool {
        self.bridge.is_some()
    }
}
