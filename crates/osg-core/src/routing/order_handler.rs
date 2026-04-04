// Order and default-node command handlers extracted from update.rs.
//
// Handles: SetChannelOrder, SetMixOrder, SetDefaultOutputNode

use crate::graph::{EndpointDescriptor, MixerSession};

impl MixerSession {
    pub(super) fn handle_set_channel_order(&mut self, order: Vec<EndpointDescriptor>) {
        self.channel_order = order;
    }

    pub(super) fn handle_set_mix_order(&mut self, order: Vec<EndpointDescriptor>) {
        self.mix_order = order;
    }

    pub(super) fn handle_set_default_output_node(&mut self, node_id: Option<u32>) {
        self.default_output_node_id = node_id;
    }
}
