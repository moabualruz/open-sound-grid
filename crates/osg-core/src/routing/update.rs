use crate::graph::{MixerSession, ReconcileSettings, RuntimeState};
use crate::pw::{AudioGraph, ToPipewireMessage};
use crate::routing::messages::{StateMsg, StateOutputMsg};

impl MixerSession {
    /// Process a single state-mutation message. Returns an optional output
    /// notification and a vec of PipeWire commands to send immediately.
    #[allow(clippy::too_many_lines, clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        graph: &AudioGraph,
        message: StateMsg,
        settings: &ReconcileSettings,
        rt: &mut RuntimeState,
    ) -> (Option<StateOutputMsg>, Vec<ToPipewireMessage>) {
        rt.generation = rt.generation.wrapping_add(1);
        let mut pw_messages = Vec::new();

        let output = match message {
            StateMsg::AddEphemeralNode(id, kind) => {
                self.handle_add_ephemeral_node(id, kind.into(), graph, rt)
            }
            StateMsg::AddChannel(name, kind) => {
                self.handle_add_channel(name, kind, rt, &mut pw_messages)
            }
            StateMsg::AddApp(id, kind) => self.handle_add_app(id, kind.into(), graph, settings, rt),
            StateMsg::RemoveEndpoint(ep) => {
                self.handle_remove_endpoint(ep, graph, settings, rt, &mut pw_messages)
            }
            StateMsg::SetVolume(ep_desc, volume) => {
                self.handle_set_volume(ep_desc, volume, graph, settings, rt, &mut pw_messages);
                None
            }
            StateMsg::SetStereoVolume(ep_desc, left, right) => {
                self.handle_set_stereo_volume(
                    ep_desc,
                    left,
                    right,
                    graph,
                    settings,
                    rt,
                    &mut pw_messages,
                );
                None
            }
            StateMsg::SetMute(ep_desc, muted) => {
                self.handle_set_mute(ep_desc, muted, graph, settings, rt, &mut pw_messages);
                None
            }
            StateMsg::SetVolumeLocked(ep_desc, locked) => {
                self.handle_set_volume_locked(
                    ep_desc,
                    locked,
                    graph,
                    settings,
                    rt,
                    &mut pw_messages,
                );
                None
            }
            StateMsg::Link(source, sink) => {
                self.handle_link(source, sink, rt, &mut pw_messages);
                None
            }
            StateMsg::RemoveLink(source, sink) => {
                self.handle_remove_link(source, sink, graph, settings, &mut pw_messages);
                None
            }
            StateMsg::SetLinkLocked(source, sink, locked) => {
                self.handle_set_link_locked(source, sink, locked, rt);
                None
            }
            StateMsg::ChangeChannelKind(id, kind) => {
                self.handle_change_channel_kind(id, kind, rt, &mut pw_messages);
                None
            }
            StateMsg::RenameEndpoint(descriptor, name) => {
                self.handle_rename_endpoint(descriptor, name, rt, &mut pw_messages);
                None
            }
            StateMsg::SetLinkVolume(source, sink, volume) => {
                self.handle_set_link_volume(
                    source,
                    sink,
                    volume,
                    graph,
                    settings,
                    &mut pw_messages,
                );
                None
            }
            StateMsg::SetLinkStereoVolume(source, sink, left, right) => {
                self.handle_set_link_stereo_volume(
                    source,
                    sink,
                    left,
                    right,
                    graph,
                    settings,
                    &mut pw_messages,
                );
                None
            }
            StateMsg::SetMixOutput(channel_id, output_node_id) => {
                pw_messages.extend(self.handle_set_mix_output(
                    channel_id,
                    output_node_id,
                    graph,
                    rt,
                ));
                None
            }
            StateMsg::SetEndpointVisible(descriptor, visible) => {
                self.handle_set_endpoint_visible(
                    descriptor,
                    visible,
                    graph,
                    settings,
                    &mut pw_messages,
                );
                None
            }
            StateMsg::SetChannelOrder(order) => {
                self.handle_set_channel_order(order);
                None
            }
            StateMsg::SetMixOrder(order) => {
                self.handle_set_mix_order(order);
                None
            }
            StateMsg::AssignApp(channel_id, assignment) => {
                pw_messages.extend(self.handle_assign_app(channel_id, assignment, graph, rt));
                None
            }
            StateMsg::UnassignApp(channel_id, assignment) => {
                pw_messages.extend(self.handle_unassign_app(channel_id, assignment, graph, rt));
                None
            }
            StateMsg::SetDefaultOutputNode(node_id) => {
                MixerSession::handle_set_default_output_node(node_id, rt);
                None
            }
            StateMsg::SetEq(ep_desc, eq) => {
                pw_messages.extend(self.handle_set_eq(ep_desc, eq));
                None
            }
            StateMsg::SetCellEq(source, sink, eq) => {
                pw_messages.extend(self.handle_set_cell_eq(source, sink, eq));
                None
            }
            StateMsg::SetEffects(ep_desc, effects) => {
                pw_messages.extend(self.handle_set_effects(ep_desc, effects));
                None
            }
            StateMsg::SetCellEffects(source, sink, effects) => {
                pw_messages.extend(self.handle_set_cell_effects(source, sink, effects));
                None
            }
        };

        (output, pw_messages)
    }
}
