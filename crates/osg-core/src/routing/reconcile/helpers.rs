// Helper functions: link discovery, connectivity checks, default links, link removal

use std::collections::{HashMap, HashSet};

use crate::graph::{
    ChannelKind, EndpointDescriptor, Link, MixerEvent, MixerSession, PortKind, ReconcileSettings,
    RuntimeState,
};
use crate::pw::{AudioGraph, Link as PwLink, Node as PwNode};
use itertools::Itertools;

impl MixerSession {
    /// Find all PipeWire links between any two active endpoints.
    #[allow(clippy::type_complexity, clippy::unused_self)]
    pub(super) fn find_relevant_links<'a>(
        &self,
        graph: &'a AudioGraph,
        endpoint_nodes: &HashMap<EndpointDescriptor, Vec<&'a PwNode>>,
    ) -> (
        HashMap<(u32, u32), Vec<&'a PwLink>>,
        HashSet<(EndpointDescriptor, EndpointDescriptor)>,
    ) {
        let mut node_links: HashMap<(u32, u32), Vec<&PwLink>> = HashMap::new();
        for link in graph.links.values() {
            node_links
                .entry((link.start_node, link.end_node))
                .or_default()
                .push(link);
        }

        let endpoint_links = endpoint_nodes
            .iter()
            .filter(|(ep, _)| ep.is_kind(PortKind::Source))
            .cartesian_product(
                endpoint_nodes
                    .iter()
                    .filter(|(ep, _)| ep.is_kind(PortKind::Sink)),
            )
            .filter_map(|((src_desc, src_nodes), (sink_desc, sink_nodes))| {
                src_nodes
                    .iter()
                    .map(|n| n.id)
                    .cartesian_product(sink_nodes.iter().map(|n| n.id))
                    .any(|ids| node_links.contains_key(&ids))
                    .then_some((*src_desc, *sink_desc))
            })
            .collect();

        (node_links, endpoint_links)
    }

    /// Generate events to remove links between two endpoints.
    #[allow(clippy::too_many_arguments)]
    pub fn remove_node_link_events(
        &self,
        graph: &AudioGraph,
        source: EndpointDescriptor,
        sink: EndpointDescriptor,
        settings: &ReconcileSettings,
    ) -> Vec<MixerEvent> {
        let source_nodes = self
            .resolve_endpoint(source, graph, settings)
            .unwrap_or_default();
        let sink_nodes = self
            .resolve_endpoint(sink, graph, settings)
            .unwrap_or_default();

        let mut messages = Vec::new();
        for src in &source_nodes {
            for snk in &sink_nodes {
                messages.push(MixerEvent::RemoveNodeLinks {
                    start_id: src.id,
                    end_id: snk.id,
                });
            }
        }
        messages
    }

    /// Auto-create ConnectedLocked links for every source × sink pair
    /// that doesn't already have one. New channels and mixes get cells
    /// by default — user can disconnect later.
    pub(super) fn ensure_default_links(&mut self) {
        let sources: Vec<_> = self
            .channels
            .iter()
            .filter(|(_, ch)| ch.kind != ChannelKind::Sink)
            .map(|(id, _)| EndpointDescriptor::Channel(*id))
            .collect();
        let sinks: Vec<_> = self
            .channels
            .iter()
            .filter(|(_, ch)| ch.kind == ChannelKind::Sink)
            .map(|(id, _)| EndpointDescriptor::Channel(*id))
            .collect();
        for source in &sources {
            for sink in &sinks {
                let exists = self
                    .links
                    .iter()
                    .any(|l| l.start == *source && l.end == *sink);
                if !exists {
                    tracing::debug!("[State] ensure_default_links: adding {source:?} → {sink:?}");
                    self.links.push(Link::connected_unlocked(*source, *sink));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Free functions: node/endpoint connectivity checks
// ---------------------------------------------------------------------------

/// Check whether two PW nodes are connected.
/// `Some(true)` = fully connected, `Some(false)` = not connected, `None` = partial.
pub(super) fn are_nodes_connected(
    source: &PwNode,
    sink: &PwNode,
    node_links: &HashMap<(u32, u32), Vec<&PwLink>>,
) -> Option<bool> {
    let relevant_links = node_links
        .get(&(source.id, sink.id))
        .map(|l| l.as_slice())
        .unwrap_or(&[]);

    if relevant_links.is_empty() {
        return Some(false);
    }

    let pw_source: crate::pw::PortKind = PortKind::Source.into();
    let pw_sink: crate::pw::PortKind = PortKind::Sink.into();
    if source
        .ports
        .iter()
        .filter(|(_, kind, _)| *kind == pw_source)
        .all(|(id, _, _)| relevant_links.iter().any(|link| link.start_port == *id))
        || sink
            .ports
            .iter()
            .filter(|(_, kind, _)| *kind == pw_sink)
            .all(|(id, _, _)| relevant_links.iter().any(|link| link.end_port == *id))
    {
        return Some(true);
    }

    None
}

/// Check whether two endpoints (each possibly multiple nodes) are connected.
pub(super) fn are_endpoints_connected(
    source: &[&PwNode],
    sink: &[&PwNode],
    node_links: &HashMap<(u32, u32), Vec<&PwLink>>,
) -> Option<bool> {
    let mut iter = source
        .iter()
        .cartesian_product(sink.iter())
        .map(|(s, k)| are_nodes_connected(s, k, node_links));
    let first = iter.next()??;
    iter.all(|x| x == Some(first)).then_some(first)
}
