// diff_links — connection reconciliation

use std::collections::HashMap;

use crate::graph::{
    EndpointDescriptor, Link, LinkKey, LinkState, MixerEvent, MixerSession, RuntimeState,
};
use crate::pw::{AudioGraph, Node as PwNode};
use itertools::Itertools;

use super::helpers::{are_endpoints_connected, are_nodes_connected};

impl MixerSession {
    /// Reconcile the link state between desired and actual PipeWire graphs.
    #[allow(clippy::too_many_lines)] // Single match-driven reconciliation loop
    pub fn diff_links(
        &mut self,
        graph: &AudioGraph,
        endpoint_nodes: &HashMap<EndpointDescriptor, Vec<&PwNode>>,
        rt: &mut RuntimeState,
    ) -> Vec<MixerEvent> {
        let (node_links, mut remaining_endpoint_links) =
            self.find_relevant_links(graph, endpoint_nodes);

        let mut messages = Vec::new();
        let mut to_remove_indices = Vec::new();

        for (i, link) in self.links.iter_mut().enumerate() {
            remaining_endpoint_links.remove(&(link.start, link.end));

            let (Some(source), Some(sink)) = (
                endpoint_nodes.get(&link.start),
                endpoint_nodes.get(&link.end),
            ) else {
                // One side is a placeholder; skip.
                continue;
            };

            let link_key: LinkKey = (link.start, link.end);

            // If a command is in-flight, just check convergence.
            if rt.link_pending(&link_key) {
                if are_endpoints_connected(source, sink, &node_links) == link.state.is_connected() {
                    rt.set_link_pending(link_key, false);
                }
                continue;
            }

            let num_messages_before = messages.len();

            match link.state {
                LinkState::PartiallyConnected => {
                    match are_endpoints_connected(source, sink, &node_links) {
                        Some(true) => link.state = LinkState::ConnectedUnlocked,
                        Some(false) => to_remove_indices.push(i),
                        None => {}
                    }
                }
                LinkState::ConnectedUnlocked => {
                    match are_endpoints_connected(source, sink, &node_links) {
                        Some(true) => {}
                        Some(false) => to_remove_indices.push(i),
                        None => link.state = LinkState::PartiallyConnected,
                    }
                }
                LinkState::ConnectedLocked => {
                    // Re-create any missing sub-links.
                    messages.extend(
                        source
                            .iter()
                            .cartesian_product(sink.iter())
                            .filter(|(s, k)| are_nodes_connected(s, k, &node_links) != Some(true))
                            .map(|(s, k)| MixerEvent::CreateNodeLinks {
                                start_id: s.id,
                                end_id: k.id,
                            }),
                    );
                }
                LinkState::DisconnectedLocked => {
                    // Remove any sub-links that exist.
                    messages.extend(
                        source
                            .iter()
                            .cartesian_product(sink.iter())
                            .filter(|(s, k)| are_nodes_connected(s, k, &node_links) != Some(false))
                            .map(|(s, k)| MixerEvent::RemoveNodeLinks {
                                start_id: s.id,
                                end_id: k.id,
                            }),
                    );
                }
            }

            if messages.len() > num_messages_before {
                rt.set_link_pending(link_key, true);
            }
        }

        // Remove dead links in reverse order to preserve indices.
        for i in to_remove_indices.into_iter().rev() {
            let removed = &self.links[i];
            rt.remove_link(&(removed.start, removed.end));
            self.links.swap_remove(i);
        }

        // Detect new external links and record them.
        for (source_desc, sink_desc) in remaining_endpoint_links {
            let (Some(source), Some(sink)) = (
                endpoint_nodes.get(&source_desc),
                endpoint_nodes.get(&sink_desc),
            ) else {
                continue;
            };
            match are_endpoints_connected(source, sink, &node_links) {
                Some(true) => self
                    .links
                    .push(Link::connected_unlocked(source_desc, sink_desc)),
                None => self.links.push(Link {
                    state: LinkState::PartiallyConnected,
                    ..Link::connected_unlocked(source_desc, sink_desc)
                }),
                Some(false) => {}
            }
        }

        messages
    }
}
