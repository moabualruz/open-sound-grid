use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

// --- ID types ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MixId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OutputId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AppId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceId {
    Hardware(u32),
    Channel(ChannelId),
}

impl fmt::Display for ChannelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ch:{}", self.0)
    }
}

impl fmt::Display for MixId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "mix:{}", self.0)
    }
}

// --- Data structs ---

#[derive(Debug, Clone)]
pub struct HardwareInput {
    pub id: u32,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct HardwareOutput {
    pub id: OutputId,
    pub name: String,
    pub description: String,
    pub pa_sink_name: String,
}

#[derive(Debug, Clone)]
pub struct AudioApplication {
    pub id: AppId,
    pub name: String,
    pub binary: String,
    pub icon_name: Option<String>,
    pub sink_input_index: u32,
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
pub struct ChannelState {
    pub id: ChannelId,
    pub name: String,
    pub apps: Vec<AppId>,
    pub muted: bool,
}

#[derive(Debug, Clone)]
pub struct MixState {
    pub id: MixId,
    pub name: String,
    pub icon: String,
    pub color: [u8; 3],
    pub output: Option<OutputId>,
    pub master_volume: f32,
    pub muted: bool,
}

/// Full mixer state snapshot
#[derive(Debug, Clone, Default)]
pub struct MixerState {
    pub channels: Vec<ChannelState>,
    pub mixes: Vec<MixState>,
    pub routes: HashMap<(SourceId, MixId), RouteState>,
    pub hardware_inputs: Vec<HardwareInput>,
    pub hardware_outputs: Vec<HardwareOutput>,
    pub applications: Vec<AudioApplication>,
    pub peak_levels: HashMap<SourceId, f32>,
}

// --- Events from backend → UI ---

#[derive(Debug, Clone)]
pub enum AudioEvent {
    /// Mixer state fully refreshed
    StateRefreshed(MixerState),
    /// An application appeared or disappeared
    ApplicationsChanged(Vec<AudioApplication>),
    /// Peak levels updated
    PeakLevels(HashMap<SourceId, f32>),
    /// A device was added/removed
    DevicesChanged,
    /// Backend error
    Error(String),
}

// --- Commands from UI → backend ---

#[derive(Debug, Clone)]
pub enum BackendCommand {
    CreateChannel { name: String },
    RemoveChannel { id: ChannelId },
    CreateMix { name: String },
    RemoveMix { id: MixId },
    SetRouteVolume { source: SourceId, mix: MixId, volume: f32 },
    SetRouteEnabled { source: SourceId, mix: MixId, enabled: bool },
    RouteAppToChannel { app: AppId, channel: ChannelId },
    SetMixOutput { mix: MixId, output: OutputId },
    SetMixMasterVolume { mix: MixId, volume: f32 },
    SetMixMuted { mix: MixId, muted: bool },
    SetSourceMuted { source: SourceId, muted: bool },
    RefreshState,
    Shutdown,
}
