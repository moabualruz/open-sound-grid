// EndpointDescriptor and Endpoint types.

use serde::{Deserialize, Serialize};

use super::effects_config::EffectsConfig;
use super::eq_config::EqConfig;
use super::identifiers::{AppId, ChannelId, DeviceId, PersistentNodeId};
use super::port_kind::PortKind;
use super::volume_state::VolumeLockMuteState;

// ---------------------------------------------------------------------------
// EndpointDescriptor — the "address" of any routable entity
// ---------------------------------------------------------------------------

/// Describes anything that can have audio routed to or from it.
/// This might be a single node, a virtual group, or all sources/sinks
/// belonging to an application or device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EndpointDescriptor {
    /// A single node identified only by its PipeWire ID.
    /// Cannot survive PipeWire restarts.
    EphemeralNode(u32, PortKind),
    /// A single node that can be identified by name/path and
    /// re-matched after PipeWire restarts.
    PersistentNode(PersistentNodeId, PortKind),
    /// A virtual node created and managed by OSG.
    Channel(ChannelId),
    /// All sources or sinks (minus explicit exceptions) for an application.
    App(AppId, PortKind),
    /// All sources or sinks for a device.
    Device(DeviceId, PortKind),
}

impl EndpointDescriptor {
    /// Whether this endpoint can carry traffic of the given `kind`.
    /// Channels are bidirectional and match both.
    pub fn is_kind(&self, kind: PortKind) -> bool {
        match self {
            Self::Channel(_) => true,
            Self::EphemeralNode(_, k)
            | Self::PersistentNode(_, k)
            | Self::App(_, k)
            | Self::Device(_, k) => *k == kind,
        }
    }

    /// True when the endpoint appears in the source/sink list of the given kind.
    pub fn is_list(&self, kind: PortKind) -> bool {
        match self {
            Self::Channel(_) => false,
            Self::EphemeralNode(_, k)
            | Self::PersistentNode(_, k)
            | Self::App(_, k)
            | Self::Device(_, k) => *k == kind,
        }
    }

    /// True for single-node endpoints (ephemeral, persistent, channel).
    pub fn is_single(&self) -> bool {
        matches!(
            self,
            Self::EphemeralNode(..) | Self::PersistentNode(..) | Self::Channel(_)
        )
    }
}

// ---------------------------------------------------------------------------
// Endpoint — the state blob we track for each routable entity
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoint {
    pub descriptor: EndpointDescriptor,
    pub is_placeholder: bool,
    pub display_name: String,
    pub custom_name: Option<String>,
    pub icon_name: String,
    pub details: Vec<String>,
    pub volume: f32,
    /// Left channel volume (0.0–1.0). Equals `volume` when mono.
    #[serde(default = "default_one")]
    pub volume_left: f32,
    /// Right channel volume (0.0–1.0). Equals `volume` when mono.
    #[serde(default = "default_one")]
    pub volume_right: f32,
    /// True when channels across the backing nodes have differing volumes.
    pub volume_mixed: bool,
    pub volume_locked_muted: VolumeLockMuteState,
    /// Hidden endpoints are preserved but not shown in the UI's active lists.
    #[serde(default = "default_true")]
    pub visible: bool,
    /// Disabled endpoints are muted and excluded from routing but retained in
    /// state. Persists across sessions.
    #[serde(default)]
    pub disabled: bool,
    /// Parametric EQ configuration for this endpoint.
    #[serde(default)]
    pub eq: EqConfig,
    /// Effects chain configuration for this endpoint.
    #[serde(default)]
    pub effects: EffectsConfig,
}

fn default_true() -> bool {
    true
}

pub(super) fn default_one() -> f32 {
    1.0
}

impl Endpoint {
    pub fn new(descriptor: EndpointDescriptor) -> Self {
        Self {
            descriptor,
            is_placeholder: false,
            display_name: String::new(),
            custom_name: None,
            icon_name: String::new(),
            details: Vec::new(),
            volume: 1.0,
            volume_left: 1.0,
            volume_right: 1.0,
            volume_mixed: false,
            volume_locked_muted: VolumeLockMuteState::UnmutedUnlocked,
            visible: true,
            disabled: false,
            eq: EqConfig::default(),
            effects: EffectsConfig::default(),
        }
    }

    pub fn with_display_name(mut self, name: String) -> Self {
        self.display_name = name;
        self
    }

    pub fn with_icon_name(mut self, name: String) -> Self {
        self.icon_name = name;
        self
    }

    pub fn with_details(mut self, details: Vec<String>) -> Self {
        self.details = details;
        self
    }

    /// Maximum volume value (150% or +3.5 dB).
    pub const MAX_VOLUME: f32 = 1.5;

    pub fn with_volume(mut self, volume: f32, mixed: bool) -> Self {
        self.volume = volume.clamp(0.0, Self::MAX_VOLUME);
        self.volume_mixed = mixed;
        self
    }

    /// Set the mono volume, clamped to [0.0, 1.5].
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, Self::MAX_VOLUME);
    }

    /// Set stereo volume, clamped to [0.0, 1.5].
    pub fn set_stereo_volume(&mut self, left: f32, right: f32) {
        self.volume_left = left.clamp(0.0, Self::MAX_VOLUME);
        self.volume_right = right.clamp(0.0, Self::MAX_VOLUME);
        self.volume = (self.volume_left + self.volume_right) / 2.0;
    }

    pub fn with_mute_unlocked(mut self, muted: bool) -> Self {
        self.volume_locked_muted = if muted {
            VolumeLockMuteState::MutedUnlocked
        } else {
            VolumeLockMuteState::UnmutedUnlocked
        };
        self
    }

    pub fn custom_or_display_name(&self) -> &str {
        self.custom_name.as_ref().unwrap_or(&self.display_name)
    }

    pub fn details_short(&self) -> String {
        self.details.first().cloned().unwrap_or_default()
    }

    pub fn details_long(&self) -> String {
        self.details.join("\n\n")
    }

    pub fn new_test(descriptor: EndpointDescriptor) -> Self {
        Self::new(descriptor)
            .with_display_name("TESTING_ENDPOINT".to_owned())
            .with_icon_name("applications-development".to_owned())
    }
}
