// Endpoint command handlers extracted from update.rs.
//
// Handles: AddEphemeralNode, AddChannel, AddApp, RemoveEndpoint,
//          RenameEndpoint, ChangeChannelKind, SetEndpointVisible

use tracing::warn;

use crate::graph::events::MixerEvent;
use crate::graph::{
    Channel, ChannelId, ChannelKind, Endpoint, EndpointDescriptor, Link, MixerSession,
    NodeIdentity, PortKind, ReconcileSettings, RuntimeState, average_volumes,
};
use crate::pw::AudioGraph;
use crate::routing::handler::CommandHandler;
use crate::routing::messages::{StateMsg, StateOutputMsg};

/// Command handler for endpoint lifecycle messages.
pub struct EndpointCommandHandler;

impl CommandHandler for EndpointCommandHandler {
    fn handles(&self, msg: &StateMsg) -> bool {
        matches!(
            msg,
            StateMsg::AddEphemeralNode(..)
                | StateMsg::AddChannel(..)
                | StateMsg::RemoveEndpoint(..)
                | StateMsg::RenameEndpoint(..)
                | StateMsg::ChangeChannelKind(..)
                | StateMsg::SetEndpointVisible(..)
                | StateMsg::SetEndpointDisabled(..)
                | StateMsg::DismissWelcome
        )
    }

    fn handle(
        &self,
        session: &mut MixerSession,
        msg: StateMsg,
        graph: &AudioGraph,
        rt: &mut RuntimeState,
        settings: &ReconcileSettings,
    ) -> (Option<StateOutputMsg>, Vec<MixerEvent>) {
        let mut events = Vec::new();
        let output = match msg {
            StateMsg::AddEphemeralNode(id, kind) => {
                session.handle_add_ephemeral_node(id, kind.into(), graph, rt)
            }
            StateMsg::AddChannel(name, kind) => {
                session.handle_add_channel(name, kind, rt, &mut events)
            }
            StateMsg::RemoveEndpoint(ep) => {
                session.handle_remove_endpoint(ep, graph, settings, rt, &mut events)
            }
            StateMsg::RenameEndpoint(descriptor, name) => {
                session.handle_rename_endpoint(descriptor, name, rt, &mut events);
                None
            }
            StateMsg::ChangeChannelKind(id, kind) => {
                session.handle_change_channel_kind(id, kind, rt, &mut events);
                None
            }
            StateMsg::SetEndpointVisible(descriptor, visible) => {
                session.handle_set_endpoint_visible(
                    descriptor,
                    visible,
                    graph,
                    settings,
                    &mut events,
                );
                None
            }
            StateMsg::SetEndpointDisabled(descriptor, disabled) => {
                session.handle_set_endpoint_disabled(
                    descriptor,
                    disabled,
                    graph,
                    settings,
                    &mut events,
                );
                None
            }
            StateMsg::DismissWelcome => {
                session.welcome_dismissed = true;
                None
            }
            _ => unreachable!(),
        };
        (output, events)
    }
}

impl MixerSession {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn handle_add_ephemeral_node(
        &mut self,
        id: u32,
        kind: PortKind,
        graph: &AudioGraph,
        rt: &mut RuntimeState,
    ) -> Option<StateOutputMsg> {
        let node = graph
            .nodes
            .get(&id)
            .filter(|n| n.has_port_kind(kind.into()))?;
        let descriptor = EndpointDescriptor::EphemeralNode(id, kind);
        rt.candidates
            .retain(|(cid, ck, _)| *cid != id || *ck != kind);
        let endpoint = Endpoint::new(descriptor)
            .with_display_name(node.identifier.human_name(kind.into()).to_owned())
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
        // If the node matches an existing active app, add as exception.
        let active_app_ids: Vec<_> = self
            .apps
            .iter()
            .filter(|(app_id, _)| rt.app_is_active(app_id))
            .filter(|(_, a)| {
                let ni: NodeIdentity = (&node.identifier).into();
                a.matches(&ni, kind)
            })
            .map(|(app_id, _)| *app_id)
            .collect();
        for app_id in active_app_ids {
            if let Some(app) = self.apps.get_mut(&app_id) {
                app.exceptions.push(descriptor);
            }
        }
        Some(StateOutputMsg::EndpointAdded(descriptor))
    }

    #[allow(clippy::too_many_lines, clippy::too_many_arguments)]
    pub(super) fn handle_add_channel(
        &mut self,
        name: String,
        kind: ChannelKind,
        rt: &mut RuntimeState,
        events: &mut Vec<MixerEvent>,
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
            rt.set_channel_pending(id, true);
            events.push(MixerEvent::CreateGroupNode {
                name,
                ulid: id.inner(),
                kind,
                instance_id: rt.instance_id,
            });
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
                self.links.push(Link::connected_unlocked(src, descriptor));
                rt.set_link_pending((src, descriptor), true);
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
                self.links.push(Link::connected_unlocked(descriptor, sink));
                rt.set_link_pending((descriptor, sink), true);
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
        rt: &mut RuntimeState,
    ) -> Option<StateOutputMsg> {
        let mut app = self.apps.get(&id).cloned()?;
        let descriptor = EndpointDescriptor::App(id, kind);
        rt.set_app_active(id, true);
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
                        .any(|n| {
                            let ni: NodeIdentity = (&n.identifier).into();
                            app.matches(&ni, kind)
                        })
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
        rt: &mut RuntimeState,
        events: &mut Vec<MixerEvent>,
    ) -> Option<StateOutputMsg> {
        if self.endpoints.remove(&ep).is_none() {
            warn!("[State] cannot remove endpoint {ep:?}: not found");
            return None;
        }

        rt.remove_endpoint(ep);
        self.active_sources.retain(|e| *e != ep);
        self.active_sinks.retain(|e| *e != ep);

        for app in self.apps.values_mut() {
            app.exceptions.retain(|e| *e != ep);
        }

        match ep {
            EndpointDescriptor::EphemeralNode(..) => {}
            EndpointDescriptor::Channel(id) => {
                rt.remove_channel(id);
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
                                && node.has_port_kind(PortKind::Source.into())
                            {
                                for &cell_id in &cell_ids {
                                    events.push(MixerEvent::ClearRedirect {
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
                events.push(MixerEvent::RemoveGroupNode { ulid: id.inner() });
                events.push(MixerEvent::RemoveFilter {
                    filter_key: id.inner().to_string(),
                });
            }
            EndpointDescriptor::App(id, _) => {
                if self.resolve_endpoint(ep, graph, settings).is_some() {
                    rt.set_app_active(id, false);
                } else {
                    rt.remove_app(id);
                    self.apps.remove(&id);
                }
            }
            _ => {
                // PersistentNode / Device: no extra cleanup yet.
            }
        }

        Some(StateOutputMsg::EndpointRemoved(ep))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn handle_rename_endpoint(
        &mut self,
        descriptor: EndpointDescriptor,
        name: Option<String>,
        rt: &mut RuntimeState,
        events: &mut Vec<MixerEvent>,
    ) {
        if let EndpointDescriptor::Channel(id) = descriptor {
            if let (Some(endpoint), Some(_ch)) = (
                self.endpoints.get_mut(&descriptor),
                self.channels.get_mut(&id),
            ) && let Some(name) = name.filter(|n| *n != endpoint.display_name)
            {
                events.push(MixerEvent::RemoveGroupNode { ulid: id.inner() });
                events.push(MixerEvent::RemoveFilter {
                    filter_key: id.inner().to_string(),
                });
                endpoint.display_name = name;
                rt.set_channel_pending(id, false);
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

    #[allow(clippy::too_many_arguments)]
    pub(super) fn handle_change_channel_kind(
        &mut self,
        id: ChannelId,
        kind: ChannelKind,
        rt: &mut RuntimeState,
        events: &mut Vec<MixerEvent>,
    ) {
        if let Some(ch) = self.channels.get_mut(&id)
            && kind != ch.kind
        {
            events.push(MixerEvent::RemoveGroupNode { ulid: id.inner() });
            events.push(MixerEvent::RemoveFilter {
                filter_key: id.inner().to_string(),
            });
            ch.kind = kind;
            rt.set_channel_pending(id, false);
        }
    }

    /// Disable or re-enable an endpoint.
    ///
    /// Disabled endpoints are muted on PipeWire and excluded from routing in
    /// the reconciler. The endpoint is retained in state so it can be
    /// re-enabled later.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn handle_set_endpoint_disabled(
        &mut self,
        descriptor: EndpointDescriptor,
        disabled: bool,
        graph: &AudioGraph,
        settings: &ReconcileSettings,
        events: &mut Vec<MixerEvent>,
    ) {
        let nodes = self.resolve_endpoint(descriptor, graph, settings);
        if let Some(endpoint) = self.endpoints.get_mut(&descriptor) {
            endpoint.disabled = disabled;
            if disabled {
                endpoint.volume_locked_muted = endpoint.volume_locked_muted.with_mute(true);
            }
        }
        // Propagate mute state to PipeWire nodes.
        if let Some(nodes) = nodes {
            events.extend(nodes.into_iter().map(|n| MixerEvent::MuteChanged {
                node_id: n.id,
                muted: disabled,
            }));
        }
    }

    #[allow(clippy::too_many_lines, clippy::too_many_arguments)]
    pub(super) fn handle_set_endpoint_visible(
        &mut self,
        descriptor: EndpointDescriptor,
        visible: bool,
        graph: &AudioGraph,
        settings: &ReconcileSettings,
        events: &mut Vec<MixerEvent>,
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
            events.extend(nodes.into_iter().map(|n| MixerEvent::MuteChanged {
                node_id: n.id,
                muted,
            }));
        }
    }
}
