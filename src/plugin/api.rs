//! Plugin Command/Event protocol.
//!
//! Core sends `PluginCommand`s to the active plugin.
//! Plugin responds with `PluginResponse` and emits `PluginEvent`s asynchronously.
//! No shared state — all communication is through these types.

use std::collections::HashMap;
use std::fmt;

// --- IDs (opaque to plugins, assigned by plugin impl) ---

/// Unique identifier for a software channel within the plugin.
pub type ChannelId = u32;
/// Unique identifier for an output mix within the plugin.
pub type MixId = u32;
/// Unique identifier for a hardware output device.
pub type OutputId = u32;
/// Unique identifier for a running audio application.
pub type AppId = u32;

/// A source can be a hardware input or a software channel.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceId {
    Hardware(u32),
    Channel(ChannelId),
}

// --- Data types ---

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct HardwareInput {
    pub id: u32,
    pub name: String,
    pub description: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct HardwareOutput {
    pub id: OutputId,
    pub name: String,
    pub description: String,
    /// Backend-specific sink/device identifier.
    pub device_id: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AudioApplication {
    pub id: AppId,
    pub name: String,
    pub binary: String,
    pub icon_name: Option<String>,
    /// Backend-specific stream index.
    pub stream_index: u32,
    pub channel: Option<ChannelId>,
}

#[derive(Debug, Clone)]
pub struct RouteState {
    pub volume: f32, // 0.0 - 1.0
    pub enabled: bool,
    pub muted: bool,
}

impl Default for RouteState {
    fn default() -> Self {
        Self {
            volume: 1.0,
            enabled: true,
            muted: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChannelInfo {
    pub id: ChannelId,
    pub name: String,
    pub apps: Vec<AppId>,
    pub muted: bool,
}

#[derive(Debug, Clone)]
pub struct MixInfo {
    pub id: MixId,
    pub name: String,
    pub output: Option<OutputId>,
    pub master_volume: f32,
    pub muted: bool,
}

/// Full snapshot of the mixer state from the plugin's perspective.
#[derive(Debug, Clone, Default)]
pub struct MixerSnapshot {
    pub channels: Vec<ChannelInfo>,
    pub mixes: Vec<MixInfo>,
    pub routes: HashMap<(SourceId, MixId), RouteState>,
    pub hardware_inputs: Vec<HardwareInput>,
    pub hardware_outputs: Vec<HardwareOutput>,
    pub applications: Vec<AudioApplication>,
    pub peak_levels: HashMap<SourceId, f32>,
}

// --- Commands (Core → Plugin) ---

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum PluginCommand {
    /// Get a full snapshot of current state.
    GetState,
    /// List hardware audio inputs.
    ListHardwareInputs,
    /// List hardware audio outputs.
    ListHardwareOutputs,
    /// List running audio applications.
    ListApplications,
    /// Create a new software channel.
    CreateChannel { name: String },
    /// Remove a software channel.
    RemoveChannel { id: ChannelId },
    /// Create a new output mix.
    CreateMix { name: String },
    /// Remove an output mix.
    RemoveMix { id: MixId },
    /// Set volume for a source in a specific mix.
    SetRouteVolume { source: SourceId, mix: MixId, volume: f32 },
    /// Enable/disable a source in a mix.
    SetRouteEnabled { source: SourceId, mix: MixId, enabled: bool },
    /// Mute/unmute a source in a specific mix.
    SetRouteMuted { source: SourceId, mix: MixId, muted: bool },
    /// Route an application to a channel.
    RouteApp { app: AppId, channel: ChannelId },
    /// Unroute an application from its channel.
    UnrouteApp { app: AppId },
    /// Set the hardware output for a mix.
    SetMixOutput { mix: MixId, output: OutputId },
    /// Set master volume for a mix.
    SetMixMasterVolume { mix: MixId, volume: f32 },
    /// Mute/unmute an entire mix.
    SetMixMuted { mix: MixId, muted: bool },
    /// Mute/unmute a source across all mixes.
    SetSourceMuted { source: SourceId, muted: bool },
}

impl fmt::Display for PluginCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            PluginCommand::GetState => "GetState",
            PluginCommand::ListHardwareInputs => "ListHardwareInputs",
            PluginCommand::ListHardwareOutputs => "ListHardwareOutputs",
            PluginCommand::ListApplications => "ListApplications",
            PluginCommand::CreateChannel { .. } => "CreateChannel",
            PluginCommand::RemoveChannel { .. } => "RemoveChannel",
            PluginCommand::CreateMix { .. } => "CreateMix",
            PluginCommand::RemoveMix { .. } => "RemoveMix",
            PluginCommand::SetRouteVolume { .. } => "SetRouteVolume",
            PluginCommand::SetRouteEnabled { .. } => "SetRouteEnabled",
            PluginCommand::SetRouteMuted { .. } => "SetRouteMuted",
            PluginCommand::RouteApp { .. } => "RouteApp",
            PluginCommand::UnrouteApp { .. } => "UnrouteApp",
            PluginCommand::SetMixOutput { .. } => "SetMixOutput",
            PluginCommand::SetMixMasterVolume { .. } => "SetMixMasterVolume",
            PluginCommand::SetMixMuted { .. } => "SetMixMuted",
            PluginCommand::SetSourceMuted { .. } => "SetSourceMuted",
        };
        f.write_str(name)
    }
}

// --- Responses (Plugin → Core, synchronous) ---

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum PluginResponse {
    Ok,
    State(MixerSnapshot),
    ChannelCreated { id: ChannelId },
    MixCreated { id: MixId },
    HardwareInputs(Vec<HardwareInput>),
    HardwareOutputs(Vec<HardwareOutput>),
    Applications(Vec<AudioApplication>),
    Error(String),
}

// --- Events (Plugin → Core, asynchronous) ---

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum PluginEvent {
    /// Full state snapshot from the plugin.
    StateRefreshed(MixerSnapshot),
    /// Devices were added, removed, or changed.
    DevicesChanged,
    /// Running audio applications changed.
    ApplicationsChanged(Vec<AudioApplication>),
    /// Updated peak levels for VU meters.
    PeakLevels(HashMap<SourceId, f32>),
    /// Plugin encountered an error.
    Error(String),
    /// Lost connection to audio server.
    ConnectionLost,
    /// Reconnected to audio server.
    ConnectionRestored,
}

impl fmt::Display for PluginEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            PluginEvent::StateRefreshed(_) => "StateRefreshed",
            PluginEvent::DevicesChanged => "DevicesChanged",
            PluginEvent::ApplicationsChanged(_) => "ApplicationsChanged",
            PluginEvent::PeakLevels(_) => "PeakLevels",
            PluginEvent::Error(_) => "Error",
            PluginEvent::ConnectionLost => "ConnectionLost",
            PluginEvent::ConnectionRestored => "ConnectionRestored",
        };
        f.write_str(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_command_display() {
        let cmd = PluginCommand::GetState;
        assert_eq!(format!("{cmd}"), "GetState");
        let cmd2 = PluginCommand::CreateChannel { name: "Music".into() };
        assert_eq!(format!("{cmd2}"), "CreateChannel");
    }

    #[test]
    fn test_plugin_event_display() {
        let evt = PluginEvent::DevicesChanged;
        assert_eq!(format!("{evt}"), "DevicesChanged");
    }

    #[test]
    fn test_route_state_default() {
        let rs = RouteState::default();
        assert_eq!(rs.volume, 1.0);
        assert!(rs.enabled);
        assert!(!rs.muted);
    }
}
