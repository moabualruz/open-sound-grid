// Per-cell volume nodes for matrix routing.
//
// Each cell (channel×mix intersection) gets its own PipeWire null-audio-sink
// node that acts as a volume gain stage. Route: channel → cell → mix.
// Cell volume is set via channelVolumes on the cell's PW node.

use std::cell::RefCell;
use std::rc::Rc;

use pipewire::core::CoreRc;
use pipewire::keys::*;
use pipewire::properties::properties;
use pipewire::proxy::ProxyT;
use pipewire::registry::RegistryRc;
use tracing::debug;

use super::PwError;
use super::store::Store;

const OSG_APP_NAME: &str = "open-sound-grid";

/// Arguments for creating a cell node.
pub(super) struct CellNodeArgs {
    pub name: String,
    pub channel_node_id: u32,
    pub mix_node_id: u32,
}

/// Create a per-cell volume node and link it: channel → cell → mix.
/// The `pw_sender` is used to schedule link creation after the cell's ports appear.
pub(super) fn create_cell_node(
    pw_core: &CoreRc,
    store: &Rc<RefCell<Store>>,
    args: CellNodeArgs,
) -> Result<(), PwError> {
    let CellNodeArgs {
        name,
        channel_node_id,
        mix_node_id,
    } = args;
    let cell_name = format!("osg.cell.{channel_node_id}.{mix_node_id}");
    let proxy = pw_core
        .create_object::<pipewire::node::Node>(
            "adapter",
            &properties! {
                *FACTORY_NAME => "support.null-audio-sink",
                *NODE_NAME => &*cell_name,
                *NODE_NICK => &*name,
                *NODE_DESCRIPTION => &*name,
                *APP_NAME => OSG_APP_NAME,
                *MEDIA_CLASS => "Audio/Duplex",
                "audio.position" => "FL,FR",
                "monitor.channel-volumes" => "true",
                "monitor.passthrough" => "true",
                *OBJECT_LINGER => "true",
            },
        )
        .map_err(|e| PwError::SinkCreationFailed(format!("cell node '{name}': {e}")))?;

    let store_clone = store.clone();
    let listener = proxy
        .upcast_ref()
        .add_listener_local()
        .bound(move |cell_id| {
            debug!(
                "[PW] cell node {cell_name} bound as {cell_id} \
                 (channel={channel_node_id}, mix={mix_node_id})"
            );
            store_clone
                .borrow_mut()
                .cell_node_ids
                .insert((channel_node_id, mix_node_id), cell_id);
            // Linking happens via diff_cell_links once the cell appears in the graph.
        })
        .register();

    store.borrow_mut().cell_proxies.push((proxy, listener));
    Ok(())
}

/// Remove all PW links originating from a node.
pub(super) fn remove_all_source_links(store: &RefCell<Store>, registry: &RegistryRc, node_id: u32) {
    let link_ids: Vec<u32> = store
        .borrow()
        .links
        .values()
        .filter(|link| link.start_node == node_id)
        .map(|link| link.id)
        .collect();
    for id in link_ids {
        registry.destroy_global(id);
    }
}

/// Remove all PW links ending at a node.
pub(super) fn remove_all_sink_links(store: &RefCell<Store>, registry: &RegistryRc, node_id: u32) {
    let link_ids: Vec<u32> = store
        .borrow()
        .links
        .values()
        .filter(|link| link.end_node == node_id)
        .map(|link| link.id)
        .collect();
    for id in link_ids {
        registry.destroy_global(id);
    }
}
