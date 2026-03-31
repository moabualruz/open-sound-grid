// Per-cell filter nodes for matrix routing.
//
// Each cell (channel×mix intersection) gets its own PipeWire filter node
// that acts as a volume + EQ gain stage. Route: channel → cell → mix.
// Cell volume is set via the filter's process callback.

use std::cell::RefCell;
use std::rc::Rc;

use pipewire::registry::RegistryRc;
use tracing::debug;

use super::PwError;
use super::filter::OsgFilter;
use super::store::Store;

/// Arguments for creating a cell filter node.
pub(super) struct CellNodeArgs {
    pub name: String,
    pub channel_node_id: u32,
    pub mix_node_id: u32,
}

/// Create a per-cell filter node. Route: channel → cell_filter → mix.
/// The cell appears in the PW graph as `osg.cell.{channel_id}.{mix_id}`.
#[allow(unsafe_code)]
pub(super) fn create_cell_filter(
    core_ptr: *mut pipewire_sys::pw_core,
    store: &Rc<RefCell<Store>>,
    args: CellNodeArgs,
) -> Result<(), PwError> {
    let CellNodeArgs {
        name,
        channel_node_id,
        mix_node_id,
    } = args;
    let cell_name = format!("osg.cell.{channel_node_id}.{mix_node_id}");

    let filter = unsafe { OsgFilter::new(core_ptr, &cell_name, "Audio/Duplex") }
        .map_err(|e| PwError::SinkCreationFailed(format!("cell filter '{name}': {e}")))?;

    debug!(
        "[PW] cell filter {cell_name} created (channel={channel_node_id}, mix={mix_node_id}) \
         — node_id: {:?}",
        filter.node_id()
    );

    // Store the filter — the node_id will be available once PW processes it.
    // Cell linking happens via diff_cell_links once the cell appears in the graph.
    store.borrow_mut().cell_filters.push(filter);
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
