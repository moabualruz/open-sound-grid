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
// EQ — parametric equalizer configuration
// ---------------------------------------------------------------------------

/// Biquad filter type for a single EQ band.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FilterType {
    Peaking,
    LowShelf,
    HighShelf,
    LowPass,
    HighPass,
    Notch,
}

/// A single parametric EQ band.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EqBand {
    pub enabled: bool,
    pub filter_type: FilterType,
    /// Center frequency in Hz (20–20 000).
    pub frequency: f32,
    /// Gain in dB (±12).
    pub gain: f32,
    /// Quality factor (0.1–10).
    pub q: f32,
}

impl Default for EqBand {
    fn default() -> Self {
        Self {
            enabled: true,
            filter_type: FilterType::Peaking,
            frequency: 1000.0,
            gain: 0.0,
            q: 0.707,
        }
    }
}

/// Full EQ configuration: enable toggle + ordered list of bands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EqConfig {
    pub enabled: bool,
    pub bands: Vec<EqBand>,
}

impl Default for EqConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bands: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Effects — compressor / gate / de-esser / limiter configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressorConfig {
    pub enabled: bool,
    /// Threshold in dBFS (e.g., -18.0).
    pub threshold: f32,
    /// Compression ratio (e.g., 3.0 for 3:1).
    pub ratio: f32,
    /// Attack time in milliseconds (converted to seconds for DSP).
    pub attack: f32,
    /// Release time in milliseconds.
    pub release: f32,
    /// Make-up gain in dB.
    pub makeup: f32,
}

impl Default for CompressorConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: -18.0,
            ratio: 3.0,
            attack: 8.0,
            release: 150.0,
            makeup: 4.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GateConfig {
    pub enabled: bool,
    /// Threshold in dBFS (e.g., -45.0).
    pub threshold: f32,
    /// Hold time in milliseconds.
    pub hold: f32,
    /// Attack time in milliseconds.
    pub attack: f32,
    /// Release time in milliseconds.
    pub release: f32,
}

impl Default for GateConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: -45.0,
            hold: 150.0,
            attack: 1.0,
            release: 50.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeEsserConfig {
    pub enabled: bool,
    /// Center frequency in Hz (5000–8000).
    pub frequency: f32,
    /// Sidechain threshold in dBFS.
    pub threshold: f32,
    /// Maximum gain reduction in dB (positive, e.g., 6.0).
    pub reduction: f32,
}

impl Default for DeEsserConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            frequency: 6000.0,
            threshold: -20.0,
            reduction: 6.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LimiterConfig {
    pub enabled: bool,
    /// Output ceiling in dBFS (e.g., -1.0).
    pub ceiling: f32,
    /// Release time in milliseconds.
    pub release: f32,
}

impl Default for LimiterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            ceiling: -1.0,
            release: 50.0,
        }
    }
}

/// Full effects chain configuration for a filter node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EffectsConfig {
    pub compressor: CompressorConfig,
    pub gate: GateConfig,
    pub de_esser: DeEsserConfig,
    pub limiter: LimiterConfig,
    /// Volume boost in dB (0–12). Applied as linear gain after limiter.
    #[serde(default)]
    pub boost: f32,
}

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

/// Opaque ID for a persistent (name-matched) node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
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
#[serde(transparent)]
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
#[serde(transparent)]
pub struct AppId(Ulid);

#[allow(clippy::new_without_default)]
impl AppId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

/// Opaque ID for a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
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
    /// Left channel volume (0.0–1.0). Equals `volume` when mono.
    #[serde(default = "default_one")]
    pub volume_left: f32,
    /// Right channel volume (0.0–1.0). Equals `volume` when mono.
    #[serde(default = "default_one")]
    pub volume_right: f32,
    /// True when channels across the backing nodes have differing volumes.
    pub volume_mixed: bool,
    pub volume_locked_muted: VolumeLockMuteState,
    /// Hidden endpoints are preserved but not shown in the UI's active lists.
    #[serde(default = "default_true")]
    pub visible: bool,
    /// Parametric EQ configuration for this endpoint.
    #[serde(default)]
    pub eq: EqConfig,
    /// Effects chain configuration for this endpoint.
    #[serde(default)]
    pub effects: EffectsConfig,
    /// Transient flag: a volume/mute command is in-flight to PipeWire.
    #[serde(skip)]
    pub volume_pending: bool,
    /// Cached volume before mute (restored on unmute). Null-audio-sinks don't
    /// honor SPA_PROP_mute, so mute is implemented as volume → 0.
    #[serde(skip)]
    pub pre_mute_volume: Option<(f32, f32)>,
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
            volume_left: 1.0,
            volume_right: 1.0,
            volume_mixed: false,
            volume_locked_muted: VolumeLockMuteState::UnmutedUnlocked,
            visible: true,
            eq: EqConfig::default(),
            effects: EffectsConfig::default(),
            volume_pending: false,
            pre_mute_volume: None,
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

fn default_one() -> f32 {
    1.0
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
// AppAssignment — persistent app-to-channel binding
// ---------------------------------------------------------------------------

/// Identifies an application for routing. Matched against PipeWire node properties
/// `application.name` and `application.process.binary`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppAssignment {
    pub application_name: String,
    pub binary_name: String,
}

// ---------------------------------------------------------------------------
// Source type — detected from PipeWire node properties
// ---------------------------------------------------------------------------

/// Classifies the audio source type for contextual EQ/effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceType {
    /// Real ALSA microphone (device.api=alsa, form-factor=microphone/headset/webcam).
    HardwareMic,
    /// ALSA line-in (not a microphone).
    HardwareLineIn,
    /// Virtual source (EasyEffects, loopback).
    VirtualSource,
    /// App playback stream (browser, game, music).
    #[default]
    AppStream,
}

// ---------------------------------------------------------------------------
// Channel — user-created virtual audio bus
// ---------------------------------------------------------------------------

/// Virtual audio bus — either user-created or auto-created for an app.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub id: ChannelId,
    pub kind: ChannelKind,
    /// Detected source type — determines which EQ presets and effects are shown.
    #[serde(default)]
    pub source_type: SourceType,
    /// For sink channels (mixes): the assigned output device node ID.
    pub output_node_id: Option<u32>,
    /// Apps assigned to this channel. Their PW streams are redirected here.
    #[serde(default)]
    pub assigned_apps: Vec<AppAssignment>,
    /// True if this channel was auto-created for a single app.
    /// Auto-channels: no grouping, no manual app assignment from UI,
    /// dissolved when the app is assigned to a user channel.
    #[serde(default)]
    pub auto_app: bool,
    /// False for input device channels and EasyEffects channels —
    /// prevents users from assigning apps to them.
    #[serde(default = "default_true")]
    pub allow_app_assignment: bool,
    /// PipeWire ID — only used for Sink (mix) channels. Source channels
    /// are logical-only and do not have a PW node (ADR-007).
    #[serde(skip)]
    pub pipewire_id: Option<u32>,
    /// True while a create request is in-flight (mixes only).
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
    #[serde(default)]
    pub active_sources: Vec<EndpointDescriptor>,
    #[serde(default)]
    pub active_sinks: Vec<EndpointDescriptor>,
    #[serde(
        default,
        serialize_with = "crate::graph::serde_helpers::serialize_map_as_vec",
        deserialize_with = "crate::graph::serde_helpers::deserialize_map_from_vec"
    )]
    pub endpoints: HashMap<EndpointDescriptor, Endpoint>,
    #[serde(skip)]
    pub candidates: Vec<(u32, PortKind, NodeIdentifier)>,
    #[serde(default)]
    pub links: Vec<Link>,
    /// Mapping from persistent node IDs to their identifiers.
    #[serde(default)]
    pub persistent_nodes: HashMap<PersistentNodeId, (NodeIdentifier, PortKind)>,
    #[serde(default)]
    pub apps: HashMap<AppId, App>,
    #[serde(default)]
    pub devices: HashMap<DeviceId, Device>,
    #[serde(default)]
    pub channels: IndexMap<ChannelId, Channel>,
    /// User-defined display order for source channels (rows).
    #[serde(default)]
    pub channel_order: Vec<EndpointDescriptor>,
    /// User-defined display order for sink mixes (columns).
    #[serde(default)]
    pub mix_order: Vec<EndpointDescriptor>,
    /// PipeWire node ID of the OS default audio sink.
    /// Updated from PipeWire metadata `default.audio.sink`.
    #[serde(skip)]
    pub default_output_node_id: Option<u32>,
    /// Tracks which cell nodes have been created (by "osg.cell.{ch_pw_id}.{mix_pw_id}" name).
    /// Prevents duplicate creation in the reconciliation loop.
    #[serde(skip)]
    pub created_cells: std::collections::HashSet<String>,
    /// Monotonically increasing counter bumped on every `update()` call.
    /// The reducer uses this to skip reconciliation when only graph events
    /// arrive but the desired state hasn't changed.
    #[serde(skip)]
    pub generation: u64,
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
