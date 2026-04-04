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
    /// Full cell name: `osg.cell.{channel_ulid}-to-{mix_ulid}`
    pub cell_id: String,
    /// Channel ULID string (for cell_node_ids key).
    pub channel_ulid: String,
    /// Mix ULID string (for cell_node_ids key).
    pub mix_ulid: String,
    /// OSG instance ULID stamped on the PW node for ownership tracking.
    pub instance_id: ulid::Ulid,
}

/// Create a per-cell null-audio-sink. ADR-007: apps link directly here.
/// Chain: app_stream → cell_sink → [EQ filter] → mix_sink
pub(super) fn create_cell_node(
    pw_core: &CoreRc,
    store: &Rc<RefCell<Store>>,
    args: CellNodeArgs,
) -> Result<(), PwError> {
    let CellNodeArgs {
        name,
        cell_id,
        channel_ulid,
        mix_ulid,
        instance_id,
    } = args;
    let cell_name = cell_id;
    let proxy = pw_core
        .create_object::<pipewire::node::Node>(
            "adapter",
            &properties! {
                *FACTORY_NAME => "support.null-audio-sink",
                *NODE_NAME => &*cell_name,
                *NODE_NICK => &*name,
                *NODE_DESCRIPTION => &*name,
                *APP_NAME => OSG_APP_NAME,
                *NODE_VIRTUAL => "true",
                *MEDIA_CLASS => "Audio/Sink",
                "audio.position" => "FL,FR",
                "monitor.channel-volumes" => "true",
                "monitor.passthrough" => "true",
                "channelmix.upmix" => "false",
                "channelmix.normalize" => "false",
                "session.suspend-timeout-seconds" => "0",
                "pulse.disable" => "true",
                *OBJECT_LINGER => "true",
                // Instance ownership tag for orphan cleanup
                "osg.instance" => instance_id.to_string(),
            },
        )
        .map_err(|e| PwError::SinkCreationFailed(format!("cell node '{name}': {e}")))?;

    let store_clone = store.clone();
    let ch_key = channel_ulid.clone();
    let mx_key = mix_ulid.clone();
    let listener = proxy
        .upcast_ref()
        .add_listener_local()
        .bound(move |cell_pw_id| {
            debug!(
                "[PW] cell sink {cell_name} bound as {cell_pw_id} \
                 (channel={ch_key}, mix={mx_key})"
            );
            store_clone
                .borrow_mut()
                .cell_node_ids
                .insert((ch_key.clone(), mx_key.clone()), cell_pw_id);
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
