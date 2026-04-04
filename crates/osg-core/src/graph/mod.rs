// Adapted from Sonusmix (MPL-2.0) — https://codeberg.org/sonusmix/sonusmix
//
// The graph module defines the desired-state model: the user's intent for
// how audio should be routed. The `pw` module holds the *actual* PipeWire
// graph; reconciliation (in `crate::routing::reconcile`) bridges the two.

pub mod effects_config;
pub mod serde_helpers;
pub mod types;

// Re-export the most commonly used items at module level.
pub use effects_config::{
    CompressorConfig, DeEsserConfig, EffectsConfig, GateConfig, LimiterConfig, SmartVolumeConfig,
    SpatialAudioConfig,
};
pub use types::{
    App, AppAssignment, AppId, Channel, ChannelId, ChannelKind, Device, DeviceId, Endpoint,
    EndpointDescriptor, EqBand, EqConfig, FilterType, Link, LinkState, MixerSession,
    PersistentNodeId, ReconcileSettings, SourceType, VolumeLockMuteState, aggregate_bools,
    average_volumes, volumes_mixed,
};
