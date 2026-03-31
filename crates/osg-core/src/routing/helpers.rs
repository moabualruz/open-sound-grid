// Helper methods on MixerSession extracted from update.rs to stay under the
// 800-line file limit. These are called by the reducer but are not part of
// the main update() match.

use tracing::debug;

use crate::graph::{
    AppAssignment, Channel, ChannelId, ChannelKind, EndpointDescriptor, Endpoint,
    EqConfig, Link, LinkState, MixerSession,
};
use crate::pw::{AudioGraph, PortKind, ToPipewireMessage};

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

    /// Auto-create pw_filter channels for apps that don't have one yet.
    /// Each app gets its own auto-channel (auto_app=true). Returns PW
    /// messages to create the filter nodes.
    pub fn auto_create_app_channels(&mut self) -> Vec<ToPipewireMessage> {
        let mut messages = Vec::new();

        // Collect apps that output audio (Source kind = app produces audio)
        let output_apps: Vec<_> = self
            .apps
            .values()
            .filter(|app| app.kind == PortKind::Source)
            .map(|app| (app.name.clone(), app.binary.clone(), app.icon_name.clone()))
            .collect();

        for (app_name, binary, icon) in output_apps {
            // Skip if already assigned to a user channel
            let assigned = self.channels.values().any(|ch| {
                !ch.auto_app
                    && ch.assigned_apps.iter().any(|a| {
                        a.application_name == app_name && a.binary_name == binary
                    })
            });
            if assigned {
                continue;
            }

            // Skip if auto-channel already exists
            let has_auto_channel = self.channels.values().any(|ch| {
                ch.auto_app
                    && ch.assigned_apps.iter().any(|a| {
                        a.application_name == app_name && a.binary_name == binary
                    })
            });
            if has_auto_channel {
                continue;
            }

            // Skip OSG's own streams and EasyEffects
            if app_name == "open-sound-grid"
                || app_name.starts_with("osg")
                || binary.contains("easyeffects")
            {
                continue;
            }

            // Create auto-channel
            let id = ChannelId::new();
            let kind = ChannelKind::Duplex;
            let descriptor = EndpointDescriptor::Channel(id);

            self.channels.insert(
                id,
                Channel {
                    id,
                    kind,
                    output_node_id: None,
                    assigned_apps: vec![AppAssignment {
                        application_name: app_name.clone(),
                        binary_name: binary.clone(),
                    }],
                    auto_app: true,
                    allow_app_assignment: false,
                    pipewire_id: None,
                    pending: true,
                },
            );
            self.endpoints.insert(
                descriptor,
                Endpoint::new(descriptor)
                    .with_display_name(app_name.clone())
                    .with_icon_name(icon),
            );

            messages.push(ToPipewireMessage::CreateGroupNode(
                app_name.clone(),
                id.inner(),
                kind,
            ));
            debug!("[State] auto-created channel for app '{app_name}'");
        }

        messages
    }

    /// Ensure every source channel output → every sink mix input is linked.
    /// Uses PW IDs directly — doesn't rely on multi-cycle endpoint resolution.
    pub fn diff_direct_channel_to_mix_links(
        &self,
        graph: &AudioGraph,
    ) -> Vec<ToPipewireMessage> {
        let mut messages = Vec::new();
        let sources: Vec<u32> = self.channels.values()
            .filter(|ch| ch.kind != ChannelKind::Sink)
            .filter_map(|ch| ch.pipewire_id)
            .collect();
        let sinks: Vec<u32> = self.channels.values()
            .filter(|ch| ch.kind == ChannelKind::Sink)
            .filter_map(|ch| ch.pipewire_id)
            .collect();

        for &src in &sources {
            for &sink in &sinks {
                let linked = graph.links.values()
                    .any(|l| l.start_node == src && l.end_node == sink);
                if !linked && graph.nodes.contains_key(&src) && graph.nodes.contains_key(&sink) {
                    messages.push(ToPipewireMessage::CreateNodeLinks {
                        start_id: src,
                        end_id: sink,
                    });
                }
            }
        }
        messages
    }

    /// Ensure every sink mix output → its assigned hardware device is linked.
    pub fn diff_mix_to_hardware_links(
        &self,
        graph: &AudioGraph,
    ) -> Vec<ToPipewireMessage> {
        let mut messages = Vec::new();
        for ch in self.channels.values() {
            if ch.kind != ChannelKind::Sink {
                continue;
            }
            let Some(mix_pw_id) = ch.pipewire_id else { continue };
            // Use output_node_id if set, otherwise find the default sink
            let hw_id = ch.output_node_id.or_else(|| {
                // Default: find the OS default sink node
                let default_name = graph.default_sink_name.as_ref()?;
                graph.nodes.values()
                    .find(|n| n.identifier.node_name() == Some(default_name))
                    .map(|n| n.id)
            });
            let Some(hw_id) = hw_id else { continue };
            let linked = graph.links.values()
                .any(|l| l.start_node == mix_pw_id && l.end_node == hw_id);
            if !linked {
                messages.push(ToPipewireMessage::CreateNodeLinks {
                    start_id: mix_pw_id,
                    end_id: hw_id,
                });
            }
        }
        messages
    }
}
