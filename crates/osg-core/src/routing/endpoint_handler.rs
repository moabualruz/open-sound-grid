// Endpoint command handlers extracted from update.rs.
//
// Handles: AddEphemeralNode, AddChannel, AddApp, RemoveEndpoint,
//          RenameEndpoint, ChangeChannelKind, SetEndpointVisible

use tracing::warn;

use crate::graph::{
    Channel, ChannelId, ChannelKind, EffectsConfig, Endpoint, EndpointDescriptor, EqConfig, Link,
    LinkState, MixerSession, ReconcileSettings, average_volumes,
};
use crate::pw::{AudioGraph, PortKind, ToPipewireMessage};
use crate::routing::messages::StateOutputMsg;

impl MixerSession {
    pub(super) fn handle_add_ephemeral_node(
        &mut self,
        id: u32,
        kind: PortKind,
        graph: &AudioGraph,
    ) -> Option<StateOutputMsg> {
        let node = graph.nodes.get(&id).filter(|n| n.has_port_kind(kind))?;
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
                !node.channel_volumes.iter().all(|v| {
                    let first = node.channel_volumes.first().copied().unwrap_or(0.0);
                    (*v - first).abs() < f32::EPSILON
                }),
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

    #[allow(clippy::too_many_lines)]
    pub(super) fn handle_add_channel(
        &mut self,
        name: String,
        kind: ChannelKind,
        pw_messages: &mut Vec<ToPipewireMessage>,
    ) -> Option<StateOutputMsg> {
        let id = ChannelId::new();
        let descriptor = EndpointDescriptor::Channel(id);
        self.channels.insert(
            id,
            Channel {
                id,
                kind,
                source_type: crate::graph::SourceType::default(),
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
            pw_messages.push(ToPipewireMessage::CreateGroupNode(
                name,
                id.inner(),
                kind,
                self.instance_id,
            ));
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
                    cell_effects: EffectsConfig::default(),
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
                    cell_effects: EffectsConfig::default(),
                    cell_node_id: None,
                    pending: true,
                });
            }
        }
        Some(StateOutputMsg::EndpointAdded(descriptor))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn handle_add_app(
        &mut self,
        id: crate::graph::AppId,
        kind: PortKind,
        graph: &AudioGraph,
        settings: &ReconcileSettings,
    ) -> Option<StateOutputMsg> {
        let mut app = self.apps.get(&id).cloned()?;
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
                EndpointDescriptor::EphemeralNode(..) | EndpointDescriptor::PersistentNode(..) => {
                    self.resolve_endpoint(*ep, graph, settings)
                        .into_iter()
                        .flatten()
                        .any(|n| app.matches(&n.identifier, kind))
                }
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

    #[allow(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        clippy::cognitive_complexity
    )]
    pub(super) fn handle_remove_endpoint(
        &mut self,
        ep: EndpointDescriptor,
        graph: &AudioGraph,
        settings: &ReconcileSettings,
        pw_messages: &mut Vec<ToPipewireMessage>,
    ) -> Option<StateOutputMsg> {
        if self.endpoints.remove(&ep).is_none() {
            warn!("[State] cannot remove endpoint {ep:?}: not found");
            return None;
        }

        self.active_sources.retain(|e| *e != ep);
        self.active_sinks.retain(|e| *e != ep);

        for app in self.apps.values_mut() {
            app.exceptions.retain(|e| *e != ep);
        }

        match ep {
            EndpointDescriptor::EphemeralNode(..) => {}
            EndpointDescriptor::Channel(id) => {
                // ADR-007: Clear links from app streams to cell sinks
                if let Some(ch) = self.channels.get(&id) {
                    let prefix = format!("osg.cell.{}-to-", id.inner());
                    let cell_ids: Vec<u32> = graph
                        .nodes
                        .iter()
                        .filter_map(|(&nid, n)| {
                            n.identifier
                                .node_name()
                                .filter(|name| name.starts_with(&prefix))
                                .map(|_| nid)
                        })
                        .collect();
                    for assignment in &ch.assigned_apps {
                        for node in graph.nodes.values() {
                            if node.identifier.application_name.as_deref()
                                == Some(&assignment.application_name)
                                && node.identifier.binary_name.as_deref()
                                    == Some(&assignment.binary_name)
                                && node.has_port_kind(PortKind::Source)
                            {
                                for &cell_id in &cell_ids {
                                    pw_messages.push(ToPipewireMessage::ClearRedirect {
                                        stream_node_id: node.id,
                                        target_node_id: cell_id,
                                    });
                                }
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

    pub(super) fn handle_rename_endpoint(
        &mut self,
        descriptor: EndpointDescriptor,
        name: Option<String>,
        pw_messages: &mut Vec<ToPipewireMessage>,
    ) {
        if let EndpointDescriptor::Channel(id) = descriptor {
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
        } else if let Some(endpoint) = self.endpoints.get_mut(&descriptor) {
            match name {
                Some(n) if n == endpoint.display_name => {
                    endpoint.custom_name = None;
                }
                _ => endpoint.custom_name = name,
            }
        }
    }

    pub(super) fn handle_change_channel_kind(
        &mut self,
        id: ChannelId,
        kind: ChannelKind,
        pw_messages: &mut Vec<ToPipewireMessage>,
    ) {
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
    }

    #[allow(clippy::too_many_lines, clippy::too_many_arguments)]
    pub(super) fn handle_set_endpoint_visible(
        &mut self,
        descriptor: EndpointDescriptor,
        visible: bool,
        graph: &AudioGraph,
        settings: &ReconcileSettings,
        pw_messages: &mut Vec<ToPipewireMessage>,
    ) {
        let nodes = self.resolve_endpoint(descriptor, graph, settings);
        if let Some(endpoint) = self.endpoints.get_mut(&descriptor) {
            endpoint.visible = visible;
            // Mute when hiding, unmute when showing
            if !visible {
                endpoint.volume_locked_muted = endpoint.volume_locked_muted.with_mute(true);
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
    }
}
