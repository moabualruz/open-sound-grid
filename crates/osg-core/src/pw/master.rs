// Master — handles PipeWire registry/core events and node/link operations.
// Adapted from Sonusmix (MPL-2.0) — https://codeberg.org/sonusmix/sonusmix

use std::{cell::RefCell, rc::Rc};

use pipewire::{
    core::CoreRc, keys::*, metadata::Metadata, properties::properties, proxy::ProxyT,
    registry::RegistryRc,
};
use tracing::{debug, warn};
use ulid::Ulid;

use super::{
    GroupNodeKind, OSG_APP_ID, OSG_APP_NAME, PortKind, PwError, ToPipewireMessage, map_ports,
    object::Port, store::Store,
};

/// # Master
///
/// The Master handles events which then get inserted into the store.
/// Therefore, the store is the slave of the master, processing what
/// it gets.
pub(super) struct Master {
    pub(super) store: Rc<RefCell<Store>>,
    pub(super) pw_core: CoreRc,
    pub(super) registry: RegistryRc,
    pub(super) sender: pipewire::channel::Sender<ToPipewireMessage>,
    /// PipeWire "default" metadata proxy — used to set default.configured.audio.sink.
    pub(super) settings_metadata: Rc<RefCell<Option<Metadata>>>,
    /// Metadata listeners that must be kept alive to receive property change events.
    /// Without this, listeners are dropped and events stop firing.
    pub(super) metadata_listeners: Rc<RefCell<Vec<pipewire::metadata::MetadataListener>>>,
}

impl Master {
    pub(super) fn new(
        store: Rc<RefCell<Store>>,
        pw_core: CoreRc,
        registry: RegistryRc,
        sender: pipewire::channel::Sender<ToPipewireMessage>,
    ) -> Self {
        Master {
            store,
            pw_core,
            registry,
            sender,
            settings_metadata: Rc::new(RefCell::new(None)),
            metadata_listeners: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Listen for info events on the core.
    /// see [Core::add_listener_local()]
    pub(super) fn init_core_listeners(&self) -> pipewire::core::Listener {
        self.pw_core
            .add_listener_local()
            .info({
                let store = self.store.clone();
                let sender = self.sender.clone();
                move |info| {
                    tracing::trace!("info event: {info:?}");
                    if let Ok(mut s) = store.try_borrow_mut() {
                        s.set_osg_client_id(info.id());
                        drop(s);
                        let _ = sender.send(ToPipewireMessage::Update);
                    } else {
                        warn!("[PW] re-entrant borrow in core info callback, skipping");
                    }
                }
            })
            .done(|id, seq| {
                tracing::trace!("Pipewire done event: {id}, {seq:?}");
            })
            .error(|id, seq, res, msg| {
                tracing::trace!("PipeWire error event ({id}, {seq}, {res}): {msg:?}");
            })
            .register()
    }

    /// Listen for new events in the registry.
    /// see [Registry::add_listener_local()]
    pub(super) fn registry_listener(&self) -> pipewire::registry::Listener {
        use super::group_nodes::{
            init_device_listeners, init_metadata_listener, init_node_listeners,
        };
        use pipewire::types::ObjectType;
        self.registry
            .add_listener_local()
            .global({
                let store = self.store.clone();
                let registry = self.registry.clone();
                let sender = self.sender.clone();
                let settings_metadata = self.settings_metadata.clone();
                let metadata_listeners = self.metadata_listeners.clone();
                move |global| {
                    let result = match store.try_borrow_mut() {
                        Ok(mut s) => s.add_object(&registry, global),
                        Err(_) => {
                            warn!("[PW] re-entrant borrow in registry global callback, skipping");
                            return;
                        }
                    };
                    match result {
                        Ok(_) => {
                            let _ = sender.send(ToPipewireMessage::Update);
                            // Add param listeners for objects
                            match global.type_ {
                                ObjectType::Node => {
                                    init_node_listeners(store.clone(), sender.clone(), global.id);
                                }
                                ObjectType::Device => {
                                    init_device_listeners(store.clone(), global.id);
                                }
                                ObjectType::Metadata => {
                                    init_metadata_listener(
                                        &registry,
                                        store.clone(),
                                        sender.clone(),
                                        &settings_metadata,
                                        &metadata_listeners,
                                        global,
                                    );
                                }
                                _ => {}
                            }
                        }
                        Err(err) => debug!("Skipping object {}: {err:?}", global.id),
                    }
                }
            })
            .register()
    }

    /// Listen for remove events in the registry.
    /// See [Registry::add_listener_local()]
    pub(super) fn registry_remove_listener(&self) -> pipewire::registry::Listener {
        self.registry
            .add_listener_local()
            .global_remove({
                let store = self.store.clone();
                let sender = self.sender.clone();
                move |global| {
                    if let Ok(mut store_borrow) = store.try_borrow_mut() {
                        store_borrow.remove_object(global);
                        drop(store_borrow);
                        let _ = sender.send(ToPipewireMessage::Update);
                    } else {
                        warn!("[PW] re-entrant borrow in registry remove callback, skipping");
                    }
                }
            })
            .register()
    }

    /// Create a link between two ports. Checks that the ports exist, and their direction. Does
    /// nothing if a link between those two ports already exists.
    pub(super) fn create_port_link(&self, start_id: u32, end_id: u32) -> Result<(), PwError> {
        let store = self.store.borrow();
        let Some(start_port) = store.ports.get(&start_id) else {
            return Err(PwError::PortNotFound(start_id));
        };
        if start_port.kind != PortKind::Source {
            return Err(PwError::InvalidPort(format!(
                "port {start_id} is not a source port"
            )));
        }
        let Some(end_port) = store.ports.get(&end_id) else {
            return Err(PwError::PortNotFound(end_id));
        };
        if end_port.kind != PortKind::Sink {
            return Err(PwError::InvalidPort(format!(
                "port {end_id} is not a sink port"
            )));
        }
        if start_port.links.iter().any(|link_id| {
            store
                .links
                .get(link_id)
                .map(|link| link.start_port == start_port.id && link.end_port == end_port.id)
                .unwrap_or(false)
        }) {
            // The link already exists
            return Ok(());
        }
        self.pw_core
            .create_object::<pipewire::link::Link>(
                "link-factory",
                &properties! {
                    *LINK_OUTPUT_NODE => start_port.node.to_string(),
                    *LINK_OUTPUT_PORT => start_port.id.to_string(),
                    *LINK_INPUT_NODE => end_port.node.to_string(),
                    *LINK_INPUT_PORT => end_port.id.to_string(),
                    *OBJECT_LINGER => "true",
                    *NODE_PASSIVE => "true",
                },
            )
            .map_err(|e| PwError::LinkCreationFailed(e.to_string()))?;
        Ok(())
    }

    /// Create links between all matching ports of two nodes. Checks that both ids are nodes, and
    /// skips links that do not already exist. Only connects nodes in the specified direction.
    pub(super) fn create_node_links(&self, start_id: u32, end_id: u32) -> Result<(), PwError> {
        let store = self.store.borrow();
        let Some(start_node) = store.nodes.get(&start_id) else {
            return Err(PwError::NodeNotFound(start_id));
        };
        let Some(end_node) = store.nodes.get(&end_id) else {
            return Err(PwError::NodeNotFound(end_id));
        };
        let end_ports: Vec<&Port> = end_node
            .ports
            .iter()
            .filter(|(_, kind, _)| *kind == PortKind::Sink)
            .filter_map(|(port_id, _, _)| store.ports.get(port_id))
            .collect();
        let start_ports: Vec<&Port> = start_node
            .ports
            .iter()
            .filter(|(_, kind, _)| *kind == PortKind::Source)
            .filter_map(|(port_id, _, _)| store.ports.get(port_id))
            .collect();
        let port_pairs = map_ports(start_ports, end_ports);
        if port_pairs.is_empty() {
            return Err(PwError::NoPortPairs { start_id, end_id });
        }
        for (start_port, end_port) in port_pairs {
            self.create_port_link(start_port, end_port)?;
        }
        Ok(())
    }

    pub(super) fn remove_port_link(&self, start_id: u32, end_id: u32) -> Result<(), PwError> {
        let store = self.store.borrow_mut();
        // Loop in case multiple links exist between the same ports.
        for link_id in store.links.values().filter_map(|link| {
            (link.start_port == start_id && link.end_port == end_id).then_some(link.id)
        }) {
            self.registry.destroy_global(link_id);
        }
        Ok(())
    }

    pub(super) fn remove_node_links(&self, start_id: u32, end_id: u32) -> Result<(), PwError> {
        let store = self.store.borrow_mut();
        for link_id in store.links.values().filter_map(|link| {
            (link.start_node == start_id && link.end_node == end_id).then_some(link.id)
        }) {
            self.registry.destroy_global(link_id);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)] // instance_id is required for ownership tagging
    pub(super) fn create_group_node(
        &self,
        name: String,
        id: Ulid,
        kind: GroupNodeKind,
        instance_id: Ulid,
    ) -> Result<(), PwError> {
        let proxy = self
            .pw_core
            .create_object::<pipewire::node::Node>(
                "adapter",
                &properties! {
                    *FACTORY_NAME => "support.null-audio-sink",
                    *NODE_NAME => format!("osg.group.{id}"),
                    *NODE_NICK => &*name,
                    *NODE_DESCRIPTION => &*name,
                    *APP_ICON_NAME => OSG_APP_ID,
                    *MEDIA_ICON_NAME => OSG_APP_ID,
                    *DEVICE_ICON_NAME => OSG_APP_ID,
                    "icon_name" => OSG_APP_ID,
                    *APP_NAME => OSG_APP_NAME,
                    *NODE_VIRTUAL => "true",
                    // All nodes use Audio/Sink — Duplex causes WP to route
                    // capture streams (OS volume panel) into our nodes = noise.
                    *MEDIA_CLASS => "Audio/Sink",
                    "audio.position" => "FL,FR",
                    "monitor.channel-volumes" => "true",
                    "monitor.passthrough" => "true",
                    "channelmix.upmix" => "false",
                    "channelmix.normalize" => "false",
                    // Hide from PulseAudio clients to prevent KDE noise
                    "pulse.disable" => "true",
                    // Instance ownership tag for orphan cleanup
                    "osg.instance" => instance_id.to_string(),
                },
            )
            .map_err(|e| PwError::SinkCreationFailed(format!("group node '{name}': {e}")))?;
        let listener = proxy
            .upcast_ref()
            .add_listener_local()
            .bound({
                let store = self.store.clone();
                move |global_id| {
                    if let Some(group_node) = store.borrow_mut().group_nodes.get_mut(&id) {
                        group_node.id = Some(global_id);
                    }
                }
            })
            .removed({
                let store = self.store.clone();
                move || {
                    store.borrow_mut().group_nodes.remove(&id);
                }
            })
            .register();
        self.store.borrow_mut().group_nodes.insert(
            id,
            super::object::GroupNode {
                id: None,
                name,
                kind,
                proxy,
                _listener: listener,
            },
        );
        Ok(())
    }

    pub(super) fn remove_group_node(&self, id: Ulid) -> Result<(), PwError> {
        let mut store = self.store.borrow_mut();
        let group_node = store
            .group_nodes
            .remove(&id)
            .ok_or(PwError::GroupNodeNotFound(id))?;
        drop(group_node);
        Ok(())
    }

    /// Create the staging sink — always-alive, vol=0, for glitch-free rerouting.
    /// Apps transit through this node during channel reassignment so they never
    /// have zero output destinations (which causes audible glitches).
    pub(super) fn create_staging_sink(&self, instance_id: Ulid) -> Result<(), PwError> {
        let proxy = self
            .pw_core
            .create_object::<pipewire::node::Node>(
                "adapter",
                &properties! {
                    *FACTORY_NAME => "support.null-audio-sink",
                    *NODE_NAME => format!("osg.staging.{instance_id}"),
                    *NODE_NICK => "OSG Staging",
                    *NODE_DESCRIPTION => "OSG Staging (silent)",
                    *APP_NAME => OSG_APP_NAME,
                    *NODE_VIRTUAL => "true",
                    *MEDIA_CLASS => "Audio/Sink",
                    "audio.position" => "FL,FR",
                    "monitor.channel-volumes" => "true",
                    "monitor.passthrough" => "true",
                    "channelmix.upmix" => "false",
                    "channelmix.normalize" => "false",
                    "session.suspend-timeout-seconds" => "0",
                    "node.always-process" => "true",
                    // Hide from PulseAudio clients and pavucontrol
                    "pulse.disable" => "true",
                    // Instance ownership tag
                    "osg.instance" => instance_id.to_string(),
                },
            )
            .map_err(|e| PwError::SinkCreationFailed(format!("staging sink: {e}")))?;

        let store_clone = self.store.clone();
        let sender_clone = self.sender.clone();
        let listener = proxy
            .upcast_ref()
            .add_listener_local()
            .bound(move |staging_pw_id| {
                debug!("[PW] staging sink bound as {staging_pw_id}");
                store_clone.borrow_mut().staging_node_id = Some(staging_pw_id);
                // Set volume to 0 — staging sink must be silent
                let _ =
                    sender_clone.send(ToPipewireMessage::NodeVolume(staging_pw_id, vec![0.0, 0.0]));
            })
            .register();

        // Keep the proxy alive by storing it in cell_proxies
        self.store.borrow_mut().cell_proxies.push((proxy, listener));
        self.store.borrow_mut().instance_id = Some(instance_id);
        Ok(())
    }

    /// Remove links from a stream to non-OSG nodes (WP default route leak).
    pub(super) fn remove_stale_stream_links(&self, stream_node_id: u32, keep_target: u32) {
        let s = self.store.borrow();
        let stale: Vec<u32> = s
            .links
            .values()
            .filter(|l| {
                l.start_node == stream_node_id
                    && l.end_node != keep_target
                    && !s
                        .nodes
                        .get(&l.end_node)
                        .and_then(|n| n.identifier.node_name())
                        .is_some_and(|n| n.starts_with("osg."))
            })
            .map(|l| l.id)
            .collect();
        drop(s);
        for id in &stale {
            self.registry.destroy_global(*id);
        }
        if !stale.is_empty() {
            debug!(
                "[PW] removed {} WP default links from stream {stream_node_id}",
                stale.len()
            );
        }
    }
}
