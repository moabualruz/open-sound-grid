// Adapted from Sonusmix (MPL-2.0) — https://codeberg.org/sonusmix/sonusmix
//
// Domain types for the desired-state graph layer. These describe what
// the user *wants* the PipeWire graph to look like; the reconciliation
// loop in `crate::routing::reconcile` compares them against reality.

use std::collections::HashMap;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::pw::{NodeIdentifier, PortKind};

/// The kind of virtual audio bus. Domain alias for pw::ChannelKind.
pub type ChannelKind = crate::pw::GroupNodeKind;

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

/// Opaque ID for a persistent (name-matched) node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistentNodeId(Ulid);

#[allow(clippy::new_without_default)]
impl PersistentNodeId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }

    pub fn inner(&self) -> Ulid {
        self.0
    }
}

/// Unique ID for a Channel. PipeWire: GroupNode ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelId(Ulid);

#[allow(clippy::new_without_default)]
impl ChannelId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }

    pub fn inner(&self) -> Ulid {
        self.0
    }
}

/// Unique ID for a detected audio app. PipeWire: Client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppId(Ulid);

#[allow(clippy::new_without_default)]
impl AppId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

/// Opaque ID for a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceId(Ulid);

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
    /// True when channels across the backing nodes have differing volumes.
    pub volume_mixed: bool,
    pub volume_locked_muted: VolumeLockMuteState,
    /// Hidden endpoints are preserved but not shown in the UI's active lists.
    #[serde(default = "default_true")]
    pub visible: bool,
    /// Transient flag: a volume/mute command is in-flight to PipeWire.
    #[serde(skip)]
    pub volume_pending: bool,
}

fn default_true() -> bool {
    true
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
            volume_mixed: false,
            volume_locked_muted: VolumeLockMuteState::UnmutedUnlocked,
            visible: true,
            volume_pending: false,
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

    pub fn with_volume(mut self, volume: f32, mixed: bool) -> Self {
        self.volume = volume;
        self.volume_mixed = mixed;
        self
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

// ---------------------------------------------------------------------------
// Link — desired connection between two endpoints
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub start: EndpointDescriptor,
    pub end: EndpointDescriptor,
    pub state: LinkState,
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

// ---------------------------------------------------------------------------
// Volume lock / mute state machine
// ---------------------------------------------------------------------------

/// Encodes the combined lock + mute state for an endpoint.
///
/// When an endpoint backs multiple PipeWire nodes, some may be muted while
/// others are not ("MuteMixed"). A user cannot input this state and cannot
/// lock volume while in it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VolumeLockMuteState {
    MuteMixed,
    MutedLocked,
    MutedUnlocked,
    UnmutedLocked,
    #[default]
    UnmutedUnlocked,
}

impl VolumeLockMuteState {
    pub fn is_locked(self) -> bool {
        matches!(self, Self::MutedLocked | Self::UnmutedLocked)
    }

    pub fn is_muted(self) -> Option<bool> {
        match self {
            Self::MuteMixed => None,
            Self::MutedLocked | Self::MutedUnlocked => Some(true),
            Self::UnmutedLocked | Self::UnmutedUnlocked => Some(false),
        }
    }

    pub fn with_mute(self, muted: bool) -> Self {
        match (muted, self) {
            (true, Self::MutedLocked | Self::UnmutedLocked) => Self::MutedLocked,
            (true, Self::MuteMixed | Self::MutedUnlocked | Self::UnmutedUnlocked) => {
                Self::MutedUnlocked
            }
            (false, Self::MutedLocked | Self::UnmutedLocked) => Self::UnmutedLocked,
            (false, Self::MuteMixed | Self::MutedUnlocked | Self::UnmutedUnlocked) => {
                Self::UnmutedUnlocked
            }
        }
    }

    pub fn lock(self) -> Option<Self> {
        match self {
            Self::MuteMixed => None,
            Self::MutedLocked | Self::MutedUnlocked => Some(Self::MutedLocked),
            Self::UnmutedLocked | Self::UnmutedUnlocked => Some(Self::UnmutedLocked),
        }
    }

    pub fn unlock(self) -> Self {
        match self {
            Self::MuteMixed => Self::MuteMixed,
            Self::MutedLocked | Self::MutedUnlocked => Self::MutedUnlocked,
            Self::UnmutedLocked | Self::UnmutedUnlocked => Self::UnmutedUnlocked,
        }
    }

    /// Build the state from multiple node mute booleans (unlocked).
    pub fn from_bools_unlocked<'a>(bools: impl IntoIterator<Item = &'a bool>) -> Self {
        match aggregate_bools(bools) {
            Some(true) => Self::MutedUnlocked,
            Some(false) => Self::UnmutedUnlocked,
            None => Self::MuteMixed,
        }
    }
}

// ---------------------------------------------------------------------------
// Channel — user-created virtual audio bus
// ---------------------------------------------------------------------------

/// User-created virtual audio bus. PipeWire: null-audio-sink / GroupNode.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub id: ChannelId,
    pub kind: ChannelKind,
    /// For sink channels (mixes): the assigned output device node ID.
    /// `None` means no output assigned. Monitor uses OS default.
    pub output_node_id: Option<u32>,
    /// PipeWire ID once the node is created; `None` while pending.
    #[serde(skip)]
    pub pipewire_id: Option<u32>,
    /// True while a create request is in-flight.
    #[serde(skip)]
    pub pending: bool,
}

// ---------------------------------------------------------------------------
// App — running application emitting audio
// ---------------------------------------------------------------------------

/// Running application emitting audio. PipeWire: Client/Stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct App {
    pub id: AppId,
    pub kind: PortKind,
    #[serde(skip)]
    pub is_active: bool,
    pub name: String,
    pub binary: String,
    pub icon_name: String,
    pub exceptions: Vec<EndpointDescriptor>,
}

impl App {
    pub fn new_inactive(
        application_name: String,
        binary: String,
        icon_name: String,
        kind: PortKind,
    ) -> Self {
        Self {
            id: AppId::new(),
            kind,
            is_active: false,
            name: application_name,
            binary,
            icon_name,
            exceptions: Vec::new(),
        }
    }

    pub fn matches(&self, identifier: &NodeIdentifier, kind: PortKind) -> bool {
        self.kind == kind
            && identifier.application_name.as_ref() == Some(&self.name)
            && identifier.binary_name.as_ref() == Some(&self.binary)
    }

    pub fn name_with_tag(&self) -> String {
        format!("[App] {}", self.name)
    }
}

// ---------------------------------------------------------------------------
// Device — placeholder for future use
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device;

// ---------------------------------------------------------------------------
// MixerSession — the full user-desired graph
// ---------------------------------------------------------------------------

/// The user's desired audio state. Aggregate root (DDD write model).
/// PipeWire: no direct equivalent — this is our domain model.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MixerSession {
    pub active_sources: Vec<EndpointDescriptor>,
    pub active_sinks: Vec<EndpointDescriptor>,
    #[serde(
        serialize_with = "crate::graph::serde_helpers::serialize_map_as_vec",
        deserialize_with = "crate::graph::serde_helpers::deserialize_map_from_vec"
    )]
    pub endpoints: HashMap<EndpointDescriptor, Endpoint>,
    #[serde(skip)]
    pub candidates: Vec<(u32, PortKind, NodeIdentifier)>,
    pub links: Vec<Link>,
    /// Mapping from persistent node IDs to their identifiers.
    pub persistent_nodes: HashMap<PersistentNodeId, (NodeIdentifier, PortKind)>,
    pub apps: HashMap<AppId, App>,
    pub devices: HashMap<DeviceId, Device>,
    pub channels: IndexMap<ChannelId, Channel>,
    /// User-defined display order for endpoints (channel/mix ordering).
    #[serde(default)]
    pub display_order: Vec<EndpointDescriptor>,
    /// PipeWire node ID of the OS default audio sink.
    /// Updated from PipeWire metadata `default.audio.sink`.
    #[serde(skip)]
    pub default_output_node_id: Option<u32>,
}

// ---------------------------------------------------------------------------
// Settings that influence reconciliation behavior
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileSettings {
    pub lock_endpoint_connections: bool,
    pub lock_group_node_connections: bool,
    pub app_sources_include_monitors: bool,
    pub volume_limit: f64,
}

impl Default for ReconcileSettings {
    fn default() -> Self {
        Self {
            lock_endpoint_connections: false,
            lock_group_node_connections: true,
            app_sources_include_monitors: false,
            volume_limit: 100.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Cubic-root weighted average of volume values (perceptual curve).
pub fn average_volumes<'a>(volumes: impl IntoIterator<Item = &'a f32>) -> f32 {
    let mut count: usize = 0;
    let mut total = 0.0;
    for volume in volumes {
        count += 1;
        total += volume.powf(1.0 / 3.0);
    }
    (total / count.max(1) as f32).powf(3.0)
}

/// True when not all channel volumes are the same.
pub fn volumes_mixed<'a>(volumes: impl IntoIterator<Item = &'a f32>) -> bool {
    let mut iterator = volumes.into_iter();
    let Some(first) = iterator.next() else {
        return false;
    };
    // NOTE: Original Sonusmix logic returns `all(|x| x == first)` — which is
    // true when volumes are NOT mixed. We preserve the original behavior here
    // (the caller in diff_properties assigns the return value to `volume_mixed`
    // just like Sonusmix does).
    iterator.all(|x| x == first)
}

/// `Some(val)` when all booleans agree, `None` when they differ.
pub fn aggregate_bools<'a>(bools: impl IntoIterator<Item = &'a bool>) -> Option<bool> {
    let mut iter = bools.into_iter();
    let first = iter.next()?;
    iter.all(|b| b == first).then_some(*first)
}
