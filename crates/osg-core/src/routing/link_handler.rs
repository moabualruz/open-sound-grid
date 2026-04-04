// Link command handlers extracted from update.rs.
//
// Handles: Link, RemoveLink, SetLinkLocked

use tracing::warn;

use crate::graph::events::MixerEvent;
use crate::graph::{
    EndpointDescriptor, Link, LinkState, MixerSession, PortKind, ReconcileSettings, RuntimeState,
};
use crate::pw::AudioGraph;
use crate::routing::handler::CommandHandler;
use crate::routing::messages::{StateMsg, StateOutputMsg};

/// Command handler for link-related messages.
pub struct LinkCommandHandler;

impl CommandHandler for LinkCommandHandler {
    fn handles(&self, msg: &StateMsg) -> bool {
        matches!(
            msg,
            StateMsg::Link(..) | StateMsg::RemoveLink(..) | StateMsg::SetLinkLocked(..)
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
        match msg {
            StateMsg::Link(source, sink) => {
                session.handle_link(source, sink, rt, &mut events);
            }
            StateMsg::RemoveLink(source, sink) => {
                session.handle_remove_link(source, sink, graph, settings, &mut events);
            }
            StateMsg::SetLinkLocked(source, sink, locked) => {
                session.handle_set_link_locked(source, sink, locked, rt);
            }
            _ => unreachable!(),
        }
        (None, events)
    }
}

impl MixerSession {
    /// Generate domain events to remove PW links between two endpoints.
    /// Mirrors `remove_node_link_events` in reconcile.rs.
    fn remove_link_events(
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

        let mut events = Vec::new();
        for src in &source_nodes {
            for snk in &sink_nodes {
                events.push(MixerEvent::RemoveNodeLinks {
                    start_id: src.id,
                    end_id: snk.id,
                });
            }
        }
        events
    }
}

impl MixerSession {
    #[allow(clippy::too_many_lines, clippy::too_many_arguments)]
    pub(super) fn handle_link(
        &mut self,
        source: EndpointDescriptor,
        sink: EndpointDescriptor,
        rt: &mut RuntimeState,
        events: &mut Vec<MixerEvent>,
    ) -> bool {
        if !source.is_kind(PortKind::Source) || !sink.is_kind(PortKind::Sink) {
            warn!("[State] cannot link {source:?} -> {sink:?}: wrong direction");
            return false;
        }

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
        // ADR-007: cell sinks keyed by ULID, not PW node ID
        let cell_id = format!("osg.cell.{src_ulid}-to-{snk_ulid}");
        msgs.push(MixerEvent::CreateCellNode {
            name: format!("{source_name}→{sink_name}"),
            cell_id,
            channel_ulid: src_ulid.clone(),
            mix_ulid: snk_ulid.clone(),
            instance_id: rt.instance_id,
        });

        let link_key = (source, sink);
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
                rt.set_link_pending(link_key, true);
            }
        } else {
            self.links.push(Link::connected_unlocked(source, sink));
            if !msgs.is_empty() {
                rt.set_link_pending(link_key, true);
            }
        }

        events.extend(msgs);
        true
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn handle_remove_link(
        &mut self,
        source: EndpointDescriptor,
        sink: EndpointDescriptor,
        graph: &AudioGraph,
        settings: &ReconcileSettings,
        events: &mut Vec<MixerEvent>,
    ) -> bool {
        if !source.is_kind(PortKind::Source) || !sink.is_kind(PortKind::Sink) {
            warn!("[State] cannot unlink {source:?} -> {sink:?}: wrong direction");
            return false;
        }

        // Destroy cell nodes for this route
        for cell_id in self.find_cell_node_ids(source, sink, graph, settings) {
            events.push(MixerEvent::RemoveCellNode {
                cell_node_id: cell_id,
            });
        }

        let Some(pos) = self
            .links
            .iter()
            .position(|l| l.start == source && l.end == sink)
        else {
            warn!("[State] link not found for removal");
            return false;
        };

        match self.links[pos].state {
            LinkState::PartiallyConnected | LinkState::ConnectedUnlocked => {
                self.links.swap_remove(pos);
                events.extend(self.remove_link_events(graph, source, sink, settings));
            }
            LinkState::ConnectedLocked => {
                self.links[pos].state = LinkState::DisconnectedLocked;
                let link_events = self.remove_link_events(graph, source, sink, settings);
                if !link_events.is_empty() {
                    // Note: link_pending tracked in rt, but handle_remove_link doesn't take rt.
                    // The pending state will be resolved on the next reconcile pass.
                }
                events.extend(link_events);
            }
            LinkState::DisconnectedLocked => {}
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn handle_set_link_locked(
        &mut self,
        source: EndpointDescriptor,
        sink: EndpointDescriptor,
        locked: bool,
        rt: &mut RuntimeState,
    ) -> bool {
        if !source.is_kind(PortKind::Source) || !sink.is_kind(PortKind::Sink) {
            warn!("[State] cannot set link lock {source:?} -> {sink:?}: wrong direction");
            return false;
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
                self.links.push(Link::disconnected_locked(source, sink));
            }
            (_, true) => {}

            (Some((i, LinkState::ConnectedLocked)), false) => {
                self.links[i].state = LinkState::ConnectedUnlocked;
            }
            (Some((i, LinkState::DisconnectedLocked)), false) => {
                let removed = self.links.swap_remove(i);
                rt.remove_link(&(removed.start, removed.end));
            }
            (_, false) => {}
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn handle_set_link_volume(
        &mut self,
        source: EndpointDescriptor,
        sink: EndpointDescriptor,
        volume: f32,
        graph: &AudioGraph,
        settings: &ReconcileSettings,
        events: &mut Vec<MixerEvent>,
    ) {
        let v = volume.clamp(0.0, 1.0);
        if let Some(link) = self
            .links
            .iter_mut()
            .find(|l| l.start == source && l.end == sink)
        {
            link.cell_volume = v;
            link.cell_volume_left = v;
            link.cell_volume_right = v;
        }
        // ADR-007: effective = channel_vol × cell_vol
        let ch_vol = self
            .endpoints
            .get(&source)
            .map(|ep| ep.volume)
            .unwrap_or(1.0);
        let eff = v * ch_vol;
        for cell_id in self.find_cell_node_ids(source, sink, graph, settings) {
            events.push(MixerEvent::VolumeChanged {
                node_id: cell_id,
                channels: vec![eff, eff],
            });
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn handle_set_link_stereo_volume(
        &mut self,
        source: EndpointDescriptor,
        sink: EndpointDescriptor,
        left: f32,
        right: f32,
        graph: &AudioGraph,
        settings: &ReconcileSettings,
        events: &mut Vec<MixerEvent>,
    ) {
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
        // ADR-007: effective = channel_vol × cell_vol
        let ch_ep = self.endpoints.get(&source);
        let ch_l = ch_ep.map(|ep| ep.volume_left).unwrap_or(1.0);
        let ch_r = ch_ep.map(|ep| ep.volume_right).unwrap_or(1.0);
        for cell_id in self.find_cell_node_ids(source, sink, graph, settings) {
            events.push(MixerEvent::VolumeChanged {
                node_id: cell_id,
                channels: vec![l * ch_l, r * ch_r],
            });
        }
    }
}
