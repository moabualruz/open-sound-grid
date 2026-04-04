// resolve_endpoint — map EndpointDescriptor to PipeWire nodes

use crate::graph::{ChannelKind, EndpointDescriptor, MixerSession, NodeIdentity, PortKind, ReconcileSettings};
use crate::pw::{AudioGraph, Node as PwNode};

impl MixerSession {
    /// Resolve an endpoint descriptor to actual PipeWire nodes.
    #[allow(clippy::too_many_lines)]
    pub fn resolve_endpoint<'g>(
        &self,
        endpoint: EndpointDescriptor,
        graph: &'g AudioGraph,
        settings: &ReconcileSettings,
    ) -> Option<Vec<&'g PwNode>> {
        match endpoint {
            EndpointDescriptor::EphemeralNode(id, kind) => graph
                .nodes
                .get(&id)
                .filter(|node| node.has_port_kind(kind.into()))
                .map(|node| vec![node]),

            EndpointDescriptor::PersistentNode(_id, _kind) => {
                // TODO: Implement persistent node matching.
                None
            }

            EndpointDescriptor::Channel(id) => {
                let ch = self.channels.get(&id);
                let is_mix = ch.is_some_and(|c| c.kind == ChannelKind::Sink);
                if is_mix {
                    // Mix channels have PW group nodes
                    graph
                        .group_nodes
                        .get(&id.inner())
                        .and_then(|gn| gn.id)
                        .and_then(|nid| graph.nodes.get(&nid))
                        .filter(|node| !node.ports.is_empty())
                        .map(|node| vec![node])
                } else {
                    // ADR-007: source channels are logical-only.
                    // Channel volume is a model-only multiplier — not applied to PW nodes.
                    // The effective volume (channel × cell) is applied by diff_properties
                    // on the cell sinks. resolve_endpoint returns None for source channels.
                    None
                }
            }

            EndpointDescriptor::App(id, kind) => {
                let app = self.apps.get(&id)?;
                let exceptions: Vec<&PwNode> = app
                    .exceptions
                    .iter()
                    .filter_map(|exc| match exc {
                        EndpointDescriptor::EphemeralNode(..)
                        | EndpointDescriptor::PersistentNode(..) => {
                            self.resolve_endpoint(*exc, graph, settings)
                        }
                        _ => None,
                    })
                    .flatten()
                    .collect();

                let nodes: Vec<&PwNode> = graph
                    .nodes
                    .values()
                    .filter(|node| {
                        let ni: NodeIdentity = (&node.identifier).into();
                        app.matches(&ni, kind)
                    })
                    .filter(|node| {
                        kind != PortKind::Source
                            || settings.app_sources_include_monitors
                            || !node.is_source_monitor()
                    })
                    .filter(|node| !exceptions.iter().any(|n| n.id == node.id))
                    .collect();

                (!nodes.is_empty()).then_some(nodes)
            }

            EndpointDescriptor::Device(_id, _kind) => {
                // TODO: Implement device resolution.
                None
            }
        }
    }
}
