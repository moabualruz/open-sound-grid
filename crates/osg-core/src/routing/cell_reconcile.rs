// Cell-specific reconciliation functions extracted from reconcile.rs.
//
// These handle the ADR-007 cell node lifecycle:
//   * `diff_cells`      — ensure every (source channel × mix) pair has a cell node
//   * `diff_app_routing` — link assigned app streams to cell sinks
//   * `diff_cell_links`  — ensure cell_sink monitor → [filter] → mix links
//   * `ensure_link`      — helper to emit CreateNodeLinks if missing

use std::collections::{HashMap, HashSet};

use crate::graph::{ChannelKind, EndpointDescriptor, MixerSession};
use crate::pw::{AudioGraph, PortKind, ToPipewireMessage};

impl MixerSession {
    // diff_cells — ensure every row×mix pair has a cell node

    /// For every (source channel × sink mix) pair, ensure a cell node exists.
    pub(crate) fn diff_cells(&mut self, graph: &AudioGraph) -> Vec<ToPipewireMessage> {
        let mut messages = Vec::new();
        let rows: Vec<_> = self
            .channels
            .iter()
            .filter(|(_, ch)| ch.kind != ChannelKind::Sink)
            .collect();
        let mixes: Vec<_> = self
            .channels
            .iter()
            .filter(|(_, ch)| ch.kind == ChannelKind::Sink && ch.pipewire_id.is_some())
            .collect();
        // ADR-007: Source channels are logical. Cell sinks keyed by (ch_ulid, mx_ulid).
        // Collect existing cell names from the graph to avoid duplicates.
        let existing_cells: HashSet<&str> = graph
            .nodes
            .values()
            .filter_map(|n| n.identifier.node_name())
            .filter(|name| name.starts_with("osg.cell."))
            .collect();
        for (row_id, _row_ch) in &rows {
            for (mix_id, _mix_ch) in &mixes {
                let cell_id = format!("osg.cell.{}-to-{}", row_id.inner(), mix_id.inner());
                if !self.created_cells.contains(&cell_id)
                    && !existing_cells.contains(cell_id.as_str())
                {
                    self.created_cells.insert(cell_id.clone());
                    let rn = self
                        .endpoints
                        .get(&EndpointDescriptor::Channel(**row_id))
                        .map(|e| e.display_name.as_str())
                        .unwrap_or("?");
                    let mn = self
                        .endpoints
                        .get(&EndpointDescriptor::Channel(**mix_id))
                        .map(|e| e.display_name.as_str())
                        .unwrap_or("?");
                    messages.push(ToPipewireMessage::CreateCellNode {
                        name: format!("{rn}→{mn}"),
                        cell_id,
                        channel_ulid: row_id.inner().to_string(),
                        mix_ulid: mix_id.inner().to_string(),
                    });
                }
            }
        }
        messages
    }
    // diff_app_routing — link assigned app streams directly to cell sinks

    /// Link each assigned app stream node to all matching cell sinks.
    /// Source channels are logical-only (ADR-007).
    #[allow(clippy::too_many_lines)] // Match-driven routing logic for app→cell linking
    pub(crate) fn diff_app_routing(&self, graph: &AudioGraph) -> Vec<ToPipewireMessage> {
        let mut messages = Vec::new();

        for (channel_id, channel) in &self.channels {
            if channel.kind == ChannelKind::Sink {
                continue;
            }
            if channel.assigned_apps.is_empty() {
                continue;
            }
            // Skip hidden auto-channels (parked while app is grouped)
            let ep_desc = EndpointDescriptor::Channel(*channel_id);
            if self.endpoints.get(&ep_desc).is_some_and(|ep| !ep.visible) {
                continue;
            }

            // Collect all cell sink PW node IDs for this channel.
            let channel_ulid = channel_id.inner().to_string();
            let cell_prefix = format!("osg.cell.{channel_ulid}-to-");
            let cell_sink_ids: Vec<u32> = graph
                .nodes
                .iter()
                .filter_map(|(&id, n)| {
                    let name = n.identifier.node_name()?;
                    name.starts_with(&cell_prefix).then_some(id)
                })
                .collect();

            if cell_sink_ids.is_empty() {
                continue;
            }

            for assignment in &channel.assigned_apps {
                for app_node in graph.nodes.values() {
                    if app_node.identifier.application_name.as_deref()
                        != Some(&assignment.application_name)
                        || app_node.identifier.binary_name.as_deref()
                            != Some(&assignment.binary_name)
                        || !app_node.has_port_kind(PortKind::Source)
                    {
                        continue;
                    }
                    // Check which cells need linking
                    let mut missing: Vec<u32> = Vec::new();
                    for &cell_id in &cell_sink_ids {
                        let already_linked = graph
                            .links
                            .values()
                            .any(|link| link.start_node == app_node.id && link.end_node == cell_id);
                        if !already_linked {
                            missing.push(cell_id);
                        }
                    }
                    if !missing.is_empty() {
                        // First: redirect to cell[0] which removes WP stale links
                        messages.push(ToPipewireMessage::RedirectStream {
                            stream_node_id: app_node.id,
                            target_node_id: missing[0],
                        });
                        // Then: create links to remaining cells (RedirectStream already handles [0])
                        for &cell_id in &missing[1..] {
                            messages.push(ToPipewireMessage::CreateNodeLinks {
                                start_id: app_node.id,
                                end_id: cell_id,
                            });
                        }
                    }
                }
            }
        }
        messages
    }
    // diff_cell_links — ensure cell_sink monitor → [filter →] mix links exist

    /// ADR-007: source channels are logical-only. The audio chain per cell is:
    ///   app stream → cell_sink (volume node)
    ///   cell_sink monitor → [osg.filter.{ch_ulid}-to-{mix_ulid} →] mix_sink
    ///
    /// This function ensures the monitor-out side of the chain exists.
    /// App→cell links are handled by `diff_app_routing`.
    pub(crate) fn diff_cell_links(&self, graph: &AudioGraph) -> Vec<ToPipewireMessage> {
        let mut messages = Vec::new();

        // Build ULID → PW node ID map for sink (mix) channels only.
        let mix_ulid_to_pw: HashMap<String, u32> = self
            .channels
            .iter()
            .filter_map(|(id, ch)| {
                (ch.kind == ChannelKind::Sink).then_some(())?;
                ch.pipewire_id.map(|pw| (id.inner().to_string(), pw))
            })
            .collect();

        // Build filter lookup: "{ch_ulid}-to-{mix_ulid}" → filter PW node ID.
        // Key format matches `osg.filter.{channel_ulid}-to-{mix_ulid}`.
        let filter_pw: HashMap<String, u32> = graph
            .nodes
            .iter()
            .filter_map(|(&id, n)| {
                let name = n.identifier.node_name()?;
                let key = name.strip_prefix("osg.filter.")?.to_owned();
                Some((key, id))
            })
            .collect();

        for (&cell_pw_id, cell_node) in &graph.nodes {
            let Some(name) = cell_node.identifier.node_name() else {
                continue;
            };
            let Some(rest) = name.strip_prefix("osg.cell.") else {
                continue;
            };
            if cell_node.ports.is_empty() {
                continue;
            }
            // Parse "osg.cell.{ch_ulid}-to-{mix_ulid}"
            let Some((ch_ulid, mix_ulid)) = rest.split_once("-to-") else {
                continue;
            };
            let Some(&mix_pw) = mix_ulid_to_pw.get(mix_ulid) else {
                continue;
            };

            let filter_key = format!("{ch_ulid}-to-{mix_ulid}");
            if let Some(&filter_id) = filter_pw.get(&filter_key) {
                // cell_sink monitor → filter → mix_sink
                messages.extend(Self::ensure_link(graph, cell_pw_id, filter_id));
                messages.extend(Self::ensure_link(graph, filter_id, mix_pw));
            } else {
                // No filter: cell_sink monitor → mix_sink directly
                messages.extend(Self::ensure_link(graph, cell_pw_id, mix_pw));
            }
        }

        // Link mix filters between mix monitor and hardware output
        for (ch_id, ch) in &self.channels {
            if ch.kind != ChannelKind::Sink {
                continue;
            }
            let Some(mix_pw) = ch.pipewire_id else {
                continue;
            };
            let mix_filter_key = format!("mix.{}", ch_id.inner());
            if let Some(&filter_id) = filter_pw.get(&mix_filter_key) {
                // Find hardware output for this mix
                let hw_id = ch.output_node_id.or(self.default_output_node_id);
                if let Some(hw) = hw_id {
                    // mix_sink → mix_filter → hardware
                    messages.extend(Self::ensure_link(graph, mix_pw, filter_id));
                    messages.extend(Self::ensure_link(graph, filter_id, hw));
                }
            }
        }

        messages
    }

    /// Populate `cell_node_id` on each link by looking up the cell sink PW node
    /// from the graph. Cell sinks are named `osg.cell.{ch_ulid}-to-{mix_ulid}`.
    pub(crate) fn resolve_cell_node_ids(&mut self, graph: &AudioGraph) {
        for link in &mut self.links {
            let src_u = match link.start {
                EndpointDescriptor::Channel(id) => id.inner().to_string(),
                _ => continue,
            };
            let snk_u = match link.end {
                EndpointDescriptor::Channel(id) => id.inner().to_string(),
                _ => continue,
            };
            let cell_name = format!("osg.cell.{src_u}-to-{snk_u}");
            link.cell_node_id = graph
                .nodes
                .iter()
                .find(|(_, n)| n.identifier.node_name() == Some(&cell_name))
                .map(|(&id, _)| id);
        }
    }

    /// Emit a `CreateNodeLinks` if `from → to` link is missing.
    pub(crate) fn ensure_link(graph: &AudioGraph, from: u32, to: u32) -> Option<ToPipewireMessage> {
        let exists = graph
            .links
            .values()
            .any(|l| l.start_node == from && l.end_node == to);
        if !exists && graph.nodes.contains_key(&from) && graph.nodes.contains_key(&to) {
            Some(ToPipewireMessage::CreateNodeLinks {
                start_id: from,
                end_id: to,
            })
        } else {
            None
        }
    }
}
