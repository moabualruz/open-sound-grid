// MixerSession (aggregate root) and ReconcileSettings.

use std::collections::HashMap;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::pw::{NodeIdentifier, PortKind};

use super::channel::App;
use super::channel::{Channel, Device};
use super::endpoint::{Endpoint, EndpointDescriptor};
use super::identifiers::{AppId, ChannelId, DeviceId, PersistentNodeId};
use super::link::Link;

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
    /// ULID of the current OSG instance. Stamped on all PW nodes for
    /// ownership tracking. Stale nodes from a crashed previous instance
    /// are reaped on startup.
    #[serde(skip)]
    pub instance_id: ulid::Ulid,
    /// PW node ID of the staging sink (vol=0, for glitch-free rerouting).
    #[serde(skip)]
    pub staging_node_id: Option<u32>,
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
