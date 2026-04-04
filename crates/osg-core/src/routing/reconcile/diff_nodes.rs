// diff_nodes — resolve every endpoint to PipeWire nodes
// diff_channels — ensure virtual channels exist in PipeWire

use std::collections::{HashMap, HashSet};

use crate::graph::{
    ChannelKind, EndpointDescriptor, MixerEvent, MixerSession, NodeIdentity, PortKind,
    ReconcileSettings, RuntimeState,
};
use crate::pw::{AudioGraph, Node as PwNode};

impl MixerSession {
    /// Try to resolve each endpoint to one or more PipeWire nodes.
    /// Unresolvable endpoints are marked as placeholders. Leftover PW nodes
    /// become candidates. Apps are discovered/updated.
    pub fn diff_nodes<'a>(
        &mut self,
        graph: &'a AudioGraph,
        settings: &ReconcileSettings,
        rt: &mut RuntimeState,
    ) -> HashMap<EndpointDescriptor, Vec<&'a PwNode>> {
        let mut remaining_nodes: HashSet<(u32, PortKind)> = graph
            .nodes
            .values()
            .flat_map(|node| {
                [
                    node.has_port_kind(PortKind::Source.into())
                        .then_some((node.id, PortKind::Source)),
                    node.has_port_kind(PortKind::Sink.into())
                        .then_some((node.id, PortKind::Sink)),
                ]
            })
            .flatten()
            .collect();

        let mut endpoint_nodes = HashMap::new();
        for endpoint in self.endpoints.keys().copied().collect::<Vec<_>>() {
            if let Some(nodes) = self.resolve_endpoint(endpoint, graph, settings) {
                // Mark the endpoint's nodes as seen
                for node in &nodes {
                    if endpoint.is_single() && endpoint.is_kind(PortKind::Source) {
                        remaining_nodes.remove(&(node.id, PortKind::Source));
                    }
                    if endpoint.is_single() && endpoint.is_kind(PortKind::Sink) {
                        remaining_nodes.remove(&(node.id, PortKind::Sink));
                    }
                }

                if let Some(ep) = self.endpoints.get_mut(&endpoint) {
                    let mut details: Vec<String> = nodes
                        .iter()
                        .filter_map(|node| node.identifier.details())
                        .map(ToOwned::to_owned)
                        .collect();
                    details.sort_unstable();
                    ep.details = details;
                    ep.is_placeholder = false;
                }

                endpoint_nodes.insert(endpoint, nodes);
            } else if let Some(ep) = self.endpoints.get_mut(&endpoint) {
                ep.is_placeholder = true;
            }
        }
        // Leftover PW nodes become candidates for the UI to offer.
        rt.candidates = remaining_nodes
            .into_iter()
            .filter_map(|(id, kind)| {
                let node = graph.nodes.get(&id)?;
                Some((id, kind, NodeIdentity::from(&node.identifier)))
            })
            .collect();

        // Discover new apps from the graph.
        self.discover_apps(graph);

        endpoint_nodes
    }

    #[allow(clippy::expect_used, clippy::expect_fun_call)] // channel keys come from self.channels iteration
    pub(super) fn diff_channels(
        &self,
        endpoint_nodes: &HashMap<EndpointDescriptor, Vec<&PwNode>>,
        _graph: &AudioGraph,
        rt: &mut RuntimeState,
    ) -> Vec<MixerEvent> {
        let mut messages = Vec::new();
        for id in self.channels.keys().copied().collect::<Vec<_>>() {
            let channel_kind = self
                .channels
                .get(&id)
                .expect(&format!("channel {id:?} must exist in channels map"))
                .kind;
            // ADR-007: Source channels are logical-only — no PW node.
            // Only Sink (mix) channels get PW group nodes.
            if channel_kind != ChannelKind::Sink {
                continue;
            }
            let endpoint_desc = EndpointDescriptor::Channel(id);
            if let Some(node) = endpoint_nodes
                .get(&endpoint_desc)
                .and_then(|nodes| nodes.first())
            {
                if rt.channel_pending(&id) {
                    rt.set_channel_pending(id, false);
                }
                if rt.channel_pipewire_id(&id) != Some(node.id) {
                    rt.set_channel_pipewire_id(id, Some(node.id));
                    // Create resident mix-level EQ filter
                    let ep = self
                        .endpoints
                        .get(&endpoint_desc)
                        .expect(&format!("endpoint for channel {id:?} must exist"));
                    messages.push(MixerEvent::CreateFilter {
                        filter_key: format!("mix.{}", id.inner()),
                        name: format!("EQ: {}", ep.display_name),
                    });
                }
            } else {
                let channel = self
                    .channels
                    .get(&id)
                    .expect(&format!("channel {id:?} must exist for mutation"));
                if !rt.channel_pending(&id) {
                    rt.set_channel_pending(id, true);
                    let ep = self
                        .endpoints
                        .get(&endpoint_desc)
                        .expect(&format!("endpoint for channel {id:?} must exist"));
                    messages.push(MixerEvent::CreateGroupNode {
                        name: ep.display_name.clone(),
                        ulid: id.inner(),
                        kind: channel.kind,
                        instance_id: rt.instance_id,
                    });
                }
            }
        }
        messages
    }
}
