// Adapted from Sonusmix (MPL-2.0) — https://codeberg.org/sonusmix/sonusmix
//
// State mutation handlers: process a `StateMsg` against the `MixerSession`
// and emit PipeWire commands + optional output notifications.

use itertools::Itertools;
use tracing::warn;

use crate::graph::{
    Channel, ChannelId, MixerSession, Endpoint, EndpointDescriptor, Link, LinkState,
    ReconcileSettings, average_volumes,
};
use crate::pw::{AudioGraph, PortKind, ToPipewireMessage};
use crate::routing::messages::{StateMsg, StateOutputMsg};

impl MixerSession {
    /// Process a single state-mutation message. Returns an optional output
    /// notification and a vec of PipeWire commands to send immediately.
    #[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
    pub fn update(
        &mut self,
        graph: &AudioGraph,
        message: StateMsg,
        settings: &ReconcileSettings,
    ) -> (Option<StateOutputMsg>, Vec<ToPipewireMessage>) {
        let mut pw_messages = Vec::new();
        let output = 'handler: {
            match message {
                StateMsg::AddEphemeralNode(id, kind) => {
                    let Some(node) = graph.nodes.get(&id).filter(|n| n.has_port_kind(kind)) else {
                        break 'handler None;
                    };

                    let descriptor = EndpointDescriptor::EphemeralNode(id, kind);

                    self.candidates
                        .retain(|(cid, ck, _)| *cid != id || *ck != kind);

                    let endpoint = Endpoint::new(descriptor)
                        .with_display_name(node.identifier.human_name(kind).to_owned())
                        .with_icon_name(node.identifier.icon_name().to_string())
                        .with_details(
                            node.identifier
                                .details()
                                .map(ToOwned::to_owned)
                                .into_iter()
                                .collect(),
                        )
                        .with_volume(
                            average_volumes(&node.channel_volumes),
                            !node.channel_volumes.iter().all_equal(),
                        )
                        .with_mute_unlocked(node.mute);

                    self.endpoints.insert(descriptor, endpoint);
                    match kind {
                        PortKind::Source => self.active_sources.push(descriptor),
                        PortKind::Sink => self.active_sinks.push(descriptor),
                    }

                    // If the node matches an existing app, add as exception.
                    if let Some(app) = self
                        .apps
                        .values_mut()
                        .find(|a| a.is_active && a.matches(&node.identifier, kind))
                    {
                        app.exceptions.push(descriptor);
                    }

                    Some(StateOutputMsg::EndpointAdded(descriptor))
                }

                StateMsg::AddChannel(name, kind) => {
                    let id = ChannelId::new();
                    let descriptor = EndpointDescriptor::Channel(id);
                    self.channels.insert(
                        id,
                        Channel {
                            id,
                            kind,
                            pipewire_id: None,
                            pending: true,
                        },
                    );
                    self.endpoints.insert(
                        descriptor,
                        Endpoint::new(descriptor).with_display_name(name.clone()),
                    );
                    pw_messages.push(ToPipewireMessage::CreateGroupNode(name, id.inner(), kind));
                    Some(StateOutputMsg::EndpointAdded(descriptor))
                }

                StateMsg::AddApp(id, kind) => {
                    let Some(mut app) = self.apps.get(&id).cloned() else {
                        warn!("[State] cannot add app {id:?}: not in state");
                        break 'handler None;
                    };

                    let descriptor = EndpointDescriptor::App(id, kind);
                    app.is_active = true;
                    match kind {
                        PortKind::Source => self.active_sources.push(descriptor),
                        PortKind::Sink => self.active_sinks.push(descriptor),
                    }

                    // Add matching existing endpoints as exceptions.
                    app.exceptions = self
                        .active_sources
                        .iter()
                        .chain(self.active_sinks.iter())
                        .copied()
                        .filter(|ep| match ep {
                            EndpointDescriptor::EphemeralNode(..)
                            | EndpointDescriptor::PersistentNode(..) => self
                                .resolve_endpoint(*ep, graph, settings)
                                .into_iter()
                                .flatten()
                                .any(|n| app.matches(&n.identifier, kind)),
                            _ => false,
                        })
                        .collect();

                    self.apps.insert(id, app.clone());
                    self.endpoints.insert(
                        descriptor,
                        Endpoint::new(descriptor)
                            .with_display_name(app.name_with_tag())
                            .with_icon_name(app.icon_name),
                    );

                    Some(StateOutputMsg::EndpointAdded(descriptor))
                }

                StateMsg::RemoveEndpoint(ep) => {
                    if self.endpoints.remove(&ep).is_none() {
                        warn!("[State] cannot remove endpoint {ep:?}: not found");
                        break 'handler None;
                    }

                    self.active_sources.retain(|e| *e != ep);
                    self.active_sinks.retain(|e| *e != ep);

                    for app in self.apps.values_mut() {
                        app.exceptions.retain(|e| *e != ep);
                    }

                    match ep {
                        EndpointDescriptor::EphemeralNode(..) => {}
                        EndpointDescriptor::Channel(id) => {
                            if self.channels.shift_remove(&id).is_none() {
                                warn!("[State] channel {id:?} was not in state");
                            }
                            pw_messages.push(ToPipewireMessage::RemoveGroupNode(id.inner()));
                        }
                        EndpointDescriptor::App(id, _) => {
                            if self.resolve_endpoint(ep, graph, settings).is_some() {
                                if let Some(app) = self.apps.get_mut(&id) {
                                    app.is_active = false;
                                } else {
                                    warn!("[State] app {id:?} missing");
                                }
                            } else {
                                self.apps.remove(&id);
                            }
                        }
                        _ => {
                            // PersistentNode / Device: no extra cleanup yet.
                        }
                    }

                    Some(StateOutputMsg::EndpointRemoved(ep))
                }

                StateMsg::SetVolume(ep_desc, volume) => {
                    let nodes = self.resolve_endpoint(ep_desc, graph, settings);
                    let Some(endpoint) = self.endpoints.get_mut(&ep_desc) else {
                        break 'handler None;
                    };
                    endpoint.volume = volume;
                    endpoint.volume_mixed = false;

                    if let Some(nodes) = nodes {
                        let msgs: Vec<_> = nodes
                            .into_iter()
                            .map(|n| {
                                ToPipewireMessage::NodeVolume(
                                    n.id,
                                    vec![volume; n.channel_volumes.len()],
                                )
                            })
                            .collect();
                        if !msgs.is_empty() {
                            endpoint.volume_pending = true;
                        }
                        pw_messages.extend(msgs);
                    }
                    None
                }

                StateMsg::SetMute(ep_desc, muted) => {
                    let nodes = self.resolve_endpoint(ep_desc, graph, settings);
                    let Some(endpoint) = self.endpoints.get_mut(&ep_desc) else {
                        break 'handler None;
                    };
                    endpoint.volume_locked_muted = endpoint.volume_locked_muted.with_mute(muted);

                    if let Some(nodes) = nodes {
                        let msgs: Vec<_> = nodes
                            .into_iter()
                            .map(|n| ToPipewireMessage::NodeMute(n.id, muted))
                            .collect();
                        if !msgs.is_empty() {
                            endpoint.volume_pending = true;
                        }
                        pw_messages.extend(msgs);
                    }
                    None
                }

                StateMsg::SetVolumeLocked(ep_desc, locked) => {
                    let nodes = self.resolve_endpoint(ep_desc, graph, settings);
                    let Some(endpoint) = self.endpoints.get_mut(&ep_desc) else {
                        break 'handler None;
                    };
                    if endpoint.volume_locked_muted.is_locked() == locked {
                        break 'handler None;
                    }

                    if locked {
                        if let Some(new_state) = endpoint.volume_locked_muted.lock() {
                            endpoint.volume_locked_muted = new_state;
                        } else {
                            break 'handler None;
                        }

                        let Some(nodes) = nodes else {
                            break 'handler None;
                        };

                        if !endpoint.volume_pending
                            && nodes
                                .iter()
                                .all(|n| n.channel_volumes.iter().all(|v| *v == endpoint.volume))
                        {
                            break 'handler None;
                        }

                        endpoint.volume_mixed = false;
                        let msgs: Vec<_> = nodes
                            .iter()
                            .map(|n| {
                                ToPipewireMessage::NodeVolume(
                                    n.id,
                                    vec![endpoint.volume; n.channel_volumes.len()],
                                )
                            })
                            .collect();
                        if !msgs.is_empty() {
                            endpoint.volume_pending = true;
                        }
                        pw_messages.extend(msgs);
                    } else {
                        endpoint.volume_locked_muted = endpoint.volume_locked_muted.unlock();
                    }
                    None
                }

                StateMsg::Link(source, sink) => {
                    if !source.is_kind(PortKind::Source) || !sink.is_kind(PortKind::Sink) {
                        warn!("[State] cannot link {source:?} -> {sink:?}: wrong direction");
                        break 'handler None;
                    }

                    let source_nodes = self
                        .resolve_endpoint(source, graph, settings)
                        .unwrap_or_default();
                    let sink_nodes = self
                        .resolve_endpoint(sink, graph, settings)
                        .unwrap_or_default();

                    let mut msgs = Vec::new();
                    for s in &source_nodes {
                        for k in &sink_nodes {
                            msgs.push(ToPipewireMessage::CreateNodeLinks {
                                start_id: s.id,
                                end_id: k.id,
                            });
                        }
                    }

                    if let Some(link) = self
                        .links
                        .iter_mut()
                        .find(|l| l.start == source && l.end == sink)
                    {
                        match link.state {
                            LinkState::PartiallyConnected => {
                                link.state = LinkState::ConnectedUnlocked;
                            }
                            LinkState::DisconnectedLocked => {
                                link.state = LinkState::ConnectedLocked;
                            }
                            _ => {}
                        }
                        if !msgs.is_empty() {
                            link.pending = true;
                        }
                    } else {
                        self.links.push(Link {
                            start: source,
                            end: sink,
                            state: LinkState::ConnectedUnlocked,
                            pending: !msgs.is_empty(),
                        });
                    }

                    pw_messages.extend(msgs);
                    None
                }

                StateMsg::RemoveLink(source, sink) => {
                    if !source.is_kind(PortKind::Source) || !sink.is_kind(PortKind::Sink) {
                        warn!("[State] cannot unlink {source:?} -> {sink:?}: wrong direction");
                        break 'handler None;
                    }

                    let Some(pos) = self
                        .links
                        .iter()
                        .position(|l| l.start == source && l.end == sink)
                    else {
                        warn!("[State] link not found for removal");
                        break 'handler None;
                    };

                    match self.links[pos].state {
                        LinkState::PartiallyConnected | LinkState::ConnectedUnlocked => {
                            self.links.swap_remove(pos);
                            pw_messages.extend(
                                self.remove_pipewire_node_links(graph, source, sink, settings),
                            );
                        }
                        LinkState::ConnectedLocked => {
                            self.links[pos].state = LinkState::DisconnectedLocked;
                            let msgs =
                                self.remove_pipewire_node_links(graph, source, sink, settings);
                            if !msgs.is_empty() {
                                self.links[pos].pending = true;
                            }
                            pw_messages.extend(msgs);
                        }
                        LinkState::DisconnectedLocked => {}
                    }
                    None
                }

                StateMsg::SetLinkLocked(source, sink, locked) => {
                    if !source.is_kind(PortKind::Source) || !sink.is_kind(PortKind::Sink) {
                        warn!(
                            "[State] cannot set link lock {source:?} -> {sink:?}: wrong direction"
                        );
                        break 'handler None;
                    }

                    let pos = self
                        .links
                        .iter()
                        .position(|l| l.start == source && l.end == sink);

                    match (pos.map(|i| (i, self.links[i].state)), locked) {
                        (Some((_, LinkState::PartiallyConnected)), true) => {
                            warn!("[State] cannot lock partially connected link");
                        }
                        (Some((i, LinkState::ConnectedUnlocked)), true) => {
                            self.links[i].state = LinkState::ConnectedLocked;
                        }
                        (None, true) => {
                            self.links.push(Link {
                                start: source,
                                end: sink,
                                state: LinkState::DisconnectedLocked,
                                pending: false,
                            });
                        }
                        (_, true) => {}

                        (Some((i, LinkState::ConnectedLocked)), false) => {
                            self.links[i].state = LinkState::ConnectedUnlocked;
                        }
                        (Some((i, LinkState::DisconnectedLocked)), false) => {
                            self.links.swap_remove(i);
                        }
                        (_, false) => {}
                    }
                    None
                }

                StateMsg::ChangeChannelKind(id, kind) => {
                    if let Some(ch) = self.channels.get_mut(&id)
                        && kind != ch.kind
                    {
                        pw_messages.push(ToPipewireMessage::RemoveGroupNode(id.inner()));
                        ch.kind = kind;
                        ch.pending = false;
                    }
                    None
                }

                StateMsg::RenameEndpoint(descriptor @ EndpointDescriptor::Channel(id), name) => {
                    if let (Some(endpoint), Some(ch)) = (
                        self.endpoints.get_mut(&descriptor),
                        self.channels.get_mut(&id),
                    ) && let Some(name) = name.filter(|n| *n != endpoint.display_name)
                    {
                        pw_messages.push(ToPipewireMessage::RemoveGroupNode(id.inner()));
                        endpoint.display_name = name;
                        ch.pending = false;
                    }
                    None
                }

                StateMsg::RenameEndpoint(ep_desc, name) => {
                    if let Some(endpoint) = self.endpoints.get_mut(&ep_desc) {
                        match name {
                            Some(n) if n == endpoint.display_name => {
                                endpoint.custom_name = None;
                            }
                            _ => endpoint.custom_name = name,
                        }
                    }
                    None
                }
            }
        };

        (output, pw_messages)
    }
}
