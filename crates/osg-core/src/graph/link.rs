// Link — desired connection between two endpoints.

use serde::{Deserialize, Serialize};

use super::effects_config::EffectsConfig;
use super::endpoint::{EndpointDescriptor, default_one};
use super::eq_config::EqConfig;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub start: EndpointDescriptor,
    pub end: EndpointDescriptor,
    pub state: LinkState,
    /// Per-route volume ratio (0.0–1.0). Independent of channel master volume.
    #[serde(default = "default_one")]
    pub cell_volume: f32,
    /// Per-route left channel volume (0.0–1.0). Equals `cell_volume` when mono.
    #[serde(default = "default_one")]
    pub cell_volume_left: f32,
    /// Per-route right channel volume (0.0–1.0). Equals `cell_volume` when mono.
    #[serde(default = "default_one")]
    pub cell_volume_right: f32,
    /// Per-route parametric EQ configuration.
    #[serde(default)]
    pub cell_eq: EqConfig,
    /// Per-route effects chain configuration.
    #[serde(default)]
    pub cell_effects: EffectsConfig,
    /// PipeWire node ID of the cell's volume-control node (null-audio-sink).
    /// Each cell gets its own PW node so volume can be controlled per-route.
    /// Route: channel → cell_node (volume here) → mix
    /// Transient: populated at runtime by reconciliation, stripped before persistence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell_node_id: Option<u32>,
    /// Transient: a link command is in-flight to PipeWire.
    #[serde(skip)]
    pub pending: bool,
}

/// The desired link state. There is no "DisconnectedUnlocked" variant;
/// a link in that state is simply absent from the state vec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LinkState {
    /// Some but not all matching node-port pairs are connected.
    PartiallyConnected,
    /// Fully connected, but OSG will not restore it if something else breaks it.
    ConnectedUnlocked,
    /// Fully connected; OSG will re-create any missing sub-links.
    ConnectedLocked,
    /// OSG will actively remove any sub-links between these endpoints.
    DisconnectedLocked,
}

impl LinkState {
    pub fn is_locked(self) -> bool {
        matches!(self, Self::ConnectedLocked | Self::DisconnectedLocked)
    }

    pub fn is_connected(self) -> Option<bool> {
        match self {
            Self::PartiallyConnected => None,
            Self::ConnectedUnlocked | Self::ConnectedLocked => Some(true),
            Self::DisconnectedLocked => Some(false),
        }
    }
}
