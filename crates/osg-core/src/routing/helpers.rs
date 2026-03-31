// Helper methods on MixerSession extracted from update.rs to stay under the
// 800-line file limit. These are called by the reducer but are not part of
// the main update() match.

use tracing::debug;

use crate::graph::{
    ChannelKind, EndpointDescriptor, EqConfig, Link, LinkState, MixerSession,
};
use crate::pw::{AudioGraph, PortKind};

const EASYEFFECTS_SOURCE: &str = "easyeffects_source";

impl MixerSession {
    /// Auto-rename channels created from `easyeffects_source` before the
    /// default source name was resolved. Called on graph updates.
    pub fn rename_easyeffects_channels(&mut self, graph: &AudioGraph) {
        let Some(ref source_name) = graph.default_source_name else {
            return;
        };

        let ee_exists = graph
            .nodes
            .values()
            .any(|n| n.identifier.node_name() == Some(EASYEFFECTS_SOURCE));
        if !ee_exists {
            return;
        }

        let source_display = graph
            .nodes
            .values()
            .find(|n| n.identifier.node_name() == Some(source_name))
            .map(|n| n.identifier.human_name(PortKind::Source).to_owned())
            .unwrap_or_else(|| source_name.clone());

        let new_name = format!("EE - {source_display}");

        for ep in self.endpoints.values_mut() {
            let is_legacy = ep.display_name == "Easy Effects Source"
                || ep.display_name == "easyeffects_source"
                || ep.display_name == "Mic (EasyEffects)"
                || ep.display_name == "EE - Mic"
                || (ep.display_name.starts_with("EE - ") && ep.display_name != new_name)
                || (ep.display_name.ends_with("(EasyEffects)") && ep.display_name != new_name);
            if is_legacy {
                debug!(
                    "[State] auto-rename EasyEffects channel: {:?} -> {new_name:?}",
                    ep.display_name
                );
                ep.display_name = new_name.clone();
            }
        }
    }

    /// Auto-create ConnectedLocked links for every source × sink pair.
    /// With pw_filter nodes, WirePlumber doesn't auto-connect so we create
    /// default links ourselves. Called by reconcile::diff().
    pub fn ensure_default_links(&mut self) {
        let sources: Vec<_> = self
            .channels
            .iter()
            .filter(|(_, ch)| ch.kind != ChannelKind::Sink && ch.pipewire_id.is_some())
            .map(|(id, _)| EndpointDescriptor::Channel(*id))
            .collect();
        let sinks: Vec<_> = self
            .channels
            .iter()
            .filter(|(_, ch)| ch.kind == ChannelKind::Sink && ch.pipewire_id.is_some())
            .map(|(id, _)| EndpointDescriptor::Channel(*id))
            .collect();

        for source in &sources {
            for sink in &sinks {
                let exists = self
                    .links
                    .iter()
                    .any(|l| l.start == *source && l.end == *sink);
                if !exists {
                    self.links.push(Link {
                        start: *source,
                        end: *sink,
                        state: LinkState::ConnectedLocked,
                        cell_volume: 1.0,
                        cell_volume_left: 1.0,
                        cell_volume_right: 1.0,
                        cell_eq: EqConfig::default(),
                        cell_node_id: None,
                        pending: false,
                    });
                }
            }
        }
    }
}
