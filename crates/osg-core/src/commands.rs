//! Wire-format commands received from the frontend.
//!
//! These are JSON-serializable and map to `routing::StateMsg` for processing.
//! The frontend sends these as `{"type": "createChannel", ...}` JSON messages.

use serde::{Deserialize, Serialize};

use crate::graph::{
    AppAssignment, ChannelId, ChannelKind, EffectsConfig, EndpointDescriptor, EqConfig,
};
use crate::routing::StateMsg;

/// Wire-format command received from the frontend.
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
        #[serde(rename = "outputNodeId")]
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

    /// Set per-route stereo cell volume (independent L/R).
    SetLinkStereoVolume {
        source: EndpointDescriptor,
        target: EndpointDescriptor,
        left: f32,
        right: f32,
    },

    /// Set the display order for source channels (rows).
    SetChannelOrder { order: Vec<EndpointDescriptor> },

    /// Set the display order for sink mixes (columns).
    SetMixOrder { order: Vec<EndpointDescriptor> },

    /// Assign an app to a channel. Redirects the app's PW streams to the channel's virtual sink.
    AssignApp {
        channel: ChannelId,
        #[serde(rename = "applicationName")]
        application_name: String,
        #[serde(rename = "binaryName")]
        binary_name: String,
    },

    /// Unassign an app from a channel. Returns the app's streams to the default sink.
    UnassignApp {
        channel: ChannelId,
        #[serde(rename = "applicationName")]
        application_name: String,
        #[serde(rename = "binaryName")]
        binary_name: String,
    },

    /// Set parametric EQ configuration for an endpoint (channel or mix).
    SetEq {
        endpoint: EndpointDescriptor,
        eq: EqConfig,
    },

    /// Set parametric EQ configuration for a per-route cell.
    SetCellEq {
        source: EndpointDescriptor,
        target: EndpointDescriptor,
        eq: EqConfig,
    },

    /// Set effects chain configuration for an endpoint (channel or mix).
    SetEffects {
        endpoint: EndpointDescriptor,
        effects: EffectsConfig,
    },

    /// Set effects chain configuration for a per-route cell.
    SetCellEffects {
        source: EndpointDescriptor,
        target: EndpointDescriptor,
        effects: EffectsConfig,
    },

    /// Dismiss the first-launch welcome wizard (persists welcome_dismissed = true).
    DismissWelcome,

    /// Disable/enable an endpoint (persisted; disabled endpoints are muted and
    /// excluded from routing but retained in state).
    SetEndpointDisabled {
        endpoint: EndpointDescriptor,
        disabled: bool,
    },

    /// Undo the last destructive operation.
    Undo,

    /// Redo the last undone operation.
    Redo,
}

impl Command {
    /// Convert a wire-format command to an internal StateMsg.
    #[allow(clippy::too_many_lines)]
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
            Self::SetLinkStereoVolume {
                source,
                target,
                left,
                right,
            } => StateMsg::SetLinkStereoVolume(source, target, left, right),
            Self::SetMixOutput {
                channel,
                output_node_id,
            } => StateMsg::SetMixOutput(channel, output_node_id),
            Self::SetEndpointVisible { endpoint, visible } => {
                StateMsg::SetEndpointVisible(endpoint, visible)
            }
            Self::SetChannelOrder { order } => StateMsg::SetChannelOrder(order),
            Self::SetMixOrder { order } => StateMsg::SetMixOrder(order),
            Self::AssignApp {
                channel,
                application_name,
                binary_name,
            } => StateMsg::AssignApp(
                channel,
                AppAssignment {
                    application_name,
                    binary_name,
                },
            ),
            Self::UnassignApp {
                channel,
                application_name,
                binary_name,
            } => StateMsg::UnassignApp(
                channel,
                AppAssignment {
                    application_name,
                    binary_name,
                },
            ),
            Self::SetEq { endpoint, eq } => StateMsg::SetEq(endpoint, eq),
            Self::SetCellEq { source, target, eq } => StateMsg::SetCellEq(source, target, eq),
            Self::SetEffects { endpoint, effects } => StateMsg::SetEffects(endpoint, effects),
            Self::SetCellEffects {
                source,
                target,
                effects,
            } => StateMsg::SetCellEffects(source, target, effects),
            Self::DismissWelcome => StateMsg::DismissWelcome,
            Self::SetEndpointDisabled { endpoint, disabled } => {
                StateMsg::SetEndpointDisabled(endpoint, disabled)
            }
            Self::Undo => StateMsg::Undo,
            Self::Redo => StateMsg::Redo,
        }
    }
}
