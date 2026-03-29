//! Node and link management for the PipeWire backend.
//!
//! Creates virtual null-audio-sink nodes (one per channel, one per output mix)
//! and the routing links that connect them. Node and link proxies are owned
//! here for their full lifetime; destroying a proxy removes the remote object.
//!
//! # Object lifecycle
//! - `create_virtual_sink` → allocates a remote `support.null-audio-sink` node.
//! - `create_link` → wires two nodes together via a `link-factory` link.
//! - `remove_node` / `remove_link` → destroys the remote objects via the core.
//! - `remove_all` → bulk teardown at plugin shutdown.

#[cfg(feature = "pipewire-backend")]
use pipewire as pw;
#[cfg(feature = "pipewire-backend")]
use pipewire::proxy::ProxyT;

#[cfg(feature = "pipewire-backend")]
use std::collections::HashMap;

#[cfg(feature = "pipewire-backend")]
use crate::error::{OsgError, Result};

/// Manages PipeWire virtual sink nodes and routing links.
///
/// Tracks proxies by the application-level IDs assigned by the plugin
/// (channel_id, mix_id) so that the plugin layer never needs to reason about
/// raw PipeWire object IDs directly.
#[cfg(feature = "pipewire-backend")]
pub struct PwNodeManager {
    /// channel_id → Node proxy for the channel's virtual sink.
    channel_nodes: HashMap<u32, pw::node::Node>,
    /// mix_id → Node proxy for the mix's virtual sink.
    mix_nodes: HashMap<u32, pw::node::Node>,
    /// (source_node_id, mix_id) → Link proxy for a channel-to-mix routing connection.
    route_links: HashMap<(u32, u32), pw::link::Link>,
    /// mix_id → Link proxy for a mix-to-output connection.
    output_links: HashMap<u32, pw::link::Link>,
}

#[cfg(feature = "pipewire-backend")]
impl PwNodeManager {
    /// Create an empty node manager. Call [`create_virtual_sink`] to populate.
    pub fn new() -> Self {
        tracing::trace!("PwNodeManager::new");
        Self {
            channel_nodes: HashMap::new(),
            mix_nodes: HashMap::new(),
            route_links: HashMap::new(),
            output_links: HashMap::new(),
        }
    }

    /// Create a `support.null-audio-sink` virtual sink node via `core`.
    ///
    /// The returned proxy is stored internally; the u32 is the remote object id
    /// that can be used as `link.output.node` / `link.input.node` when wiring.
    ///
    /// The caller must register the returned id with [`register_channel`] or
    /// [`register_mix`] to associate it with an application-level id.
    ///
    /// # PipeWire factory
    /// - Factory: `"support.null-audio-sink"` (always present in PipeWire)
    /// - `media.class` = `"Audio/Sink"`
    /// - `audio.position` = `"[FL FR]"` (stereo)
    /// - `object.linger` = `"1"` so the node survives proxy destruction until
    ///   we explicitly call `destroy_object`.
    #[tracing::instrument(skip(self, core))]
    pub fn create_virtual_sink(
        &mut self,
        core: &pw::core::CoreRc,
        name: &str,
        description: &str,
    ) -> Result<u32> {
        tracing::debug!(name, description, "creating PW virtual null-audio-sink");

        let props = pw::properties::properties! {
            "factory.name"    => "support.null-audio-sink",
            "node.name"       => name,
            "node.description"=> description,
            "media.class"     => "Audio/Sink",
            "audio.position"  => "[FL FR]",
            "object.linger"   => "1",
        };

        let node = core
            .create_object::<pw::node::Node>("adapter", &props)
            .map_err(|e| {
                OsgError::PulseAudio(format!(
                    "PW create_object failed for virtual sink '{name}': {e}"
                ))
            })?;

        // The remote object id is available immediately via the proxy.
        let node_id = node.upcast_ref().id();

        tracing::info!(name, node_id, "PW virtual null-audio-sink created");
        Ok(node_id)
    }

    /// Wire two PipeWire nodes together using the `link-factory`.
    ///
    /// `output_node` and `input_node` are remote PipeWire object ids, as
    /// returned by [`create_virtual_sink`]. PipeWire will auto-connect matching
    /// ports; set `object.linger` so the link persists even if the proxy drops.
    ///
    /// Returns the remote link object id.
    #[tracing::instrument(skip(self, core))]
    pub fn create_link(
        &mut self,
        core: &pw::core::CoreRc,
        output_node: u32,
        input_node: u32,
    ) -> Result<u32> {
        tracing::debug!(output_node, input_node, "creating PW link");

        let props = pw::properties::properties! {
            "link.output.node" => output_node.to_string().as_str(),
            "link.input.node"  => input_node.to_string().as_str(),
            "object.linger"    => "1",
        };

        let link = core
            .create_object::<pw::link::Link>("link-factory", &props)
            .map_err(|e| {
                OsgError::PulseAudio(format!(
                    "PW create_object failed for link {output_node}→{input_node}: {e}"
                ))
            })?;

        let link_id = link.upcast_ref().id();

        tracing::info!(output_node, input_node, link_id, "PW link created");
        Ok(link_id)
    }

    /// Store a channel node proxy under `channel_id`.
    ///
    /// This must be called after [`create_virtual_sink`] to associate the
    /// returned proxy with the application-level channel id.
    pub fn register_channel_node(&mut self, channel_id: u32, node: pw::node::Node) {
        tracing::trace!(channel_id, "registering channel node proxy");
        self.channel_nodes.insert(channel_id, node);
    }

    /// Store a mix node proxy under `mix_id`.
    pub fn register_mix_node(&mut self, mix_id: u32, node: pw::node::Node) {
        tracing::trace!(mix_id, "registering mix node proxy");
        self.mix_nodes.insert(mix_id, node);
    }

    /// Store a channel→mix routing link proxy.
    pub fn register_route_link(&mut self, source_node_id: u32, mix_id: u32, link: pw::link::Link) {
        tracing::trace!(source_node_id, mix_id, "registering route link proxy");
        self.route_links.insert((source_node_id, mix_id), link);
    }

    /// Store a mix→output link proxy.
    pub fn register_output_link(&mut self, mix_id: u32, link: pw::link::Link) {
        tracing::trace!(mix_id, "registering output link proxy");
        self.output_links.insert(mix_id, link);
    }

    /// Retrieve the remote object id for a channel's virtual sink node.
    ///
    /// Returns `None` if no node has been registered for `channel_id`.
    pub fn channel_node_id(&self, channel_id: u32) -> Option<u32> {
        self.channel_nodes
            .get(&channel_id)
            .map(|n| n.upcast_ref().id())
    }

    /// Retrieve the remote object id for a mix's virtual sink node.
    ///
    /// Returns `None` if no node has been registered for `mix_id`.
    pub fn mix_node_id(&self, mix_id: u32) -> Option<u32> {
        self.mix_nodes.get(&mix_id).map(|n| n.upcast_ref().id())
    }

    /// Remove and destroy the channel node for `channel_id`.
    ///
    /// Dropping the proxy is sufficient when `object.linger` is `"0"`.
    /// Because we set `object.linger = "1"` during creation, the caller should
    /// call `core.destroy_object(proxy)` before this if immediate remote
    /// destruction is required. For plugin shutdown, simply dropping (via
    /// `remove_all`) is the normal path.
    #[tracing::instrument(skip(self))]
    pub fn remove_channel_node(&mut self, channel_id: u32) {
        if self.channel_nodes.remove(&channel_id).is_some() {
            tracing::debug!(channel_id, "removed channel node proxy");
        } else {
            tracing::warn!(channel_id, "remove_channel_node: no proxy found");
        }
    }

    /// Remove and destroy the mix node for `mix_id`.
    #[tracing::instrument(skip(self))]
    pub fn remove_mix_node(&mut self, mix_id: u32) {
        if self.mix_nodes.remove(&mix_id).is_some() {
            tracing::debug!(mix_id, "removed mix node proxy");
        } else {
            tracing::warn!(mix_id, "remove_mix_node: no proxy found");
        }
    }

    /// Remove the routing link between `source_node_id` and `mix_id`.
    #[tracing::instrument(skip(self))]
    pub fn remove_route_link(&mut self, source_node_id: u32, mix_id: u32) {
        if self.route_links.remove(&(source_node_id, mix_id)).is_some() {
            tracing::debug!(source_node_id, mix_id, "removed route link proxy");
        } else {
            tracing::warn!(source_node_id, mix_id, "remove_route_link: no proxy found");
        }
    }

    /// Remove the output link for `mix_id`.
    #[tracing::instrument(skip(self))]
    pub fn remove_output_link(&mut self, mix_id: u32) {
        if self.output_links.remove(&mix_id).is_some() {
            tracing::debug!(mix_id, "removed output link proxy");
        } else {
            tracing::warn!(mix_id, "remove_output_link: no proxy found");
        }
    }

    /// Drop all node and link proxies, triggering remote object cleanup.
    ///
    /// Called at plugin shutdown. With `object.linger = "1"`, the remote
    /// objects survive until explicitly destroyed; without it, dropping the
    /// proxy is sufficient. Either way, this clears all local tracking state.
    #[tracing::instrument(skip(self))]
    pub fn remove_all(&mut self) {
        tracing::info!(
            channel_nodes = self.channel_nodes.len(),
            mix_nodes = self.mix_nodes.len(),
            route_links = self.route_links.len(),
            output_links = self.output_links.len(),
            "removing all PW node and link proxies"
        );
        self.output_links.clear();
        self.route_links.clear();
        self.mix_nodes.clear();
        self.channel_nodes.clear();
        tracing::debug!("all PW proxies dropped");
    }
}

#[cfg(feature = "pipewire-backend")]
impl Default for PwNodeManager {
    fn default() -> Self {
        Self::new()
    }
}
