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
    SetMute(EndpointDescriptor, bool),
    SetVolumeLocked(EndpointDescriptor, bool),
    /// `None` resets to the default display name.
    RenameEndpoint(EndpointDescriptor, Option<String>),
    ChangeChannelKind(ChannelId, ChannelKind),
    Link(EndpointDescriptor, EndpointDescriptor),
    RemoveLink(EndpointDescriptor, EndpointDescriptor),
    SetLinkLocked(EndpointDescriptor, EndpointDescriptor, bool),
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
