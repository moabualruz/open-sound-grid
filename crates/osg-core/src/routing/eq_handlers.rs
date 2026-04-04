// EQ, effects, and app-assignment command handlers extracted from update.rs.
//
// These are helper methods on MixerSession called from the main `update()` match.

use tracing::debug;

use crate::graph::AppAssignment;
use crate::graph::{ChannelKind, EffectsConfig, EndpointDescriptor, EqConfig, MixerSession};
use crate::pw::{AudioGraph, PortKind, ToPipewireMessage};

impl MixerSession {
    /// Handle `StateMsg::SetEq` — update endpoint EQ and dispatch to PW filter.
    pub(crate) fn handle_set_eq(
        &mut self,
        ep_desc: EndpointDescriptor,
        eq: EqConfig,
    ) -> Vec<ToPipewireMessage> {
        let mut pw_messages = Vec::new();
        if let Some(ep) = self.endpoints.get_mut(&ep_desc) {
            ep.eq = eq.clone();
        }
        // Dispatch EQ to PW filter — mix filters keyed as "mix.{ulid}"
        let filter_key = match ep_desc {
            EndpointDescriptor::Channel(id) => {
                let ch = self.channels.get(&id);
                if ch.is_some_and(|c| c.kind == ChannelKind::Sink) {
                    format!("mix.{}", id.inner())
                } else {
                    String::new() // source channels have no direct filter
                }
            }
            _ => String::new(),
        };
        if !filter_key.is_empty() {
            pw_messages.push(ToPipewireMessage::UpdateFilterEq { filter_key, eq });
        }
        pw_messages
    }

    /// Handle `StateMsg::SetCellEq` — update link EQ and dispatch to cell filter.
    pub(crate) fn handle_set_cell_eq(
        &mut self,
        source: EndpointDescriptor,
        sink: EndpointDescriptor,
        eq: EqConfig,
    ) -> Vec<ToPipewireMessage> {
        let mut pw_messages = Vec::new();
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
        pw_messages
    }

    /// Handle `StateMsg::SetEffects` — update endpoint effects and dispatch to PW filter.
    pub(crate) fn handle_set_effects(
        &mut self,
        ep_desc: EndpointDescriptor,
        effects: EffectsConfig,
    ) -> Vec<ToPipewireMessage> {
        let mut pw_messages = Vec::new();
        if let Some(ep) = self.endpoints.get_mut(&ep_desc) {
            ep.effects = effects.clone();
        }
        // Dispatch effects to PW filter — mix filters keyed as "mix.{ulid}"
        let filter_key = match ep_desc {
            EndpointDescriptor::Channel(id) => {
                let ch = self.channels.get(&id);
                if ch.is_some_and(|c| c.kind == ChannelKind::Sink) {
                    format!("mix.{}", id.inner())
                } else {
                    String::new()
                }
            }
            _ => String::new(),
        };
        if !filter_key.is_empty() {
            pw_messages.push(ToPipewireMessage::UpdateFilterEffects {
                filter_key,
                effects,
            });
        }
        pw_messages
    }

    /// Handle `StateMsg::SetCellEffects` — update link effects and dispatch to cell filter.
    pub(crate) fn handle_set_cell_effects(
        &mut self,
        source: EndpointDescriptor,
        sink: EndpointDescriptor,
        effects: EffectsConfig,
    ) -> Vec<ToPipewireMessage> {
        let mut pw_messages = Vec::new();
        if let Some(l) = self
            .links
            .iter_mut()
            .find(|l| l.start == source && l.end == sink)
        {
            l.cell_effects = effects.clone();
        }
        // Dispatch effects to cell's PW filter
        let filter_key = match (&source, &sink) {
            (EndpointDescriptor::Channel(ch), EndpointDescriptor::Channel(mx)) => {
                format!("{}-to-{}", ch.inner(), mx.inner())
            }
            _ => String::new(),
        };
        if !filter_key.is_empty() {
            pw_messages.push(ToPipewireMessage::UpdateFilterEffects {
                filter_key,
                effects,
            });
        }
        pw_messages
    }

    /// Handle `StateMsg::AssignApp` — assign an app to a channel and park its auto-channel.
    pub(crate) fn handle_assign_app(
        &mut self,
        channel_id: crate::graph::ChannelId,
        assignment: AppAssignment,
        graph: &AudioGraph,
    ) -> Vec<ToPipewireMessage> {
        let mut pw_messages = Vec::new();
        let Some(ch) = self.channels.get_mut(&channel_id) else {
            tracing::warn!("[State] cannot assign app: channel {channel_id:?} not found");
            return pw_messages;
        };

        // Don't add duplicates
        if ch.assigned_apps.contains(&assignment) {
            return pw_messages;
        }

        ch.assigned_apps.push(assignment.clone());
        // ADR-007: Reconciler's diff_app_routing handles actual linking
        // to cell sinks on the next tick. No immediate redirect needed.
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
            // ADR-007: Don't destroy the auto-channel's cells/filters —
            // just unlink apps from them. Cells keep their volume/EQ state
            // and get relinked when the app is ungrouped.
            let prefix = format!("osg.cell.{}-to-", id.inner());
            for (&nid, n) in &graph.nodes {
                if n.identifier
                    .node_name()
                    .is_some_and(|name| name.starts_with(&prefix))
                {
                    for app_node in graph.nodes.values() {
                        if graph
                            .links
                            .values()
                            .any(|l| l.start_node == app_node.id && l.end_node == nid)
                        {
                            pw_messages.push(ToPipewireMessage::RemoveNodeLinks {
                                start_id: app_node.id,
                                end_id: nid,
                            });
                        }
                    }
                    // Set cell volume to 0 so it's silent while parked
                    pw_messages.push(ToPipewireMessage::NodeVolume(nid, vec![0.0, 0.0]));
                }
            }
            // Hide the auto-channel but keep it in the model
            if let Some(ep) = self.endpoints.get_mut(&EndpointDescriptor::Channel(id)) {
                ep.visible = false;
            }
        }
        pw_messages
    }

    /// Handle `StateMsg::UnassignApp` — unassign an app from a channel and restore auto-channel.
    #[allow(clippy::too_many_lines)] // Multi-step teardown/restore logic for app unassignment
    pub(crate) fn handle_unassign_app(
        &mut self,
        channel_id: crate::graph::ChannelId,
        assignment: AppAssignment,
        graph: &AudioGraph,
    ) -> Vec<ToPipewireMessage> {
        let mut pw_messages = Vec::new();
        let Some(ch) = self.channels.get_mut(&channel_id) else {
            tracing::warn!("[State] cannot unassign app: channel {channel_id:?} not found");
            return pw_messages;
        };

        ch.assigned_apps.retain(|a| a != &assignment);

        // ADR-007: Clear links from app streams to all cell sinks for this channel
        let prefix = format!("osg.cell.{}-to-", channel_id.inner());
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
        for node in graph.nodes.values() {
            if node.identifier.application_name.as_deref() == Some(&assignment.application_name)
                && node.identifier.binary_name.as_deref() == Some(&assignment.binary_name)
                && node.has_port_kind(PortKind::Source)
            {
                for &cell_id in &cell_ids {
                    pw_messages.push(ToPipewireMessage::ClearRedirect {
                        stream_node_id: node.id,
                        target_node_id: cell_id,
                    });
                }
                debug!(
                    "[State] cleared redirect for {} (node {})",
                    assignment.application_name, node.id
                );
            }
        }
        // Restore hidden auto-channel if it exists
        let auto_id = self
            .channels
            .iter()
            .find(|(_, c)| {
                c.auto_app
                    && !c.assigned_apps.is_empty()
                    && c.assigned_apps.iter().any(|a| {
                        a.application_name == assignment.application_name
                            && a.binary_name == assignment.binary_name
                    })
            })
            .map(|(id, _)| *id);
        if let Some(id) = auto_id {
            if let Some(ep) = self.endpoints.get_mut(&EndpointDescriptor::Channel(id)) {
                ep.visible = true;
            }
            // Restore cell volumes from endpoint state
            let vol = self
                .endpoints
                .get(&EndpointDescriptor::Channel(id))
                .map(|ep| (ep.volume_left, ep.volume_right))
                .unwrap_or((1.0, 1.0));
            let auto_prefix = format!("osg.cell.{}-to-", id.inner());
            for (&nid, n) in &graph.nodes {
                if n.identifier
                    .node_name()
                    .is_some_and(|name| name.starts_with(&auto_prefix))
                {
                    pw_messages.push(ToPipewireMessage::NodeVolume(nid, vec![vol.0, vol.1]));
                }
            }
        }
        // Force graph update so reconciler relinks
        pw_messages.push(ToPipewireMessage::Update);
        pw_messages
    }

    /// Handle `StateMsg::SetMixOutput` — change a mix's hardware output.
    pub(crate) fn handle_set_mix_output(
        &mut self,
        channel_id: crate::graph::ChannelId,
        output_node_id: Option<u32>,
        graph: &AudioGraph,
    ) -> Vec<ToPipewireMessage> {
        let mut pw_messages = Vec::new();
        let Some(ch) = self.channels.get_mut(&channel_id) else {
            return pw_messages;
        };
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
            pw_messages.push(ToPipewireMessage::SetDefaultSink(name.to_owned(), new_id));
        }
        pw_messages
    }
}
