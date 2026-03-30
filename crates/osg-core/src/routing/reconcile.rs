// Adapted from Sonusmix (MPL-2.0) — https://codeberg.org/sonusmix/sonusmix
//
// The correction loop: diff the desired state (`MixerSession`) against the
// PipeWire reality (`AudioGraph`) and emit `ToPipewireMessage` commands to bring
// reality in line with intent.
//
// Key concepts:
//   * `diff_nodes`      — resolve endpoints to PW nodes, mark placeholders
//   * `diff_channels`   — ensure virtual channels exist in PW
//   * `diff_properties` — reconcile volume / mute between desired & actual
//   * `diff_links`      — reconcile connections, respecting lock state

use std::collections::{HashMap, HashSet};

use crate::graph::{
    App, EndpointDescriptor, Link, LinkState, MixerSession, PersistentNodeId, ReconcileSettings,
    VolumeLockMuteState, average_volumes, volumes_mixed,
};
use crate::pw::{AudioGraph, Link as PwLink, Node as PwNode, PortKind, ToPipewireMessage};
use itertools::Itertools;

// ---------------------------------------------------------------------------
// ReconciliationService — stateless domain service
// ---------------------------------------------------------------------------

/// Stateless domain service. Reads MixerSession + AudioGraph, emits corrective commands.
/// PipeWire: no equivalent — this is our domain reconciliation logic.
#[allow(missing_debug_implementations)] // Stateless service, no fields to debug
pub struct ReconciliationService;

impl ReconciliationService {
    /// Compare desired state against PipeWire reality and produce corrective commands.
    pub fn reconcile(
        state: &mut MixerSession,
        graph: &AudioGraph,
        settings: &ReconcileSettings,
    ) -> Vec<ToPipewireMessage> {
        state.diff(graph, settings)
    }
}

// ---------------------------------------------------------------------------
// Top-level diff entry point
// ---------------------------------------------------------------------------

impl MixerSession {
    /// Run the full reconciliation pass. Returns PipeWire commands to execute.
    pub fn diff(
        &mut self,
        graph: &AudioGraph,
        settings: &ReconcileSettings,
    ) -> Vec<ToPipewireMessage> {
        let endpoint_nodes = self.diff_nodes(graph, settings);
        let mut messages = self.diff_channels(&endpoint_nodes);
        messages.extend(self.diff_properties(&endpoint_nodes));
        messages.extend(self.diff_links(graph, &endpoint_nodes));
        messages
    }

    // -----------------------------------------------------------------------
    // diff_nodes — resolve every endpoint to PipeWire nodes
    // -----------------------------------------------------------------------

    /// Try to resolve each endpoint to one or more PipeWire nodes.
    /// Unresolvable endpoints are marked as placeholders. Leftover PW nodes
    /// become candidates. Apps are discovered/updated.
    pub fn diff_nodes<'a>(
        &mut self,
        graph: &'a AudioGraph,
        settings: &ReconcileSettings,
    ) -> HashMap<EndpointDescriptor, Vec<&'a PwNode>> {
        let mut remaining_nodes: HashSet<(u32, PortKind)> = graph
            .nodes
            .values()
            .flat_map(|node| {
                [
                    node.has_port_kind(PortKind::Source)
                        .then_some((node.id, PortKind::Source)),
                    node.has_port_kind(PortKind::Sink)
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
        self.candidates = remaining_nodes
            .into_iter()
            .filter_map(|(id, kind)| {
                let node = graph.nodes.get(&id)?;
                Some((id, kind, node.identifier.clone()))
            })
            .collect();

        // Discover new apps from the graph.
        self.discover_apps(graph);

        endpoint_nodes
    }

    // -----------------------------------------------------------------------
    // diff_channels — ensure virtual channels exist in PipeWire
    // -----------------------------------------------------------------------

    #[allow(clippy::expect_used)] // channel keys come from self.channels iteration
    fn diff_channels(
        &mut self,
        endpoint_nodes: &HashMap<EndpointDescriptor, Vec<&PwNode>>,
    ) -> Vec<ToPipewireMessage> {
        let mut messages = Vec::new();
        for id in self.channels.keys().copied().collect::<Vec<_>>() {
            let endpoint_desc = EndpointDescriptor::Channel(id);
            if let Some(node) = endpoint_nodes
                .get(&endpoint_desc)
                .and_then(|nodes| nodes.first())
            {
                let channel = self.channels.get_mut(&id).expect("channel must exist");
                if channel.pending {
                    channel.pending = false;
                }
                if channel.pipewire_id != Some(node.id) {
                    channel.pipewire_id = Some(node.id);
                }
            } else {
                let channel = self.channels.get_mut(&id).expect("channel must exist");
                if !channel.pending {
                    channel.pending = true;
                    let ep = self
                        .endpoints
                        .get(&endpoint_desc)
                        .expect("endpoint must exist");
                    messages.push(ToPipewireMessage::CreateGroupNode(
                        ep.display_name.clone(),
                        id.inner(),
                        channel.kind,
                    ));
                }
            }
        }
        messages
    }

    // -----------------------------------------------------------------------
    // diff_properties — volume / mute reconciliation
    // -----------------------------------------------------------------------

    /// Compare backend node properties against desired endpoints.
    /// Locked endpoints push their values to PW; unlocked endpoints pull from PW.
    pub fn diff_properties(
        &mut self,
        endpoint_nodes: &HashMap<EndpointDescriptor, Vec<&PwNode>>,
    ) -> Vec<ToPipewireMessage> {
        let mut messages = Vec::new();

        for (ep_desc, nodes) in endpoint_nodes {
            let Some(endpoint) = self.endpoints.get_mut(ep_desc) else {
                continue;
            };
            let num_messages_before = messages.len();

            if endpoint.volume_pending {
                // While a command is in-flight, just check if PW has converged.
                let volumes_match = if endpoint.volume_locked_muted.is_locked() {
                    nodes
                        .iter()
                        .flat_map(|n| &n.channel_volumes)
                        .all(|vol| *vol == endpoint.volume)
                } else {
                    average_volumes(nodes.iter().flat_map(|n| &n.channel_volumes))
                        == endpoint.volume
                };
                let mute_match = endpoint.volume_locked_muted.is_muted()
                    == crate::graph::aggregate_bools(nodes.iter().map(|n| &n.mute));
                if volumes_match && mute_match {
                    endpoint.volume_pending = false;
                }
            } else if endpoint.volume_locked_muted.is_locked() {
                // Locked: push desired volume to any divergent nodes.
                endpoint.volume_mixed = false;
                messages.extend(
                    nodes
                        .iter()
                        .filter(|n| n.channel_volumes.iter().any(|cv| *cv != endpoint.volume))
                        .map(|n| {
                            ToPipewireMessage::NodeVolume(
                                n.id,
                                vec![endpoint.volume; n.channel_volumes.len()],
                            )
                        }),
                );
                // Push desired mute state.
                // Locked endpoints cannot be in MuteMixed state (lock() returns None for it)
                #[allow(clippy::expect_used)]
                let endpoint_muted = endpoint
                    .volume_locked_muted
                    .is_muted()
                    .expect("locked endpoint cannot be MuteMixed");
                messages.extend(
                    nodes
                        .iter()
                        .filter(|n| n.mute != endpoint_muted)
                        .map(|n| ToPipewireMessage::NodeMute(n.id, endpoint_muted)),
                );
            } else {
                // Unlocked: pull volume/mute from PW nodes into desired state.
                endpoint.volume_locked_muted =
                    VolumeLockMuteState::from_bools_unlocked(nodes.iter().map(|n| &n.mute));
                endpoint.volume = average_volumes(nodes.iter().flat_map(|n| &n.channel_volumes));
                for node in nodes {
                    endpoint.volume_mixed = volumes_mixed(&node.channel_volumes);
                }
            }

            if messages.len() > num_messages_before {
                endpoint.volume_pending = true;
            }
        }

        messages
    }

    // -----------------------------------------------------------------------
    // diff_links — connection reconciliation
    // -----------------------------------------------------------------------

    /// Reconcile the link state between desired and actual PipeWire graphs.
    #[allow(clippy::too_many_lines)] // Single match-driven reconciliation loop
    pub fn diff_links(
        &mut self,
        graph: &AudioGraph,
        endpoint_nodes: &HashMap<EndpointDescriptor, Vec<&PwNode>>,
    ) -> Vec<ToPipewireMessage> {
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

            // If a command is in-flight, just check convergence.
            if link.pending {
                if are_endpoints_connected(source, sink, &node_links) == link.state.is_connected() {
                    link.pending = false;
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
                            .map(|(s, k)| ToPipewireMessage::CreateNodeLinks {
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
                            .map(|(s, k)| ToPipewireMessage::RemoveNodeLinks {
                                start_id: s.id,
                                end_id: k.id,
                            }),
                    );
                }
            }

            if messages.len() > num_messages_before {
                link.pending = true;
            }
        }

        // Remove dead links in reverse order to preserve indices.
        for i in to_remove_indices.into_iter().rev() {
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
                Some(true) => self.links.push(Link {
                    start: source_desc,
                    end: sink_desc,
                    state: LinkState::ConnectedUnlocked,
                    pending: false,
                }),
                None => self.links.push(Link {
                    start: source_desc,
                    end: sink_desc,
                    state: LinkState::PartiallyConnected,
                    pending: false,
                }),
                Some(false) => {}
            }
        }

        messages
    }

    // -----------------------------------------------------------------------
    // Endpoint resolution
    // -----------------------------------------------------------------------

    /// Resolve an endpoint descriptor to actual PipeWire nodes.
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
                .filter(|node| node.has_port_kind(kind))
                .map(|node| vec![node]),

            EndpointDescriptor::PersistentNode(_id, _kind) => {
                // TODO: Implement persistent node matching.
                None
            }

            EndpointDescriptor::Channel(id) => graph
                .group_nodes
                .get(&id.inner())
                .and_then(|gn| gn.id)
                .and_then(|nid| graph.nodes.get(&nid))
                .filter(|node| !node.ports.is_empty())
                .map(|node| vec![node]),

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
                    .filter(|node| app.matches(&node.identifier, kind))
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

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Find all PipeWire links between any two active endpoints.
    #[allow(clippy::type_complexity, clippy::unused_self)]
    fn find_relevant_links<'a>(
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

    /// Generate commands to remove PW links between two endpoints.
    #[allow(clippy::too_many_arguments)]
    pub fn remove_pipewire_node_links(
        &self,
        graph: &AudioGraph,
        source: EndpointDescriptor,
        sink: EndpointDescriptor,
        settings: &ReconcileSettings,
    ) -> Vec<ToPipewireMessage> {
        let source_nodes = self
            .resolve_endpoint(source, graph, settings)
            .unwrap_or_default();
        let sink_nodes = self
            .resolve_endpoint(sink, graph, settings)
            .unwrap_or_default();

        let mut messages = Vec::new();
        for src in &source_nodes {
            for snk in &sink_nodes {
                messages.push(ToPipewireMessage::RemoveNodeLinks {
                    start_id: src.id,
                    end_id: snk.id,
                });
            }
        }
        messages
    }

    /// Discover new apps from the PipeWire graph.
    fn discover_apps(&mut self, graph: &AudioGraph) {
        let mut discovered = HashMap::<(String, String, PortKind), String>::new();
        for node in graph.nodes.values() {
            if let (Some(app_name), Some(binary)) = (
                node.identifier.application_name.as_ref(),
                node.identifier.binary_name.as_ref(),
            ) {
                if node.has_port_kind(PortKind::Source) {
                    discovered.insert(
                        (app_name.clone(), binary.clone(), PortKind::Source),
                        node.identifier.icon_name().to_owned(),
                    );
                }
                if node.has_port_kind(PortKind::Sink) {
                    discovered.insert(
                        (app_name.clone(), binary.clone(), PortKind::Sink),
                        node.identifier.icon_name().to_owned(),
                    );
                }
            }
        }
        // Remove existing combinations.
        for app in self.apps.values() {
            discovered.remove(&(app.name.clone(), app.binary.clone(), app.kind));
        }
        // Add new inactive apps.
        for ((name, binary, kind), icon) in discovered {
            let app = App::new_inactive(name, binary, icon, kind);
            self.apps.insert(app.id, app);
        }
    }

    /// Get or create a persistent node for the given PW node.
    pub fn get_persistent_node(&mut self, node: &PwNode, kind: PortKind) -> EndpointDescriptor {
        let id = self
            .persistent_nodes
            .iter()
            .find(|(_, (identifier, nk))| *nk == kind && identifier.matches(&node.identifier))
            .map(|(id, _)| *id);

        if let Some(id) = id {
            EndpointDescriptor::PersistentNode(id, kind)
        } else {
            let id = PersistentNodeId::new();
            self.persistent_nodes
                .insert(id, (node.identifier.clone(), kind));
            EndpointDescriptor::PersistentNode(id, kind)
        }
    }
}

// ---------------------------------------------------------------------------
// Free functions: node/endpoint connectivity checks
// ---------------------------------------------------------------------------

/// Check whether two PW nodes are connected.
/// `Some(true)` = fully connected, `Some(false)` = not connected, `None` = partial.
fn are_nodes_connected(
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

    if source
        .ports
        .iter()
        .filter(|(_, kind, _)| *kind == PortKind::Source)
        .all(|(id, _, _)| relevant_links.iter().any(|link| link.start_port == *id))
        || sink
            .ports
            .iter()
            .filter(|(_, kind, _)| *kind == PortKind::Sink)
            .all(|(id, _, _)| relevant_links.iter().any(|link| link.end_port == *id))
    {
        return Some(true);
    }

    None
}

/// Check whether two endpoints (each possibly multiple nodes) are connected.
fn are_endpoints_connected(
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
