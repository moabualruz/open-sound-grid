// Adapted from Sonusmix (MPL-2.0) — https://codeberg.org/sonusmix/sonusmix
//
// The graph module defines the desired-state model: the user's intent for
// how audio should be routed. The `pw` module holds the *actual* PipeWire
// graph; reconciliation (in `crate::routing::reconcile`) bridges the two.

pub mod serde_helpers;
pub mod types;

// Re-export the most commonly used items at module level.
pub use types::{
    App, AppId, Channel, ChannelId, ChannelKind, Device, DeviceId, Endpoint, EndpointDescriptor,
    Link, LinkState, MixerSession, PersistentNodeId, ReconcileSettings, VolumeLockMuteState,
    aggregate_bools, average_volumes, volumes_mixed,
};
