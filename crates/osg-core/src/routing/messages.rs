// Adapted from Sonusmix (MPL-2.0) — https://codeberg.org/sonusmix/sonusmix
//
// Messages flowing through the routing layer.

use crate::graph::{AppId, ChannelId, ChannelKind, EndpointDescriptor};
use crate::pw::PortKind;

// ---------------------------------------------------------------------------
// Inbound: UI / API -> Reducer
// ---------------------------------------------------------------------------

/// Commands that mutate the desired state.
#[derive(Debug, Clone)]
pub enum StateMsg {
    AddEphemeralNode(u32, PortKind),
    AddApp(AppId, PortKind),
    AddChannel(String, ChannelKind),
    RemoveEndpoint(EndpointDescriptor),
    SetVolume(EndpointDescriptor, f32),
    SetStereoVolume(EndpointDescriptor, f32, f32),
    SetMute(EndpointDescriptor, bool),
    SetVolumeLocked(EndpointDescriptor, bool),
    /// `None` resets to the default display name.
    RenameEndpoint(EndpointDescriptor, Option<String>),
    ChangeChannelKind(ChannelId, ChannelKind),
    /// Assign an output device node to a mix channel.
    SetMixOutput(ChannelId, Option<u32>),
    /// Toggle endpoint visibility (hide/show instead of delete).
    SetEndpointVisible(EndpointDescriptor, bool),
    /// Set the display order for source channels (rows).
    SetChannelOrder(Vec<EndpointDescriptor>),
    /// Set the display order for sink mixes (columns).
    SetMixOrder(Vec<EndpointDescriptor>),
    /// Update the OS default output node (from PipeWire metadata).
    SetDefaultOutputNode(Option<u32>),
    Link(EndpointDescriptor, EndpointDescriptor),
    RemoveLink(EndpointDescriptor, EndpointDescriptor),
    SetLinkLocked(EndpointDescriptor, EndpointDescriptor, bool),
    /// Set per-route cell volume (independent of channel master).
    SetLinkVolume(EndpointDescriptor, EndpointDescriptor, f32),
    /// Set per-route stereo cell volume (independent L/R).
    SetLinkStereoVolume(EndpointDescriptor, EndpointDescriptor, f32, f32),
}

// ---------------------------------------------------------------------------
// Outbound: Reducer -> UI / subscribers
// ---------------------------------------------------------------------------

/// Notifications emitted after a state mutation.
#[derive(Debug, Clone)]
pub enum StateOutputMsg {
    EndpointAdded(EndpointDescriptor),
    EndpointRemoved(EndpointDescriptor),
}

// ---------------------------------------------------------------------------
// Internal: Reducer thread messages
// ---------------------------------------------------------------------------

use crate::pw::AudioGraph;

/// Messages consumed by the reducer's internal event loop.
#[derive(Debug)]
pub enum ReducerMsg {
    Update(StateMsg),
    GraphUpdate(Box<AudioGraph>),
    SettingsChanged,
    Save {
        clear_state: bool,
        clear_settings: bool,
    },
    SaveAndExit,
}
