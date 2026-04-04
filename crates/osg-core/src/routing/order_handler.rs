// Order and default-node command handlers extracted from update.rs.
//
// Handles: SetChannelOrder, SetMixOrder, SetDefaultOutputNode

use crate::graph::events::MixerEvent;
use crate::graph::{EndpointDescriptor, MixerSession, RuntimeState};
use crate::routing::handler::CommandHandler;
use crate::routing::messages::{StateMsg, StateOutputMsg};

/// Command handler for ordering and default-node messages.
pub struct OrderCommandHandler;

impl CommandHandler for OrderCommandHandler {
    fn handles(&self, msg: &StateMsg) -> bool {
        matches!(
            msg,
            StateMsg::SetChannelOrder(..)
                | StateMsg::SetMixOrder(..)
                | StateMsg::SetDefaultOutputNode(..)
        )
    }

    fn handle(
        &self,
        session: &mut MixerSession,
        msg: StateMsg,
        _graph: &crate::pw::AudioGraph,
        rt: &mut RuntimeState,
        _settings: &crate::graph::ReconcileSettings,
    ) -> (Option<StateOutputMsg>, Vec<MixerEvent>) {
        match msg {
            StateMsg::SetChannelOrder(order) => {
                session.handle_set_channel_order(order);
            }
            StateMsg::SetMixOrder(order) => {
                session.handle_set_mix_order(order);
            }
            StateMsg::SetDefaultOutputNode(node_id) => {
                MixerSession::handle_set_default_output_node(node_id, rt);
            }
            _ => unreachable!(),
        }
        (None, Vec::new())
    }
}

impl MixerSession {
    pub(super) fn handle_set_channel_order(&mut self, order: Vec<EndpointDescriptor>) {
        self.channel_order = order;
    }

    pub(super) fn handle_set_mix_order(&mut self, order: Vec<EndpointDescriptor>) {
        self.mix_order = order;
    }

    pub(super) fn handle_set_default_output_node(node_id: Option<u32>, rt: &mut RuntimeState) {
        rt.default_output_node_id = node_id;
    }
}
