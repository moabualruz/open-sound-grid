// Adapted from Sonusmix (MPL-2.0) — https://codeberg.org/sonusmix/sonusmix

use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::mpsc, thread::JoinHandle};

use std::sync::Arc;

use tracing::{debug, warn};
use ulid::Ulid;

use super::group_nodes::init_pipewire;
use super::master::Master;
use super::{AudioGraph, FromPipewireMessage, PwError, ToPipewireMessage, store::Store};

/// Create the Master and register all registry/core listeners.
fn setup_master(
    store: Rc<RefCell<Store>>,
    pw_core: pipewire::core::CoreRc,
    registry: pipewire::registry::RegistryRc,
    sender: pipewire::channel::Sender<ToPipewireMessage>,
) -> Master {
    let master = Master::new(store, pw_core, registry, sender);
    let _listener = master.registry_listener();
    let _remove_listener = master.registry_remove_listener();
    let _core_listeners = master.init_core_listeners();
    master
}

// Message handler closure is inherently a large match on 18 ToPipewireMessage variants.
// Cannot be extracted into a separate function due to captured environment (master, store, etc.).
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

        let init_result = init_pipewire();
        let (mainloop, _context, pw_core, registry) = match init_result {
            Ok(result) => {
                let _ = init_status_tx.send(Ok(()));
                result
            }
            Err(err) => {
                let _ = init_status_tx.send(Err(err));
                return;
            }
        };

        let master = setup_master(store.clone(), pw_core.clone(), registry, to_pw_tx_clone);

        let startup_cleanup_done = Rc::new(RefCell::new(false));
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
                    // Instance-aware orphan cleanup: reap osg.* nodes from
                    // crashed previous instances (different or missing osg.instance).
                    if !*startup_cleanup_done.borrow() {
                        *startup_cleanup_done.borrow_mut() = true;
                        let s = store.borrow();
                        let current_instance = s.instance_id;
                        let orphans: Vec<u32> = s
                            .nodes
                            .iter()
                            .filter(|(_, n)| {
                                let Some(name) = n.identifier.node_name() else {
                                    return false;
                                };
                                if !name.starts_with("osg.") {
                                    return false;
                                }
                                // Check osg.instance property — reap if missing or stale
                                let node_instance = n.identifier.osg_instance
                                    .as_deref()
                                    .and_then(|v| v.parse::<Ulid>().ok());
                                match (node_instance, current_instance) {
                                    // Node has our instance — keep (we just created it)
                                    (Some(ni), Some(ci)) if ni == ci => false,
                                    // Node has different instance or no instance — orphan
                                    _ => true,
                                }
                            })
                            .map(|(id, _)| *id)
                            .collect();
                        drop(s);
                        for id in &orphans {
                            master.registry.destroy_global(*id);
                        }
                        if !orphans.is_empty() {
                            debug!(
                                "[PW] cleaned {} orphaned osg nodes on startup",
                                orphans.len()
                            );
                        }
                    }
                    for (key, filter) in active_filters.borrow().iter() {
                        let (l, r) = filter.handle().peak();
                        if l > 0.0 || r > 0.0 {
                            if let Some(node_id) = filter.node_id() {
                                peak_store.get_or_insert(node_id).store(l, r);
                            }
                            // Cell filter keys: "{ch_ulid}-to-{mix_ulid}"
                            // Store peak under the cell sink's PW node ID for VU metering.
                            if let Some((ch_ulid, mix_ulid)) = key.split_once("-to-") {
                                let s = store.borrow();
                                if let Some(&cell_pw_id) =
                                    s.cell_node_ids.get(&(ch_ulid.to_owned(), mix_ulid.to_owned()))
                                {
                                    peak_store.get_or_insert(cell_pw_id).store(l, r);
                                }
                            }
                            // Mix filter keys: "mix.{ulid}" — store peak under the mix PW node.
                            if let Some(ulid) = key
                                .strip_prefix("mix.")
                                .and_then(|s| s.parse::<Ulid>().ok())
                            {
                                let s = store.borrow();
                                if let Some((&ch_pw_id, _)) = s.nodes.iter().find(|(_, n)| {
                                    n.identifier
                                        .node_name()
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
                    if let Err(err) = super::volume_ops::set_node_volume(&store.borrow(), id, volume) {
                        warn!("Error setting volume: {err:?}");
                    }
                }
                ToPipewireMessage::NodeMute(id, mute) => {
                    if let Err(err) = super::volume_ops::set_node_mute(&store.borrow(), id, mute) {
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
                ToPipewireMessage::CreateGroupNode(name, id, kind, instance_id) => {
                    if let Err(err) = master.create_group_node(name, id, kind, instance_id) {
                        warn!("Error creating group node: {err:?}");
                    }
                }
                ToPipewireMessage::RemoveGroupNode(name) => {
                    if let Err(err) = master.remove_group_node(name) {
                        warn!("Error removing group node: {err:?}");
                    }
                }
                ToPipewireMessage::SetDefaultSink(node_name, _node_id) => {
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
                ToPipewireMessage::CreateStagingSink { instance_id } => {
                    if let Err(err) = master.create_staging_sink(instance_id) {
                        warn!("[PW] failed to create staging sink: {err:?}");
                    }
                }
                ToPipewireMessage::CreateCellNode {
                    name,
                    cell_id,
                    channel_ulid,
                    mix_ulid,
                    instance_id,
                } => {
                    if let Err(err) = super::cell::create_cell_node(
                        &master.pw_core,
                        &master.store,
                        super::cell::CellNodeArgs {
                            name: name.clone(),
                            cell_id,
                            channel_ulid: channel_ulid.clone(),
                            mix_ulid: mix_ulid.clone(),
                            instance_id,
                        },
                    ) {
                        warn!("[PW] failed to create cell node: {err:?}");
                    }
                    let filter_key = format!("{channel_ulid}-to-{mix_ulid}");
                    let filter_name = format!("osg.filter.{filter_key}");
                    #[allow(unsafe_code)]
                    let filter_result = unsafe {
                        super::filter::OsgFilter::new(
                            pw_core.as_raw_ptr(),
                            &filter_name,
                            &format!("EQ: {name}"),
                        )
                    };
                    match filter_result {
                        Ok(osg_filter) => {
                            filter_store.insert(filter_key.clone(), osg_filter.handle().clone());
                            active_filters
                                .borrow_mut()
                                .insert(filter_key.clone(), osg_filter);
                            debug!("[PW] created resident cell filter '{filter_key}'");
                        }
                        Err(e) => warn!("[PW] failed to create cell filter '{filter_key}': {e}"),
                    }
                }
                ToPipewireMessage::RemoveCellNode { cell_node_id } => {
                    super::cell::remove_all_source_links(
                        &master.store,
                        &master.registry,
                        cell_node_id,
                    );
                    super::cell::remove_all_sink_links(&master.store, &master.registry, cell_node_id);
                    master.registry.destroy_global(cell_node_id);
                    debug!("[PW] removed cell node {cell_node_id}");
                }
                ToPipewireMessage::RedirectStream {
                    stream_node_id,
                    target_node_id,
                } => {
                    master.remove_stale_stream_links(stream_node_id, target_node_id);
                    if let Some(ref metadata) = *master.settings_metadata.borrow() {
                        metadata.set_property(
                            stream_node_id,
                            "target.node",
                            Some("Spa:Id"),
                            Some(&target_node_id.to_string()),
                        );
                    }
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
                    if let Some(ref metadata) = *master.settings_metadata.borrow() {
                        metadata.set_property(stream_node_id, "target.node", None, None);
                    }
                    if let Err(err) = master.remove_node_links(stream_node_id, target_node_id) {
                        debug!(
                            "[PW] no links to clear for {stream_node_id} -> {target_node_id}: {err:?}"
                        );
                    } else {
                        debug!("[PW] cleared redirect {stream_node_id} -> {target_node_id}");
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
                            filter_store.insert(filter_key.clone(), osg_filter.handle().clone());
                            active_filters
                                .borrow_mut()
                                .insert(filter_key.clone(), osg_filter);
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
                ToPipewireMessage::UpdateFilterEffects { filter_key, effects } => {
                    if let Some(handle) = filter_store.get(&filter_key) {
                        let ms_to_s = |ms: f32| ms / 1000.0;
                        let params = super::filter::EffectsParams {
                            compressor: super::filter::CompressorParams {
                                enabled: effects.compressor.enabled,
                                threshold: effects.compressor.threshold,
                                ratio: effects.compressor.ratio,
                                attack: ms_to_s(effects.compressor.attack),
                                release: ms_to_s(effects.compressor.release),
                                makeup: effects.compressor.makeup,
                            },
                            gate: super::filter::GateParams {
                                enabled: effects.gate.enabled,
                                threshold: effects.gate.threshold,
                                hold: ms_to_s(effects.gate.hold),
                                attack: ms_to_s(effects.gate.attack),
                                release: ms_to_s(effects.gate.release),
                            },
                            de_esser: super::filter::DeEsserParams {
                                enabled: effects.de_esser.enabled,
                                frequency: effects.de_esser.frequency,
                                threshold: effects.de_esser.threshold,
                                reduction: effects.de_esser.reduction,
                            },
                            limiter: super::filter::LimiterParams {
                                enabled: effects.limiter.enabled,
                                ceiling: effects.limiter.ceiling,
                                release: ms_to_s(effects.limiter.release),
                            },
                            boost: effects.boost,
                            smart_volume: super::filter::SmartVolumeParams {
                                enabled: effects.smart_volume.enabled,
                                target_db: effects.smart_volume.target_db,
                                speed: effects.smart_volume.speed,
                                max_gain_db: effects.smart_volume.max_gain_db,
                            },
                            spatial: super::filter::SpatialAudioParams {
                                enabled: effects.spatial.enabled,
                                crossfeed: effects.spatial.crossfeed,
                                width: effects.spatial.width,
                            },
                        };
                        handle.set_effects(params);
                        debug!("[PW] updated effects on filter '{filter_key}'");
                    }
                }
                ToPipewireMessage::Exit => {
                    let s = store.borrow();
                    for (&node_id, node) in &s.nodes {
                        if node
                            .identifier
                            .node_name()
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
