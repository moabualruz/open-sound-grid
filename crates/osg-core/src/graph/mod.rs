// Adapted from Sonusmix (MPL-2.0) — https://codeberg.org/sonusmix/sonusmix
//
// The graph module defines the desired-state model: the user's intent for
// how audio should be routed. The `pw` module holds the *actual* PipeWire
// graph; reconciliation (in `crate::routing::reconcile`) bridges the two.

pub mod channel;
pub mod effects_config;
pub mod endpoint;
pub mod eq_config;
pub mod identifiers;
pub mod link;
pub mod serde_helpers;
pub mod session;
pub mod types;
pub mod utils;
pub mod volume_state;

// Re-export all domain types at module level for ergonomic imports.
pub use channel::{App, AppAssignment, Channel, ChannelKind, Device, SourceType};
pub use effects_config::{
    CompressorConfig, DeEsserConfig, EffectsConfig, GateConfig, LimiterConfig, SmartVolumeConfig,
    SpatialAudioConfig,
};
pub use endpoint::{Endpoint, EndpointDescriptor};
pub use eq_config::{EqBand, EqConfig, FilterType};
pub use identifiers::{AppId, ChannelId, DeviceId, PersistentNodeId};
pub use link::{Link, LinkState};
pub use session::{MixerSession, ReconcileSettings};
pub use utils::{aggregate_bools, average_volumes, volumes_mixed};
pub use volume_state::VolumeLockMuteState;
