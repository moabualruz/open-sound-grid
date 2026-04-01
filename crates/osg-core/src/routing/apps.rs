// App discovery, auto-channel management, and EasyEffects helpers.
//
// Extracted from reconcile.rs / update.rs to respect the 800-line file limit.

use std::collections::HashMap;

use tracing::debug;

use crate::graph::{
    App, AppAssignment, Channel, ChannelId, ChannelKind, Endpoint, EndpointDescriptor,
    MixerSession, PersistentNodeId, ReconcileSettings,
};
use crate::pw::{AudioGraph, Node as PwNode, PortKind, ToPipewireMessage};

/// EasyEffects processed mic source node name in PipeWire.
const EASYEFFECTS_SOURCE: &str = "easyeffects_source";

/// System binaries that must never get auto-channels.
const BLOCKED_BINARIES: &[&str] = &[
    "easyeffects",
    "kwin",
    "pipewire",
    "wireplumber",
    "xdg-desktop-portal",
    "gnome-shell",
    "pulseaudio",
    "speech-dispatcher",
];

/// App names that must never get auto-channels.
const BLOCKED_APP_NAMES: &[&str] = &["kwin_wayland", "GNOME Shell"];

pub(super) fn is_blocked_app(app_name: &str, binary: &str) -> bool {
    app_name.is_empty()
        || binary.is_empty()
        || app_name == "open-sound-grid"
        || app_name.starts_with("osg")
        || BLOCKED_APP_NAMES.contains(&app_name)
        || BLOCKED_BINARIES.iter().any(|b| binary.contains(b))
}

impl MixerSession {
    pub(super) fn has_channel_for_app(
        &self,
        app_name: &str,
        binary: &str,
        auto_only: bool,
    ) -> bool {
        self.channels.values().any(|ch| {
            (if auto_only { ch.auto_app } else { !ch.auto_app })
                && ch
                    .assigned_apps
                    .iter()
                    .any(|a| a.application_name == app_name && a.binary_name == binary)
        })
    }

    /// Auto-create a real null-audio-sink channel for each discovered app.
    pub(super) fn auto_create_app_channels(&mut self) -> Vec<ToPipewireMessage> {
        let mut messages = Vec::new();
        let output_apps: Vec<_> = self
            .apps
            .values()
            .filter(|app| app.kind == PortKind::Source)
            .map(|app| (app.name.clone(), app.binary.clone(), app.icon_name.clone()))
            .collect();
        for (app_name, binary, icon) in output_apps {
            if is_blocked_app(&app_name, &binary)
                || self.has_channel_for_app(&app_name, &binary, false)
                || self.has_channel_for_app(&app_name, &binary, true)
            {
                continue;
            }
            let id = ChannelId::new();
            let kind = ChannelKind::Duplex;
            let descriptor = EndpointDescriptor::Channel(id);
            // ADR-007: App channels are logical-only — no PW node.
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
                    pending: false,
                },
            );
            self.endpoints.insert(
                descriptor,
                Endpoint::new(descriptor)
                    .with_display_name(app_name.clone())
                    .with_icon_name(icon),
            );
            tracing::debug!("[State] auto-created channel for app '{app_name}'");
        }
        messages
    }

    pub(super) fn discover_apps(&mut self, graph: &crate::pw::AudioGraph) {
        let mut discovered = HashMap::<(String, String, PortKind), String>::new();
        for node in graph.nodes.values() {
            if let (Some(app_name), Some(binary)) = (
                node.identifier
                    .application_name
                    .as_ref()
                    .filter(|n| !n.is_empty()),
                node.identifier
                    .binary_name
                    .as_ref()
                    .filter(|n| !n.is_empty()),
            ) {
                if node.has_port_kind(PortKind::Source) {
                    discovered.insert(
                        (app_name.clone(), binary.clone(), PortKind::Source),
                        node.identifier.icon_name().to_owned(),
                    );
                }
                if node.has_port_kind(PortKind::Sink) {
                    discovered.insert(
                        (app_name.clone(), binary.clone(), PortKind::Sink),
                        node.identifier.icon_name().to_owned(),
                    );
                }
            }
        }
        // Remove existing combinations.
        for app in self.apps.values() {
            discovered.remove(&(app.name.clone(), app.binary.clone(), app.kind));
        }
        // Add new inactive apps.
        for ((name, binary, kind), icon) in discovered {
            tracing::debug!("[State] discovered app '{name}' ({binary}, {kind:?})");
            let app = App::new_inactive(name, binary, icon, kind);
            self.apps.insert(app.id, app);
        }
    }

    /// Get or create a persistent node for the given PW node.
    pub fn get_persistent_node(&mut self, node: &PwNode, kind: PortKind) -> EndpointDescriptor {
        let id = self
            .persistent_nodes
            .iter()
            .find(|(_, (identifier, nk))| *nk == kind && identifier.matches(&node.identifier))
            .map(|(id, _)| *id);

        if let Some(id) = id {
            EndpointDescriptor::PersistentNode(id, kind)
        } else {
            let id = PersistentNodeId::new();
            self.persistent_nodes
                .insert(id, (node.identifier.clone(), kind));
            EndpointDescriptor::PersistentNode(id, kind)
        }
    }

    /// Rename EasyEffects channels to match the current default mic.
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

    /// Find PipeWire cell node IDs for a route by scanning for `osg.cell.{ch}.{mix}` names.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn find_cell_node_ids(
        &self,
        source: EndpointDescriptor,
        sink: EndpointDescriptor,
        graph: &AudioGraph,
        settings: &ReconcileSettings,
    ) -> Vec<u32> {
        let source_nodes = self
            .resolve_endpoint(source, graph, settings)
            .unwrap_or_default();
        let sink_nodes = self
            .resolve_endpoint(sink, graph, settings)
            .unwrap_or_default();
        let mut ids = Vec::new();
        for s in &source_nodes {
            for k in &sink_nodes {
                let src_u = if let EndpointDescriptor::Channel(id) = source {
                    id.inner().to_string()
                } else {
                    s.id.to_string()
                };
                let snk_u = if let EndpointDescriptor::Channel(id) = sink {
                    id.inner().to_string()
                } else {
                    k.id.to_string()
                };
                let ulid_name = format!("osg.cell.{src_u}-to-{snk_u}");
                let legacy_name = format!("osg.cell.{}.{}", s.id, k.id);
                if let Some((&cell_id, _)) = graph.nodes.iter().find(|(_, n)| {
                    let nn = n.identifier.node_name();
                    nn == Some(&ulid_name) || nn == Some(&legacy_name)
                }) {
                    ids.push(cell_id);
                }
            }
        }
        ids
    }
}
