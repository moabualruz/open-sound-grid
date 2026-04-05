// Adapted from Sonusmix (MPL-2.0) — https://codeberg.org/sonusmix/sonusmix
//
// The correction loop: diff the desired state (`MixerSession`) against the
// PipeWire reality (`AudioGraph`) and emit `MixerEvent` domain events to bring
// reality in line with intent.
//
// Key concepts:
//   * `diff_nodes`      — resolve endpoints to PW nodes, mark placeholders
//   * `diff_channels`   — ensure virtual channels exist in PW
//   * `diff_properties` — reconcile volume / mute between desired & actual
//   * `diff_links`      — reconcile connections, respecting lock state

mod diff_links;
mod diff_nodes;
mod diff_properties;
mod helpers;
mod resolve_endpoint;

use tracing::warn;

use crate::graph::runtime::MAX_CONSECUTIVE_RECONCILIATIONS;
use crate::graph::{MixerEvent, MixerSession, ReconcileSettings, RuntimeState};
use crate::pw::AudioGraph;

// ---------------------------------------------------------------------------
// ReconciliationService — stateless domain service
// ---------------------------------------------------------------------------

/// Stateless domain service. Reads MixerSession + AudioGraph, emits corrective commands.
/// PipeWire: no equivalent — this is our domain reconciliation logic.
#[allow(missing_debug_implementations)] // Stateless service, no fields to debug
pub struct ReconciliationService;

impl ReconciliationService {
    /// Compare desired state against PipeWire reality and produce corrective commands.
    ///
    /// Tracks consecutive non-empty reconciliation passes via `RuntimeState`.
    /// If the counter exceeds `MAX_CONSECUTIVE_RECONCILIATIONS`, returns empty
    /// to break potential oscillation loops where PW events echo back diffs.
    pub fn reconcile(
        state: &mut MixerSession,
        graph: &AudioGraph,
        settings: &ReconcileSettings,
        rt: &mut RuntimeState,
    ) -> Vec<MixerEvent> {
        if rt.consecutive_reconciliations >= MAX_CONSECUTIVE_RECONCILIATIONS {
            warn!(
                "[Reconcile] skipping — {} consecutive passes exceeded limit",
                rt.consecutive_reconciliations
            );
            rt.consecutive_reconciliations = 0;
            return vec![];
        }
        let events = state.diff(graph, settings, rt);
        if events.is_empty() {
            rt.consecutive_reconciliations = 0;
        } else {
            rt.consecutive_reconciliations += 1;
        }
        events
    }
}

// ---------------------------------------------------------------------------
// Top-level diff entry point
// ---------------------------------------------------------------------------

impl MixerSession {
    /// Run the full reconciliation pass. Returns domain events for the infrastructure layer.
    pub fn diff(
        &mut self,
        graph: &AudioGraph,
        settings: &ReconcileSettings,
        rt: &mut RuntimeState,
    ) -> Vec<MixerEvent> {
        let endpoint_nodes = self.diff_nodes(graph, settings, rt);
        let mut messages = self.auto_create_app_channels(graph, rt);
        self.ensure_default_links();
        messages.extend(self.diff_channels(&endpoint_nodes, graph, rt));
        messages.extend(self.diff_cells(graph, rt));
        self.resolve_cell_node_ids(graph);
        messages.extend(self.diff_cell_links(graph, rt));
        messages.extend(self.diff_app_routing(graph));
        messages.extend(self.diff_properties(&endpoint_nodes, rt));
        messages.extend(self.diff_links(graph, &endpoint_nodes, rt));
        messages
    }
}
