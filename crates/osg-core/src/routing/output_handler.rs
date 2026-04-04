// Mix output command handler extracted from eq_handlers.rs.
//
// Handles: SetMixOutput (hardware output routing)

use crate::graph::{ChannelId, EndpointDescriptor, MixerSession};
use crate::pw::{AudioGraph, ToPipewireMessage};

impl MixerSession {
    /// Handle `StateMsg::SetMixOutput` — change a mix's hardware output.
    pub(crate) fn handle_set_mix_output(
        &mut self,
        channel_id: ChannelId,
        output_node_id: Option<u32>,
        graph: &AudioGraph,
    ) -> Vec<ToPipewireMessage> {
        let mut pw_messages = Vec::new();
        let Some(ch) = self.channels.get_mut(&channel_id) else {
            return pw_messages;
        };
        let old_output = ch.output_node_id;
        ch.output_node_id = output_node_id;

        // Remove old links if previously assigned
        if let (Some(pw_id), Some(old_id)) = (ch.pipewire_id, old_output) {
            pw_messages.push(ToPipewireMessage::RemoveNodeLinks {
                start_id: pw_id,
                end_id: old_id,
            });
        }

        // Create new links to the output device
        if let (Some(pw_id), Some(new_id)) = (ch.pipewire_id, output_node_id) {
            pw_messages.push(ToPipewireMessage::CreateNodeLinks {
                start_id: pw_id,
                end_id: new_id,
            });
        }

        // If this is the Monitor mix, update OS default sink
        let desc = EndpointDescriptor::Channel(channel_id);
        let is_monitor = self
            .endpoints
            .get(&desc)
            .map(|ep| ep.display_name.to_lowercase().contains("monitor"))
            .unwrap_or(false);
        if is_monitor
            && let Some(new_id) = output_node_id
            && let Some(node) = graph.nodes.get(&new_id)
            && let Some(name) = node.identifier.node_name()
        {
            pw_messages.push(ToPipewireMessage::SetDefaultSink(name.to_owned(), new_id));
        }
        pw_messages
    }
}
