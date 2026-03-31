// Per-cell filter-chain nodes for matrix routing.
//
// Each cell (channel×mix intersection) gets its own filter-chain module
// that acts as a volume + EQ gain stage. Route: channel → cell → mix.

use std::cell::RefCell;
use std::rc::Rc;

use pipewire::registry::RegistryRc;

use super::PwError;
use super::store::Store;

/// Arguments for creating a cell filter-chain node.
pub(super) struct CellNodeArgs {
    pub name: String,
    pub channel_node_id: u32,
    pub mix_node_id: u32,
}

/// Create a per-cell filter-chain node. Route: channel → cell_filter → mix.
/// The cell appears in the PW graph as `osg.cell.{channel_id}.{mix_id}`.
#[allow(unsafe_code)]
pub(super) fn create_cell_filter(
    context_ptr: *mut pipewire_sys::pw_context,
    store: &Rc<RefCell<Store>>,
    args: CellNodeArgs,
) -> Result<(), PwError> {
    let CellNodeArgs {
        name,
        channel_node_id,
        mix_node_id,
    } = args;
    let cell_name = format!("osg.cell.{channel_node_id}.{mix_node_id}");
    let eq = crate::graph::EqConfig::default();

    let chain = unsafe {
        super::filter_chain::EqFilterChain::load(
            context_ptr,
            &cell_name,
            &name,
            "Audio/Duplex",
            &eq,
        )
    }
    .map_err(|e| PwError::SinkCreationFailed(format!("cell filter '{name}': {e}")))?;

    store.borrow_mut().cell_filter_chains.push(chain);
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
