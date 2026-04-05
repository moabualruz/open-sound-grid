// Adapted from Sonusmix (MPL-2.0) — https://codeberg.org/sonusmix/sonusmix
//
// The graph module defines the desired-state model: the user's intent for
// how audio should be routed. The `pw` module holds the *actual* PipeWire
// graph; reconciliation (in `crate::routing::reconcile`) bridges the two.

pub mod channel;
pub mod effects_config;
pub mod endpoint;
pub mod eq_config;
pub mod events;
pub mod identifiers;
pub mod link;
pub mod node_identity;
pub mod port_kind;
pub mod runtime;
pub mod serde_helpers;
pub mod session;
pub mod types;
pub mod undo;
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
pub use events::MixerEvent;
pub use identifiers::{AppId, ChannelId, DeviceId, PersistentNodeId};
pub use link::LinkKey;
pub use link::{Link, LinkState};
pub use node_identity::NodeIdentity;
pub use port_kind::PortKind;
pub use runtime::RuntimeState;
pub use session::{MixerSession, ReconcileSettings};
pub use utils::{aggregate_bools, average_volumes, volumes_mixed};
pub use volume_state::VolumeLockMuteState;
