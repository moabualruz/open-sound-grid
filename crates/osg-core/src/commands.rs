//! Wire-format commands received from the frontend via WebSocket.
//!
//! These are JSON-serializable and map to `routing::StateMsg` for processing.
//! The frontend sends these as `{"type": "createChannel", ...}` JSON messages.

use serde::{Deserialize, Serialize};

use crate::graph::{ChannelId, ChannelKind, EndpointDescriptor};
use crate::routing::StateMsg;

/// A command received from the frontend over WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Command {
    /// Create a new virtual audio channel (PipeWire null-audio-sink).
    CreateChannel { name: String, kind: ChannelKind },

    /// Remove an endpoint (channel, app, or node).
    RemoveEndpoint { endpoint: EndpointDescriptor },

    /// Set volume on an endpoint (0.0 to 1.0).
    SetVolume {
        endpoint: EndpointDescriptor,
        volume: f32,
    },

    /// Set independent L/R stereo volume on an endpoint.
    SetStereoVolume {
        endpoint: EndpointDescriptor,
        left: f32,
        right: f32,
    },

    /// Set mute state on an endpoint.
    SetMute {
        endpoint: EndpointDescriptor,
        muted: bool,
    },

    /// Lock/unlock volume on an endpoint.
    SetVolumeLocked {
        endpoint: EndpointDescriptor,
        locked: bool,
    },

    /// Rename an endpoint. `None` resets to default display name.
    RenameEndpoint {
        endpoint: EndpointDescriptor,
        name: Option<String>,
    },

    /// Create a link (route) between two endpoints.
    Link {
        source: EndpointDescriptor,
        target: EndpointDescriptor,
    },

    /// Remove a link (route) between two endpoints.
    RemoveLink {
        source: EndpointDescriptor,
        target: EndpointDescriptor,
    },

    /// Lock/unlock a link.
    SetLinkLocked {
        source: EndpointDescriptor,
        target: EndpointDescriptor,
        locked: bool,
    },

    /// Assign an output device to a mix. `None` clears the assignment.
    SetMixOutput {
        channel: ChannelId,
        output_node_id: Option<u32>,
    },

    /// Toggle endpoint visibility (hide/show instead of delete).
    SetEndpointVisible {
        endpoint: EndpointDescriptor,
        visible: bool,
    },

    /// Set per-route cell volume (0.0–1.0, independent of channel master).
    SetLinkVolume {
        source: EndpointDescriptor,
        target: EndpointDescriptor,
        volume: f32,
    },

    /// Set the display order for endpoints.
    SetDisplayOrder { order: Vec<EndpointDescriptor> },
}

impl Command {
    /// Convert a wire-format command to an internal StateMsg.
    pub fn into_state_msg(self) -> StateMsg {
        match self {
            Self::CreateChannel { name, kind } => StateMsg::AddChannel(name, kind),
            Self::RemoveEndpoint { endpoint } => StateMsg::RemoveEndpoint(endpoint),
            Self::SetVolume { endpoint, volume } => StateMsg::SetVolume(endpoint, volume),
            Self::SetStereoVolume {
                endpoint,
                left,
                right,
            } => StateMsg::SetStereoVolume(endpoint, left, right),
            Self::SetMute { endpoint, muted } => StateMsg::SetMute(endpoint, muted),
            Self::SetVolumeLocked { endpoint, locked } => {
                StateMsg::SetVolumeLocked(endpoint, locked)
            }
            Self::RenameEndpoint { endpoint, name } => StateMsg::RenameEndpoint(endpoint, name),
            Self::Link { source, target } => StateMsg::Link(source, target),
            Self::RemoveLink { source, target } => StateMsg::RemoveLink(source, target),
            Self::SetLinkLocked {
                source,
                target,
                locked,
            } => StateMsg::SetLinkLocked(source, target, locked),
            Self::SetLinkVolume {
                source,
                target,
                volume,
            } => StateMsg::SetLinkVolume(source, target, volume),
            Self::SetMixOutput {
                channel,
                output_node_id,
            } => StateMsg::SetMixOutput(channel, output_node_id),
            Self::SetEndpointVisible { endpoint, visible } => {
                StateMsg::SetEndpointVisible(endpoint, visible)
            }
            Self::SetDisplayOrder { order } => StateMsg::SetDisplayOrder(order),
        }
    }
}
