// MixerSession (aggregate root) and ReconcileSettings.

use std::collections::HashMap;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::channel::{App, Channel, Device};
use super::endpoint::{Endpoint, EndpointDescriptor};
use super::identifiers::{AppId, ChannelId, DeviceId, PersistentNodeId};
use super::link::Link;
use super::node_identity::NodeIdentity;
use super::port_kind::PortKind;

/// The user's desired audio state. Aggregate root (DDD write model).
/// PipeWire: no direct equivalent — this is our domain model.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MixerSession {
    /// Whether the user has dismissed the first-launch welcome wizard.
    #[serde(default)]
    pub welcome_dismissed: bool,
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
    #[serde(default)]
    pub links: Vec<Link>,
    /// Mapping from persistent node IDs to their identifiers.
    #[serde(default)]
    pub persistent_nodes: HashMap<PersistentNodeId, (NodeIdentity, PortKind)>,
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
