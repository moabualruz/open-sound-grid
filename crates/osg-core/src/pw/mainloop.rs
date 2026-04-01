// Adapted from Sonusmix (MPL-2.0) — https://codeberg.org/sonusmix/sonusmix

use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::mpsc, thread::JoinHandle};

use std::sync::Arc;

use pipewire::{
    context::ContextRc, core::CoreRc, keys::*, main_loop::MainLoopRc, metadata::Metadata,
    properties::properties, proxy::ProxyT, registry::RegistryRc, spa::param::ParamType,
    types::ObjectType,
};
use tracing::{debug, trace, warn};
use ulid::Ulid;

use super::{
    AudioGraph, FromPipewireMessage, GroupNodeKind, OSG_APP_ID, OSG_APP_NAME, PortKind, PwError,
    ToPipewireMessage,
    object::Port,
    store::{Store, map_ports},
};

/// # Master
///
/// The Master handles events which then get inserted into the store.
/// Therefore, the store is the slave of the master, processing what
/// it gets.
struct Master {
    store: Rc<RefCell<Store>>,
    pw_core: CoreRc,
    registry: RegistryRc,
    sender: pipewire::channel::Sender<ToPipewireMessage>,
    /// PipeWire "default" metadata proxy — used to set default.configured.audio.sink.
    settings_metadata: Rc<RefCell<Option<Metadata>>>,
}

impl Master {
    fn new(
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
        }
    }

    /// Listen for info events on the core.
    /// see [Core::add_listener_local()]
    fn init_core_listeners(&self) -> pipewire::core::Listener {
        self.pw_core
            .add_listener_local()
            .info({
                let store = self.store.clone();
                let sender = self.sender.clone();
                move |info| {
                    trace!("info event: {info:?}");
                    store.borrow_mut().set_osg_client_id(info.id());
                    let _ = sender.send(ToPipewireMessage::Update);
                }
            })
            .done(|id, seq| {
                trace!("Pipewire done event: {id}, {seq:?}");
            })
            .error(|id, seq, res, msg| {
                trace!("PipeWire error event ({id}, {seq}, {res}): {msg:?}");
            })
            .register()
    }

    /// Listen for new events in the registry.
    /// see [Registry::add_listener_local()]
    fn registry_listener(&self) -> pipewire::registry::Listener {
        self.registry
            .add_listener_local()
            .global({
                let store = self.store.clone();
                let registry = self.registry.clone();
                let sender = self.sender.clone();
                let settings_metadata = self.settings_metadata.clone();
                move |global| {
                    let result = { store.borrow_mut().add_object(&registry, global) };
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
    fn registry_remove_listener(&self) -> pipewire::registry::Listener {
        self.registry
            .add_listener_local()
            .global_remove({
                let store = self.store.clone();
                let sender = self.sender.clone();
                move |global| {
                    let mut store_borrow = store.borrow_mut();
                    store_borrow.remove_object(global);
                    let _ = sender.send(ToPipewireMessage::Update);
                }
            })
            .register()
    }

    /// Create a link between two ports. Checks that the ports exist, and their direction. Does
    /// nothing if a link between those two ports already exists.
    fn create_port_link(&self, start_id: u32, end_id: u32) -> Result<(), PwError> {
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
    fn create_node_links(&self, start_id: u32, end_id: u32) -> Result<(), PwError> {
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

    fn remove_port_link(&self, start_id: u32, end_id: u32) -> Result<(), PwError> {
        let store = self.store.borrow_mut();
        // There shouldn't be more than one link between the same two ports, but loop just in case
        // there is for some reason.
        for link_id in store.links.values().filter_map(|link| {
            (link.start_port == start_id && link.end_port == end_id).then_some(link.id)
        }) {
            self.registry.destroy_global(link_id);
        }
        Ok(())
    }

    fn remove_node_links(&self, start_id: u32, end_id: u32) -> Result<(), PwError> {
        let store = self.store.borrow_mut();
        for link_id in store.links.values().filter_map(|link| {
            (link.start_node == start_id && link.end_node == end_id).then_some(link.id)
        }) {
            self.registry.destroy_global(link_id);
        }
        Ok(())
    }

    fn create_group_node(
        &self,
        name: String,
        id: Ulid,
        kind: GroupNodeKind,
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

    fn remove_group_node(&self, id: Ulid) -> Result<(), PwError> {
        let mut store = self.store.borrow_mut();
        let group_node = store
            .group_nodes
            .remove(&id)
            .ok_or(PwError::GroupNodeNotFound(id))?;
        drop(group_node);
        Ok(())
    }

    /// Remove links from a stream to non-OSG nodes (WP default route leak).
    fn remove_stale_stream_links(&self, stream_node_id: u32, keep_target: u32) {
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

pub fn init_node_listeners(
    store: Rc<RefCell<Store>>,
    sender: pipewire::channel::Sender<ToPipewireMessage>,
    id: u32,
) {
    if let Some(node) = store.clone().borrow_mut().nodes.get_mut(&id) {
        node.listener = Some(
            node.proxy
                .add_listener_local()
                .info({
                    let store = store.clone();
                    let sender = sender.clone();
                    move |info| {
                        store.borrow_mut().update_node_info(info);
                        let _ = sender.send(ToPipewireMessage::Update);
                    }
                })
                .param({
                    move |_, type_, _, _, pod| {
                        let mut store_borrow = store.borrow_mut();
                        store_borrow.update_node_param(type_, id, pod);
                        let _ = sender.send(ToPipewireMessage::Update);
                    }
                })
                .register(),
        );
        node.proxy
            .enum_params(0, Some(ParamType::Props), 0, u32::MAX);
        node.proxy.subscribe_params(&[ParamType::Props]);
    }
}

pub fn init_device_listeners(store: Rc<RefCell<Store>>, id: u32) {
    if let Some(device) = store.clone().borrow_mut().devices.get_mut(&id) {
        device.listener = Some(
            device
                .proxy
                .add_listener_local()
                .param({
                    move |_seq, type_, index, _next, pod| {
                        store
                            .borrow_mut()
                            .update_device_param(type_, id, index, pod);
                    }
                })
                .register(),
        );
        device
            .proxy
            .enum_params(0, Some(ParamType::Route), 0, u32::MAX);
        device.proxy.subscribe_params(&[ParamType::Route]);
    }
}

/// Bind and listen for PipeWire metadata `default.audio.sink` changes.
/// Stores the metadata proxy in `metadata_out` so it can be used to set the default sink.
#[allow(clippy::too_many_arguments)]
fn init_metadata_listener(
    registry: &RegistryRc,
    store: Rc<RefCell<Store>>,
    sender: pipewire::channel::Sender<ToPipewireMessage>,
    metadata_out: &Rc<RefCell<Option<Metadata>>>,
    global: &pipewire::registry::GlobalObject<&pipewire::spa::utils::dict::DictRef>,
) {
    // Only bind the "default" metadata — it has default.audio.sink/source
    let is_default = global
        .props
        .map(|p| p.get("metadata.name") == Some("default"))
        .unwrap_or(false);
    if !is_default {
        return;
    }

    let Ok(metadata) = registry.bind::<Metadata, _>(global) else {
        warn!("[PW] failed to bind metadata object {}", global.id);
        return;
    };
    debug!("[PW] bound 'default' metadata object (id={})", global.id);
    let listener = metadata
        .add_listener_local()
        .property({
            let store = store.clone();
            let sender = sender.clone();
            move |_subject, key, _type, value| {
                let parse_name = |v: &str| -> Option<String> {
                    serde_json::from_str::<serde_json::Value>(v)
                        .ok()
                        .and_then(|v| v.get("name")?.as_str().map(String::from))
                };
                match key {
                    Some("default.audio.sink") => {
                        let name = value.and_then(parse_name);
                        debug!("default.audio.sink changed: {name:?}");
                        store.borrow_mut().default_sink_name = name;
                        let _ = sender.send(ToPipewireMessage::Update);
                    }
                    Some("default.audio.source") => {
                        let name = value.and_then(parse_name);
                        debug!("default.audio.source changed: {name:?}");
                        store.borrow_mut().default_source_name = name;
                        let _ = sender.send(ToPipewireMessage::Update);
                    }
                    _ => {}
                }
                0
            }
        })
        .register();

    // Store the proxy so we can call set_property later. Leak the listener.
    *metadata_out.borrow_mut() = Some(metadata);
    std::mem::forget(listener);
}

#[allow(
    clippy::type_complexity,
    clippy::too_many_lines,
    clippy::cognitive_complexity
)]
pub(super) fn init_mainloop(
    update_fn: impl Fn(Box<AudioGraph>) + Send + 'static,
    peak_store: Arc<super::peak::PeakStore>,
    filter_store: super::FilterHandleStore,
) -> Result<
    (
        JoinHandle<()>,
        pipewire::channel::Sender<ToPipewireMessage>,
        mpsc::Receiver<FromPipewireMessage>,
    ),
    PwError,
> {
    let (to_pw_tx, to_pw_rx) = pipewire::channel::channel();
    let (from_pw_tx, from_pw_rx) = mpsc::channel();
    let (init_status_tx, init_status_rx) = oneshot::channel::<Result<(), PwError>>();

    let to_pw_tx_clone = to_pw_tx.clone();
    let handle = std::thread::spawn(move || {
        let _sender = from_pw_tx;
        let receiver = to_pw_rx;
        let store = Rc::new(RefCell::new(Store::new()));

        // Initialize PipeWire — using the Rc variants from pipewire-rs 0.9
        // These are internally reference-counted and can be cloned freely.
        let init_result = (|| -> Result<(MainLoopRc, ContextRc, CoreRc, RegistryRc), PwError> {
            let mainloop = MainLoopRc::new(None)
                .map_err(|e| PwError::ConnectionFailed(format!("mainloop init: {e}")))?;
            let context = ContextRc::new(&mainloop, None)
                .map_err(|e| PwError::ConnectionFailed(format!("context init: {e}")))?;
            let pw_core = context
                .connect_rc(Some(properties! {
                    *MEDIA_CATEGORY => "Manager",
                    *APP_ICON_NAME => OSG_APP_ID,
                }))
                .map_err(|e| PwError::ConnectionFailed(format!("core connect: {e}")))?;
            let registry = pw_core
                .get_registry_rc()
                .map_err(|e| PwError::ConnectionFailed(format!("registry: {e}")))?;
            Ok((mainloop, context, pw_core, registry))
        })();
        // If there was an error, report it and exit
        let (mainloop, _context, pw_core, registry) = match init_result {
            Ok(result) => {
                // Receiver dropped means caller already gave up — nothing we can do
                let _ = init_status_tx.send(Ok(()));
                result
            }
            Err(err) => {
                let _ = init_status_tx.send(Err(err));
                return;
            }
        };

        // init registry listener
        let master = Master::new(store.clone(), pw_core.clone(), registry, to_pw_tx_clone);

        let _listener = master.registry_listener();
        let _remove_listener = master.registry_remove_listener();
        let _core_listeners = master.init_core_listeners();

        // Flag: cleanup orphaned osg nodes on the first Update after startup
        let startup_cleanup_done = Rc::new(RefCell::new(false));

        // Active OsgFilter instances — kept alive on the PW thread.
        // FilterHandles are shared via filter_store for cross-thread peak/EQ access.
        let active_filters: Rc<RefCell<HashMap<String, super::filter::OsgFilter>>> =
            Rc::new(RefCell::new(HashMap::new()));

        let _receiver = receiver.attach(mainloop.loop_(), {
            let mainloop = mainloop.clone();
            let store = store.clone();
            let pw_core = pw_core.clone();
            let peak_store = peak_store.clone();
            let startup_cleanup_done = startup_cleanup_done.clone();
            let active_filters = active_filters.clone();
            move |message| match message {
                ToPipewireMessage::Update => {
                    // On first update, destroy orphaned osg nodes from previous runs
                    if !*startup_cleanup_done.borrow() {
                        *startup_cleanup_done.borrow_mut() = true;
                        let s = store.borrow();
                        let orphans: Vec<u32> = s.nodes.iter()
                            .filter(|(_, n)| n.identifier.node_name()
                                .is_some_and(|name| name.starts_with("osg.")))
                            .map(|(id, _)| *id)
                            .collect();
                        drop(s);
                        for id in &orphans {
                            master.registry.destroy_global(*id);
                        }
                        if !orphans.is_empty() {
                            debug!("[PW] cleaned {} orphaned osg nodes on startup", orphans.len());
                        }
                    }
                    // Read peaks from all active pw_filter handles → PeakStore.
                    // Write peaks keyed by filter node ID AND by the filter key
                    // (channel Ulid). The frontend maps channel Ulid → node ID.
                    for (key, filter) in active_filters.borrow().iter() {
                        let (l, r) = filter.handle().peak();
                        if l > 0.0 || r > 0.0 {
                            if let Some(node_id) = filter.node_id() {
                                peak_store.get_or_insert(node_id).store(l, r);
                            }
                            // Also write keyed by the channel's own PW node ID
                            // so frontend VU bars work with existing node IDs.
                            if let Ok(ulid) = key.parse::<Ulid>() {
                                let s = store.borrow();
                                if let Some((&ch_pw_id, _)) = s.nodes.iter().find(|(_, n)| {
                                    n.identifier.node_name()
                                        .is_some_and(|name| name.contains(&ulid.to_string()))
                                }) {
                                    peak_store.get_or_insert(ch_pw_id).store(l, r);
                                }
                            }
                        }
                    }
                    update_fn(Box::new(store.borrow().dump_graph()));
                }
                ToPipewireMessage::NodeVolume(id, volume) => {
                    if let Err(err) = store.borrow_mut().set_node_volume(id, volume) {
                        warn!("Error setting volume: {err:?}");
                    }
                }
                ToPipewireMessage::NodeMute(id, mute) => {
                    if let Err(err) = store.borrow_mut().set_node_mute(id, mute) {
                        warn!("Error setting mute: {err:?}");
                    }
                }
                ToPipewireMessage::CreatePortLink { start_id, end_id } => {
                    if let Err(err) = master.create_port_link(start_id, end_id) {
                        warn!("Error creating port link: {err:?}");
                    };
                }
                ToPipewireMessage::CreateNodeLinks { start_id, end_id } => {
                    if let Err(err) = master.create_node_links(start_id, end_id) {
                        warn!("Error creating node links: {err:?}");
                    };
                }
                ToPipewireMessage::RemovePortLink { start_id, end_id } => {
                    if let Err(err) = master.remove_port_link(start_id, end_id) {
                        warn!("Error removing port link: {err:?}");
                    };
                }
                ToPipewireMessage::RemoveNodeLinks { start_id, end_id } => {
                    if let Err(err) = master.remove_node_links(start_id, end_id) {
                        warn!("Error removing node links: {err:?}");
                    };
                }
                ToPipewireMessage::CreateGroupNode(name, id, kind) => {
                    if let Err(err) = master.create_group_node(name, id, kind) {
                        warn!("Error creating group node: {err:?}");
                    }
                }
                ToPipewireMessage::RemoveGroupNode(name) => {
                    if let Err(err) = master.remove_group_node(name) {
                        warn!("Error removing group node: {err:?}");
                    }
                }
                ToPipewireMessage::SetDefaultSink(node_name, _node_id) => {
                    // Write to default.configured.audio.sink — the user preference key.
                    // WirePlumber watches this, applies it to default.audio.sink,
                    // and persists the choice to disk. This is what wpctl set-default does.
                    if let Some(ref metadata) = *master.settings_metadata.borrow() {
                        let value = format!(r#"{{"name":"{node_name}"}}"#);
                        metadata.set_property(
                            0,
                            "default.configured.audio.sink",
                            Some("Spa:String:JSON"),
                            Some(&value),
                        );
                        debug!("[PW] set default.configured.audio.sink: {node_name}");
                    } else {
                        warn!("[PW] no metadata proxy for SetDefaultSink");
                    }
                }
                ToPipewireMessage::CreateCellNode {
                    name,
                    cell_id,
                    channel_node_id,
                    mix_node_id,
                } => {
                    if let Err(err) = super::cell::create_cell_node(
                        &master.pw_core,
                        &master.store,
                        super::cell::CellNodeArgs {
                            name,
                            cell_id,
                            channel_node_id,
                            mix_node_id,
                        },
                    ) {
                        warn!("[PW] failed to create cell node: {err:?}");
                    }
                }
                ToPipewireMessage::RemoveCellNode { cell_node_id } => {
                    super::cell::remove_all_source_links(
                        &master.store,
                        &master.registry,
                        cell_node_id,
                    );
                    super::cell::remove_all_sink_links(
                        &master.store,
                        &master.registry,
                        cell_node_id,
                    );
                    master.registry.destroy_global(cell_node_id);
                    debug!("[PW] removed cell node {cell_node_id}");
                }
                ToPipewireMessage::RedirectStream {
                    stream_node_id,
                    target_node_id,
                } => {
                    // Remove WP's default links to prevent audio leaking to hardware
                    master.remove_stale_stream_links(stream_node_id, target_node_id);
                    // Set target.object so WP re-routes the stream to our channel.
                    let target_name = store.borrow().nodes.get(&target_node_id)
                        .and_then(|n| n.identifier.node_name().map(String::from));
                    if let Some(ref name) = target_name
                        && let Some(ref metadata) = *master.settings_metadata.borrow()
                    {
                        let value = format!(r#"{{"name":"{name}"}}"#);
                        metadata.set_property(
                            stream_node_id, "target.object",
                            Some("Spa:String:JSON"), Some(&value),
                        );
                    }
                    // Create direct links to our channel
                    if let Err(err) = master.create_node_links(stream_node_id, target_node_id) {
                        warn!("[PW] redirect {stream_node_id} -> {target_node_id}: {err:?}");
                    } else {
                        debug!("[PW] redirect stream {stream_node_id} -> node {target_node_id}");
                    }
                }
                ToPipewireMessage::ClearRedirect {
                    stream_node_id,
                    target_node_id,
                } => {
                    // Clear target.object so WP auto-routes back to default
                    if let Some(ref metadata) = *master.settings_metadata.borrow() {
                        metadata.set_property(stream_node_id, "target.object", None, None);
                    }
                    if let Err(err) =
                        master.remove_node_links(stream_node_id, target_node_id)
                    {
                        debug!(
                            "[PW] no links to clear for {stream_node_id} -> {target_node_id}: {err:?}"
                        );
                    } else {
                        debug!(
                            "[PW] cleared redirect {stream_node_id} -> {target_node_id}"
                        );
                    }
                }
                ToPipewireMessage::CreateFilter { filter_key, name } => {
                    let core_ptr = pw_core.as_raw_ptr();
                    #[allow(unsafe_code)]
                    let result = unsafe {
                        super::filter::OsgFilter::new(
                            core_ptr,
                            &format!("osg.filter.{filter_key}"),
                            &name,
                        )
                    };
                    match result {
                        Ok(osg_filter) => {
                            filter_store.insert(
                                filter_key.clone(),
                                osg_filter.handle().clone(),
                            );
                            active_filters.borrow_mut().insert(filter_key.clone(), osg_filter);
                            debug!("[PW] created inline filter '{filter_key}' ({name})");
                        }
                        Err(e) => {
                            warn!("[PW] failed to create filter '{filter_key}': {e}");
                        }
                    }
                }
                ToPipewireMessage::RemoveFilter { filter_key } => {
                    if active_filters.borrow_mut().remove(&filter_key).is_some() {
                        filter_store.remove(&filter_key);
                        debug!("[PW] removed filter '{filter_key}'");
                    }
                }
                ToPipewireMessage::UpdateFilterEq { filter_key, eq } => {
                    if let Some(handle) = filter_store.get(&filter_key) {
                        handle.set_eq(&eq);
                        debug!("[PW] updated EQ on filter '{filter_key}'");
                    }
                }
                ToPipewireMessage::Exit => {
                    // Cleanup: destroy all lingering osg nodes
                    let s = store.borrow();
                    for (&node_id, node) in &s.nodes {
                        if node.identifier.node_name()
                            .is_some_and(|n| n.starts_with("osg."))
                        {
                            master.registry.destroy_global(node_id);
                        }
                    }
                    drop(s);
                    debug!("[PW] cleaned up osg nodes on shutdown");
                    mainloop.quit();
                }
            }
        });
        debug!("PipeWire mainloop initialization done");
        mainloop.run();
    });

    match init_status_rx.recv() {
        Ok(Ok(_)) => Ok((handle, to_pw_tx, from_pw_rx)),
        Ok(Err(init_error)) => Err(init_error),
        Err(_) => Err(PwError::ThreadExited),
    }
}
