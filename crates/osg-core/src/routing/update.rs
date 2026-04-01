use itertools::Itertools;
use tracing::{debug, warn};

use crate::graph::{
    Channel, ChannelId, ChannelKind, Endpoint, EndpointDescriptor, EqConfig, Link, LinkState,
    MixerSession, ReconcileSettings, average_volumes,
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
                            output_node_id: None,
                            assigned_apps: Vec::new(),
                            pipewire_id: None,
                            pending: kind == ChannelKind::Sink,
                            auto_app: false,
                            allow_app_assignment: kind != ChannelKind::Source,
                        },
                    );
                    self.endpoints.insert(
                        descriptor,
                        Endpoint::new(descriptor).with_display_name(name.clone()),
                    );
                    // ADR-007: Only mixes get PW nodes. Source channels are logical.
                    if kind == ChannelKind::Sink {
                        pw_messages
                            .push(ToPipewireMessage::CreateGroupNode(name, id.inner(), kind));
                    }
                    // Auto-create active links to all existing counterparts.
                    // New source channel → all existing sinks. New sink → all existing sources + apps.
                    if kind == ChannelKind::Sink {
                        // New mix: link every existing source channel + active app to it
                        let sources: Vec<_> = self
                            .channels
                            .iter()
                            .filter(|(_, ch)| ch.kind != ChannelKind::Sink)
                            .map(|(cid, _)| EndpointDescriptor::Channel(*cid))
                            .chain(
                                self.active_sources
                                    .iter()
                                    .copied()
                                    .filter(|d| matches!(d, EndpointDescriptor::App(..))),
                            )
                            .collect();
                        for src in sources {
                            self.links.push(Link {
                                start: src,
                                end: descriptor,
                                state: LinkState::ConnectedUnlocked,
                                cell_volume: 1.0,
                                cell_volume_left: 1.0,
                                cell_volume_right: 1.0,
                                cell_eq: EqConfig::default(),
                                cell_node_id: None,
                                pending: true,
                            });
                        }
                    } else {
                        // New source channel: link it to every existing sink
                        let sinks: Vec<_> = self
                            .channels
                            .iter()
                            .filter(|(cid, ch)| **cid != id && ch.kind == ChannelKind::Sink)
                            .map(|(cid, _)| EndpointDescriptor::Channel(*cid))
                            .collect();
                        for sink in sinks {
                            self.links.push(Link {
                                start: descriptor,
                                end: sink,
                                state: LinkState::ConnectedUnlocked,
                                cell_volume: 1.0,
                                cell_volume_left: 1.0,
                                cell_volume_right: 1.0,
                                cell_eq: EqConfig::default(),
                                cell_node_id: None,
                                pending: true,
                            });
                        }
                    }
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
                            // Clear redirects for all apps assigned to this channel
                            if let Some(ch) = self.channels.get(&id)
                                && let Some(target_id) = ch.pipewire_id
                            {
                                for assignment in &ch.assigned_apps {
                                    for node in graph.nodes.values() {
                                        if node.identifier.application_name.as_deref()
                                            == Some(&assignment.application_name)
                                            && node.identifier.binary_name.as_deref()
                                                == Some(&assignment.binary_name)
                                            && node.has_port_kind(PortKind::Source)
                                        {
                                            pw_messages.push(ToPipewireMessage::ClearRedirect {
                                                stream_node_id: node.id,
                                                target_node_id: target_id,
                                            });
                                        }
                                    }
                                }
                            }
                            if self.channels.shift_remove(&id).is_none() {
                                warn!("[State] channel {id:?} was not in state");
                            }
                            pw_messages.push(ToPipewireMessage::RemoveGroupNode(id.inner()));
                            pw_messages.push(ToPipewireMessage::RemoveFilter {
                                filter_key: id.inner().to_string(),
                            });
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
                    endpoint.volume_left = volume;
                    endpoint.volume_right = volume;
                    endpoint.volume_mixed = false;

                    if let Some(nodes) = nodes {
                        let msgs: Vec<_> = nodes
                            .into_iter()
                            .map(|n| {
                                let len = n.channel_volumes.len().max(2);
                                ToPipewireMessage::NodeVolume(n.id, vec![volume; len])
                            })
                            .collect();
                        if !msgs.is_empty() {
                            endpoint.volume_pending = true;
                        }
                        pw_messages.extend(msgs);
                    }
                    None
                }
                StateMsg::SetStereoVolume(ep_desc, left, right) => {
                    let nodes = self.resolve_endpoint(ep_desc, graph, settings);
                    let Some(endpoint) = self.endpoints.get_mut(&ep_desc) else {
                        break 'handler None;
                    };
                    endpoint.volume = (left + right) / 2.0;
                    endpoint.volume_left = left;
                    endpoint.volume_right = right;
                    endpoint.volume_mixed = (left - right).abs() > f32::EPSILON;

                    if let Some(nodes) = nodes {
                        let msgs: Vec<_> = nodes
                            .into_iter()
                            .map(|n| {
                                let vols = if n.channel_volumes.len() >= 2 {
                                    vec![left, right]
                                } else {
                                    vec![(left + right) / 2.0]
                                };
                                ToPipewireMessage::NodeVolume(n.id, vols)
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
                        let is_device = matches!(ep_desc, EndpointDescriptor::Device(..));
                        if is_device {
                            // Hardware devices honor SPA_PROP_mute
                            let msgs: Vec<_> = nodes
                                .into_iter()
                                .map(|n| ToPipewireMessage::NodeMute(n.id, muted))
                                .collect();
                            if !msgs.is_empty() {
                                endpoint.volume_pending = true;
                            }
                            pw_messages.extend(msgs);
                        } else {
                            // Null-audio-sinks ignore mute — use volume 0 instead
                            if muted {
                                endpoint.pre_mute_volume =
                                    Some((endpoint.volume_left, endpoint.volume_right));
                                endpoint.volume_left = 0.0;
                                endpoint.volume_right = 0.0;
                                endpoint.volume = 0.0;
                            } else if let Some((left, right)) = endpoint.pre_mute_volume.take() {
                                endpoint.volume_left = left;
                                endpoint.volume_right = right;
                                endpoint.volume = (left + right) / 2.0;
                            }
                            let vols = vec![endpoint.volume_left, endpoint.volume_right];
                            let msgs: Vec<_> = nodes
                                .into_iter()
                                .map(|n| ToPipewireMessage::NodeVolume(n.id, vols.clone()))
                                .collect();
                            if !msgs.is_empty() {
                                endpoint.volume_pending = true;
                            }
                            pw_messages.extend(msgs);
                        }
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

                    // Create cell nodes (one per source×sink pair) for per-route volume control.
                    // Route: channel → cell_node (volume) → mix
                    let mut msgs = Vec::new();
                    let source_name = self
                        .endpoints
                        .get(&source)
                        .map(|e| e.display_name.clone())
                        .unwrap_or_default();
                    let sink_name = self
                        .endpoints
                        .get(&sink)
                        .map(|e| e.display_name.clone())
                        .unwrap_or_default();
                    let src_ulid = if let EndpointDescriptor::Channel(id) = source {
                        id.inner().to_string()
                    } else {
                        format!("{source:?}")
                    };
                    let snk_ulid = if let EndpointDescriptor::Channel(id) = sink {
                        id.inner().to_string()
                    } else {
                        format!("{sink:?}")
                    };
                    for s in &source_nodes {
                        for k in &sink_nodes {
                            let cell_id = format!("osg.cell.{src_ulid}-to-{snk_ulid}");
                            msgs.push(ToPipewireMessage::CreateCellNode {
                                name: format!("{source_name}→{sink_name}"),
                                cell_id,
                                channel_node_id: s.id,
                                mix_node_id: k.id,
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
                            cell_volume: 1.0,
                            cell_volume_left: 1.0,
                            cell_volume_right: 1.0,
                            cell_node_id: None,
                            pending: !msgs.is_empty(),
                            cell_eq: EqConfig::default(),
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

                    // Destroy cell nodes for this route
                    for cell_id in self.find_cell_node_ids(source, sink, graph, settings) {
                        pw_messages.push(ToPipewireMessage::RemoveCellNode {
                            cell_node_id: cell_id,
                        });
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
                                cell_volume: 1.0,
                                cell_volume_left: 1.0,
                                cell_volume_right: 1.0,
                                cell_node_id: None,
                                pending: false,
                                cell_eq: EqConfig::default(),
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
                        pw_messages.push(ToPipewireMessage::RemoveFilter {
                            filter_key: id.inner().to_string(),
                        });
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
                        pw_messages.push(ToPipewireMessage::RemoveFilter {
                            filter_key: id.inner().to_string(),
                        });
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
                StateMsg::SetLinkVolume(source, sink, volume) => {
                    if let Some(link) = self
                        .links
                        .iter_mut()
                        .find(|l| l.start == source && l.end == sink)
                    {
                        let v = volume.clamp(0.0, 1.0);
                        link.cell_volume = v;
                        link.cell_volume_left = v;
                        link.cell_volume_right = v;
                    }
                    let v = volume.clamp(0.0, 1.0);
                    for cell_id in self.find_cell_node_ids(source, sink, graph, settings) {
                        pw_messages.push(ToPipewireMessage::NodeVolume(cell_id, vec![v, v]));
                    }
                    None
                }
                StateMsg::SetLinkStereoVolume(source, sink, left, right) => {
                    if let Some(link) = self
                        .links
                        .iter_mut()
                        .find(|l| l.start == source && l.end == sink)
                    {
                        let l = left.clamp(0.0, 1.0);
                        let r = right.clamp(0.0, 1.0);
                        link.cell_volume_left = l;
                        link.cell_volume_right = r;
                        link.cell_volume = (l + r) / 2.0;
                    }
                    let l = left.clamp(0.0, 1.0);
                    let r = right.clamp(0.0, 1.0);
                    for cell_id in self.find_cell_node_ids(source, sink, graph, settings) {
                        pw_messages.push(ToPipewireMessage::NodeVolume(cell_id, vec![l, r]));
                    }
                    None
                }
                StateMsg::SetMixOutput(channel_id, output_node_id) => {
                    if let Some(ch) = self.channels.get_mut(&channel_id) {
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
                            pw_messages
                                .push(ToPipewireMessage::SetDefaultSink(name.to_owned(), new_id));
                        }
                    }
                    None
                }
                StateMsg::SetEndpointVisible(descriptor, visible) => {
                    let nodes = self.resolve_endpoint(descriptor, graph, settings);
                    if let Some(endpoint) = self.endpoints.get_mut(&descriptor) {
                        endpoint.visible = visible;
                        // Mute when hiding, unmute when showing
                        if !visible {
                            endpoint.volume_locked_muted =
                                endpoint.volume_locked_muted.with_mute(true);
                        }
                    }
                    // Mute/unmute the PipeWire nodes
                    if let Some(nodes) = nodes {
                        let muted = !visible;
                        pw_messages.extend(
                            nodes
                                .into_iter()
                                .map(|n| ToPipewireMessage::NodeMute(n.id, muted)),
                        );
                    }
                    None
                }
                StateMsg::SetChannelOrder(order) => {
                    self.channel_order = order;
                    None
                }
                StateMsg::SetMixOrder(order) => {
                    self.mix_order = order;
                    None
                }
                StateMsg::AssignApp(channel_id, assignment) => {
                    let Some(ch) = self.channels.get_mut(&channel_id) else {
                        warn!("[State] cannot assign app: channel {channel_id:?} not found");
                        break 'handler None;
                    };

                    // Don't add duplicates
                    if ch.assigned_apps.contains(&assignment) {
                        break 'handler None;
                    }

                    let target_node_id = ch.pipewire_id;
                    ch.assigned_apps.push(assignment.clone());

                    // Find all matching PW stream nodes and redirect them
                    if let Some(target_id) = target_node_id {
                        for node in graph.nodes.values() {
                            if node.identifier.application_name.as_deref()
                                == Some(&assignment.application_name)
                                && node.identifier.binary_name.as_deref()
                                    == Some(&assignment.binary_name)
                                && node.has_port_kind(PortKind::Source)
                            {
                                pw_messages.push(ToPipewireMessage::RedirectStream {
                                    stream_node_id: node.id,
                                    target_node_id: target_id,
                                });
                            }
                        }
                    }
                    // Destroy the app's auto-channel — it will be recreated on unassign
                    let auto_id = self
                        .channels
                        .iter()
                        .find(|(_, c)| {
                            c.auto_app
                                && c.assigned_apps.iter().any(|a| {
                                    a.application_name == assignment.application_name
                                        && a.binary_name == assignment.binary_name
                                })
                        })
                        .map(|(id, _)| *id);
                    if let Some(id) = auto_id {
                        self.endpoints.remove(&EndpointDescriptor::Channel(id));
                        self.links.retain(|l| {
                            l.start != EndpointDescriptor::Channel(id)
                                && l.end != EndpointDescriptor::Channel(id)
                        });
                        self.channels.shift_remove(&id);
                        pw_messages.push(ToPipewireMessage::RemoveGroupNode(id.inner()));
                        pw_messages.push(ToPipewireMessage::RemoveFilter {
                            filter_key: id.inner().to_string(),
                        });
                    }
                    None
                }
                StateMsg::UnassignApp(channel_id, assignment) => {
                    let Some(ch) = self.channels.get_mut(&channel_id) else {
                        warn!("[State] cannot unassign app: channel {channel_id:?} not found");
                        break 'handler None;
                    };

                    let target_node_id = ch.pipewire_id;
                    ch.assigned_apps.retain(|a| a != &assignment);

                    // Force a graph update so the reconciler picks up the unassigned
                    // app immediately and creates a new auto-channel for it.
                    pw_messages.push(ToPipewireMessage::Update);

                    // Clear redirect on all matching PW stream nodes
                    if let Some(target_id) = target_node_id {
                        for node in graph.nodes.values() {
                            if node.identifier.application_name.as_deref()
                                == Some(&assignment.application_name)
                                && node.identifier.binary_name.as_deref()
                                    == Some(&assignment.binary_name)
                                && node.has_port_kind(PortKind::Source)
                            {
                                pw_messages.push(ToPipewireMessage::ClearRedirect {
                                    stream_node_id: node.id,
                                    target_node_id: target_id,
                                });
                                debug!(
                                    "[State] cleared redirect for {} (node {})",
                                    assignment.application_name, node.id
                                );
                            }
                        }
                    }
                    None
                }
                StateMsg::SetDefaultOutputNode(node_id) => {
                    self.default_output_node_id = node_id;
                    None
                }
                StateMsg::SetEq(ep_desc, eq) => {
                    if let Some(ep) = self.endpoints.get_mut(&ep_desc) {
                        ep.eq = eq.clone();
                    }
                    // Dispatch EQ to PW filter if one exists for this endpoint
                    let filter_key = match ep_desc {
                        EndpointDescriptor::Channel(id) => id.inner().to_string(),
                        _ => String::new(),
                    };
                    if !filter_key.is_empty() {
                        pw_messages.push(ToPipewireMessage::UpdateFilterEq { filter_key, eq });
                    }
                    None
                }
                StateMsg::SetCellEq(source, sink, eq) => {
                    if let Some(l) = self
                        .links
                        .iter_mut()
                        .find(|l| l.start == source && l.end == sink)
                    {
                        l.cell_eq = eq.clone();
                    }
                    // Dispatch EQ to cell's PW filter
                    let filter_key = match (&source, &sink) {
                        (EndpointDescriptor::Channel(ch), EndpointDescriptor::Channel(mx)) => {
                            format!("{}-to-{}", ch.inner(), mx.inner())
                        }
                        _ => String::new(),
                    };
                    if !filter_key.is_empty() {
                        pw_messages.push(ToPipewireMessage::UpdateFilterEq { filter_key, eq });
                    }
                    None
                }
            }
        };

        (output, pw_messages)
    }
}
