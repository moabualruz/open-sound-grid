// Mix output command handler extracted from eq_handlers.rs.
//
// Handles: SetMixOutput (hardware output routing)

use crate::graph::events::MixerEvent;
use crate::graph::{ChannelId, EndpointDescriptor, MixerSession, RuntimeState};
use crate::pw::AudioGraph;
use crate::routing::handler::CommandHandler;
use crate::routing::messages::{StateMsg, StateOutputMsg};

/// Command handler for output routing messages.
pub struct OutputCommandHandler;

impl CommandHandler for OutputCommandHandler {
    fn handles(&self, msg: &StateMsg) -> bool {
        matches!(msg, StateMsg::SetMixOutput(..))
    }

    fn handle(
        &self,
        session: &mut MixerSession,
        msg: StateMsg,
        graph: &AudioGraph,
        rt: &mut RuntimeState,
        _settings: &crate::graph::ReconcileSettings,
    ) -> (Option<StateOutputMsg>, Vec<MixerEvent>) {
        match msg {
            StateMsg::SetMixOutput(channel_id, output_node_id) => {
                let events = session.handle_set_mix_output(channel_id, output_node_id, graph, rt);
                (None, events)
            }
            _ => unreachable!(),
        }
    }
}

impl MixerSession {
    /// Handle `StateMsg::SetMixOutput` — change a mix's hardware output.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_set_mix_output(
        &mut self,
        channel_id: ChannelId,
        output_node_id: Option<u32>,
        graph: &AudioGraph,
        rt: &RuntimeState,
    ) -> Vec<MixerEvent> {
        let mut events = Vec::new();
        let Some(ch) = self.channels.get_mut(&channel_id) else {
            return events;
        };
        let old_output = ch.output_node_id;
        ch.output_node_id = output_node_id;

        let pw_id = rt.channel_pipewire_id(&channel_id);

        // Remove old links if previously assigned
        if let (Some(pw_id), Some(old_id)) = (pw_id, old_output) {
            events.push(MixerEvent::RemoveNodeLinks {
                start_id: pw_id,
                end_id: old_id,
            });
        }

        // Create new links to the output device
        if let (Some(pw_id), Some(new_id)) = (pw_id, output_node_id) {
            events.push(MixerEvent::CreateNodeLinks {
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
            events.push(MixerEvent::SetDefaultSink {
                node_name: name.to_owned(),
                pipewire_node_id: new_id,
            });
        }
        events
    }
}
