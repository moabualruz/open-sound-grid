// Adapted from Sonusmix (MPL-2.0) — https://codeberg.org/sonusmix/sonusmix

use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::mpsc, thread::JoinHandle};

use std::sync::Arc;

use pipewire::{
    context::ContextRc, core::CoreRc, keys::*, main_loop::MainLoopRc, metadata::Metadata,
    properties::properties, proxy::ProxyT, registry::RegistryRc, spa::param::ParamType,
    stream::StreamRc, types::ObjectType,
};
use tracing::{debug, trace, warn};
use ulid::Ulid;

use super::{
    AudioGraph, FromPipewireMessage, GroupNodeKind, OSG_APP_ID, OSG_APP_NAME, PortKind, PwError,
    ToPipewireMessage, object::Port, store::Store,
};

/// # Master
///
/// The Master handles events which then get inserted into the store.
/// Therefore, the store is the slave of the master, processing what
/// it gets.
struct Master {
    store: Rc<RefCell<Store>>,
    pw_core: CoreRc,
    context: ContextRc,
    registry: RegistryRc,
    sender: pipewire::channel::Sender<ToPipewireMessage>,
    /// PipeWire "default" metadata proxy — used to set default.configured.audio.sink.
    settings_metadata: Rc<RefCell<Option<Metadata>>>,
}

impl Master {
    fn new(
        store: Rc<RefCell<Store>>,
        pw_core: CoreRc,
        context: ContextRc,
        registry: RegistryRc,
        sender: pipewire::channel::Sender<ToPipewireMessage>,
    ) -> Self {
        Master {
            store,
            pw_core,
            context,
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

    /// Link pending peak stream nodes to their target's monitor ports.
    fn resolve_peak_links(&self, pending: &mut HashMap<String, u32>) {
        let store = self.store.borrow();
        let resolved: Vec<String> = pending
            .iter()
            .filter_map(|(peak_name, &target_id)| {
                let (&peak_node_id, peak_node) = store
                    .nodes
                    .iter()
                    .find(|(_, n)| n.identifier.node_name() == Some(peak_name.as_str()))?;
                let has_sink_ports = peak_node
                    .ports
                    .iter()
                    .any(|(_, kind, _)| *kind == PortKind::Sink);
                if !has_sink_ports {
                    return None;
                }
                let target_node = store.nodes.get(&target_id)?;
                let monitor_ports: Vec<&Port> = target_node
                    .ports
                    .iter()
                    .filter_map(|(port_id, _, _)| store.ports.get(port_id))
                    .filter(|p| p.kind == PortKind::Source && p.is_monitor)
                    .collect();
                let sink_ports: Vec<&Port> = peak_node
                    .ports
                    .iter()
                    .filter_map(|(port_id, _, _)| store.ports.get(port_id))
                    .filter(|p| p.kind == PortKind::Sink)
                    .collect();
                if monitor_ports.is_empty() || sink_ports.is_empty() {
                    return None;
                }
                let pairs = map_ports(monitor_ports, sink_ports);
                for (src, dst) in &pairs {
                    if let Err(e) = self.create_port_link(*src, *dst) {
                        warn!("[PW] peak link {peak_name} failed: {e}");
                    }
                }
                if !pairs.is_empty() {
                    debug!(
                        "[PW] linked peak {peak_name} (node {peak_node_id}) → target {target_id} ({} pairs)",
                        pairs.len()
                    );
                }
                Some(peak_name.clone())
            })
            .collect();
        for name in resolved {
            pending.remove(&name);
        }
    }

    #[allow(unsafe_code)]
    fn create_group_node(
        &self,
        name: String,
        id: Ulid,
        kind: GroupNodeKind,
    ) -> Result<(), PwError> {
        // pw_filter with correct media.class — appears under Sinks in WirePlumber,
        // apps can route to it, and we get a DSP process callback for EQ.
        // No node.link-group = not classified as a Filter by WP.
        let filter = super::filter::create_group_filter(
            self.pw_core.as_raw_ptr(), &name, id, kind,
        ).map_err(|e| PwError::SinkCreationFailed(e))?;
        self.store.borrow_mut().group_filters.0.insert(id, filter);
        Ok(())
    }

    fn remove_group_node(&self, id: Ulid) -> Result<(), PwError> {
        let mut store = self.store.borrow_mut();
        if let Some(filter) = store.group_filters.0.remove(&id) {
            drop(filter);
            return Ok(());
        }
        Err(PwError::GroupNodeNotFound(id))
    }
}

/// Maps two different list of ports to a list of mappings.
/// These are made at best guess but by no means are always correct.
/// Standard cases such as surround sound, stereo and MONO ports should
/// always be correctly mapped.
///
/// | Situation | Output |
/// |-----------|--------|
/// | start = 1 | map single port to all end ports |
/// | otherwise | map by channel names |
pub fn map_ports<P>(start: Vec<&Port<P>>, end: Vec<&Port<P>>) -> Vec<(u32, u32)> {
    if start.len() == 1 {
        return end
            .iter()
            .map(|end_port| (start[0].id, end_port.id))
            .collect();
    }
    let pairs: Vec<(u32, u32)> = start
        .iter()
        .enumerate()
        .filter_map(|(index, start_port)| {
            let start_port_id: u32 = start_port.id;
            // Try matching by channel name first, then fall back to positional
            let end_port_id: Option<u32> = end
                .get(index)
                .and_then(|port| (port.channel == start_port.channel).then_some(port.id))
                .or_else(|| {
                    Some(
                        end.iter()
                            .find(|end_port| end_port.channel == start_port.channel)?
                            .id,
                    )
                });
            if end_port_id.is_none() {
                trace!("Could not find matching end port for {}", start_port_id);
            }
            Some((start_port_id, end_port_id?))
        })
        .collect();

    // Fall back to positional mapping when channel names don't match
    // (e.g. FL/FR vs AUX0/AUX1 on pro audio hardware)
    if pairs.is_empty() && !start.is_empty() && !end.is_empty() {
        return start
            .iter()
            .zip(end.iter())
            .map(|(s, e)| (s.id, e.id))
            .collect();
    }
    pairs
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
        let (mainloop, context, pw_core, registry) = match init_result {
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

        let master = Master::new(store.clone(), pw_core.clone(), context, registry, to_pw_tx_clone);
        let _listener = master.registry_listener();
        let _remove_listener = master.registry_remove_listener();
        let _core_listeners = master.init_core_listeners();

        // Peak monitor streams — dropping them stops the stream.
        let peak_streams: Rc<
            RefCell<HashMap<u32, (StreamRc, pipewire::stream::StreamListener<()>)>>,
        > = Rc::new(RefCell::new(HashMap::new()));
        // Pending peak links: peak_stream_name → target_node_id.
        let pending_peak_links: Rc<RefCell<HashMap<String, u32>>> =
            Rc::new(RefCell::new(HashMap::new()));

        let _receiver = receiver.attach(mainloop.loop_(), {
            let mainloop = mainloop.clone();
            let store = store.clone();
            let pw_core = pw_core.clone();
            let peak_streams = peak_streams.clone();
            let peak_store = peak_store.clone();
            let pending_peak_links = pending_peak_links.clone();
            move |message| match message {
                ToPipewireMessage::Update => {
                    if !pending_peak_links.borrow().is_empty() {
                        master.resolve_peak_links(&mut pending_peak_links.borrow_mut());
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
                    channel_node_id,
                    mix_node_id,
                } => {
                    if let Err(err) = super::cell::create_cell_filter(
                        master.pw_core.as_raw_ptr(),
                        &master.store,
                        super::cell::CellNodeArgs {
                            name,
                            channel_node_id,
                            mix_node_id,
                        },
                    ) {
                        warn!("[PW] failed to create cell filter: {err:?}");
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
                    // First disconnect the stream from wherever it's currently linked
                    super::cell::remove_all_source_links(
                        &master.store,
                        &master.registry,
                        stream_node_id,
                    );
                    // Then create links to the target channel node
                    if let Err(err) =
                        master.create_node_links(stream_node_id, target_node_id)
                    {
                        warn!(
                            "[PW] failed to create links {stream_node_id} -> {target_node_id}: {err:?}"
                        );
                    } else {
                        debug!(
                            "[PW] redirect stream {stream_node_id} -> node {target_node_id}"
                        );
                    }
                }
                ToPipewireMessage::ClearRedirect {
                    stream_node_id,
                    target_node_id,
                } => {
                    // Remove links between stream and channel. WirePlumber will
                    // auto-link the stream back to the default sink.
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
                ToPipewireMessage::StartPeakMonitor(node_id) => {
                    if peak_streams.borrow().contains_key(&node_id) {
                        return;
                    }
                    match super::peak::create_peak_stream(
                        pw_core.clone(),
                        node_id,
                        &peak_store,
                    ) {
                        Ok((stream, listener, peak_name)) => {
                            pending_peak_links.borrow_mut().insert(peak_name, node_id);
                            peak_streams
                                .borrow_mut()
                                .insert(node_id, (stream, listener));
                        }
                        Err(e) => warn!("[PW] peak monitor failed for {node_id}: {e}"),
                    }
                }
                ToPipewireMessage::StopPeakMonitor(node_id) => {
                    if peak_streams.borrow_mut().remove(&node_id).is_some() {
                        peak_store.remove(node_id);
                        debug!("[PW] peak monitor stopped for node {node_id}");
                    }
                }
                ToPipewireMessage::SetFilterEq { node_id, ref eq } => {
                    let s = store.borrow();
                    let applied = s.group_filters.0.values()
                        .chain(s.cell_filters.iter())
                        .find(|f| f.node_id() == Some(node_id))
                        .map(|f| f.handle().set_eq(eq))
                        .is_some();
                    debug!("[PW] SetFilterEq node {node_id}: {}", if applied { "applied" } else { "no filter" });
                }
                ToPipewireMessage::Exit => mainloop.quit(),
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
