use std::collections::HashMap;

use ulid::Ulid;

use super::{Client, Device, GroupNode, Link, Node, Port};

/// Read-only projection of PipeWire's current graph state. DDD read model.
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioGraph {
    pub group_nodes: HashMap<Ulid, GroupNode>,
    pub clients: HashMap<u32, Client>,
    pub devices: HashMap<u32, Device>,
    pub nodes: HashMap<u32, Node>,
    pub ports: HashMap<u32, Port>,
    pub links: HashMap<u32, Link>,
    /// The PipeWire node name of the OS default audio sink (from metadata).
    pub default_sink_name: Option<String>,
    /// The PipeWire node name of the OS default audio source/mic (from metadata).
    pub default_source_name: Option<String>,
    /// Map (channel_node_id, mix_node_id) → cell PW node ID for per-route volume.
    #[serde(skip)]
    pub cell_node_ids: HashMap<(String, String), u32>,
    /// PW node ID of the staging sink (vol=0, for glitch-free rerouting).
    #[serde(skip)]
    pub staging_node_id: Option<u32>,
}
