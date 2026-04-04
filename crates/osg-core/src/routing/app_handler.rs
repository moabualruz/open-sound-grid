// App-assignment command handlers extracted from eq_handlers.rs.
//
// Handles: AssignApp, UnassignApp (staging sink choreography)

use tracing::debug;

use crate::graph::events::MixerEvent;
use crate::graph::{AppAssignment, ChannelId, EndpointDescriptor, MixerSession, RuntimeState};
use crate::pw::{AudioGraph, PortKind};

/// Build `CreateNodeLinks` events linking each stream to the staging sink.
fn stage_streams(stream_ids: &[u32], staging_id: u32) -> Vec<MixerEvent> {
    stream_ids
        .iter()
        .map(|&id| MixerEvent::CreateNodeLinks {
            start_id: id,
            end_id: staging_id,
        })
        .collect()
}

/// Build `RemoveNodeLinks` events unlinking each stream from the staging sink.
fn unstage_streams(stream_ids: &[u32], staging_id: u32) -> Vec<MixerEvent> {
    stream_ids
        .iter()
        .map(|&id| MixerEvent::RemoveNodeLinks {
            start_id: id,
            end_id: staging_id,
        })
        .collect()
}

/// Collect PipeWire node IDs for all source streams belonging to an app assignment.
fn collect_app_stream_ids(assignment: &AppAssignment, graph: &AudioGraph) -> Vec<u32> {
    graph
        .nodes
        .values()
        .filter(|n| {
            n.identifier.application_name.as_deref() == Some(&assignment.application_name)
                && n.identifier.binary_name.as_deref() == Some(&assignment.binary_name)
                && n.has_port_kind(PortKind::Source)
        })
        .map(|n| n.id)
        .collect()
}

impl MixerSession {
    /// Handle `StateMsg::AssignApp` — assign an app to a channel and park its auto-channel.
    ///
    /// Uses the staging sink for glitch-free rerouting: before unlinking from
    /// old cell sinks, each app stream is linked to the staging sink (vol=0)
    /// so it always has an output destination. The reconciler links to new
    /// cell sinks on the next tick, then the staging link is cleaned up.
    #[allow(clippy::too_many_lines)] // Multi-step staging + unlink + restore logic
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_assign_app(
        &mut self,
        channel_id: ChannelId,
        assignment: AppAssignment,
        graph: &AudioGraph,
        rt: &RuntimeState,
    ) -> Vec<MixerEvent> {
        let mut events = Vec::new();
        let Some(ch) = self.channels.get_mut(&channel_id) else {
            tracing::warn!("[State] cannot assign app: channel {channel_id:?} not found");
            return events;
        };

        // Don't add duplicates
        if ch.assigned_apps.contains(&assignment) {
            return events;
        }

        ch.assigned_apps.push(assignment.clone());

        // Collect app stream node IDs for staging sink linking
        let app_stream_ids = collect_app_stream_ids(&assignment, graph);

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
            // Staging sink: link app streams to staging BEFORE unlinking from old cells.
            // This prevents audio glitches from having no output destination.
            if let Some(staging_id) = rt.staging_node_id {
                events.extend(stage_streams(&app_stream_ids, staging_id));
            }

            // ADR-007: Don't destroy the auto-channel's cells/filters —
            // just unlink apps from them. Cells keep their volume/EQ state
            // and get relinked when the app is ungrouped.
            let prefix = format!("osg.cell.{}-to-", id.inner());
            for (&nid, n) in &graph.nodes {
                if n.identifier
                    .node_name()
                    .is_some_and(|name| name.starts_with(&prefix))
                {
                    for &stream_id in &app_stream_ids {
                        if graph
                            .links
                            .values()
                            .any(|l| l.start_node == stream_id && l.end_node == nid)
                        {
                            events.push(MixerEvent::RemoveNodeLinks {
                                start_id: stream_id,
                                end_id: nid,
                            });
                        }
                    }
                    // Set cell volume to 0 so it's silent while parked
                    events.push(MixerEvent::VolumeChanged {
                        node_id: nid,
                        channels: vec![0.0, 0.0],
                    });
                }
            }

            // Remove staging links — the reconciler will link to new cells
            if let Some(staging_id) = rt.staging_node_id {
                events.extend(unstage_streams(&app_stream_ids, staging_id));
            }

            // Hide the auto-channel but keep it in the model
            if let Some(ep) = self.endpoints.get_mut(&EndpointDescriptor::Channel(id)) {
                ep.visible = false;
            }
        }
        events
    }

    /// Handle `StateMsg::UnassignApp` — unassign an app from a channel and restore auto-channel.
    #[allow(clippy::too_many_lines)] // Multi-step teardown/restore logic for app unassignment
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_unassign_app(
        &mut self,
        channel_id: ChannelId,
        assignment: AppAssignment,
        graph: &AudioGraph,
        rt: &RuntimeState,
    ) -> Vec<MixerEvent> {
        let mut events = Vec::new();
        let Some(ch) = self.channels.get_mut(&channel_id) else {
            tracing::warn!("[State] cannot unassign app: channel {channel_id:?} not found");
            return events;
        };

        ch.assigned_apps.retain(|a| a != &assignment);

        // Collect app stream node IDs for staging sink linking
        let app_stream_ids = collect_app_stream_ids(&assignment, graph);

        // Staging sink: link app streams to staging BEFORE clearing old redirects
        if let Some(staging_id) = rt.staging_node_id {
            events.extend(stage_streams(&app_stream_ids, staging_id));
        }

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
        for &stream_id in &app_stream_ids {
            for &cell_id in &cell_ids {
                events.push(MixerEvent::ClearRedirect {
                    stream_node_id: stream_id,
                    target_node_id: cell_id,
                });
            }
            debug!(
                "[State] cleared redirect for {} (node {stream_id})",
                assignment.application_name
            );
        }

        // Remove staging links — the auto-channel restore or reconciler handles new links
        if let Some(staging_id) = rt.staging_node_id {
            events.extend(unstage_streams(&app_stream_ids, staging_id));
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
                    events.push(MixerEvent::VolumeChanged {
                        node_id: nid,
                        channels: vec![vol.0, vol.1],
                    });
                }
            }
        }
        // Force graph update so reconciler relinks
        events.push(MixerEvent::RequestReconciliation);
        events
    }
}
