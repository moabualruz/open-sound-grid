// App discovery, auto-channel management, and EasyEffects helpers.
//
// Extracted from reconcile.rs / update.rs to respect the 800-line file limit.

use std::collections::HashMap;

use tracing::debug;

use crate::graph::{
    App, AppAssignment, Channel, ChannelId, ChannelKind, Endpoint, EndpointDescriptor,
    MixerSession, PersistentNodeId, ReconcileSettings, SourceType,
};
use crate::pw::{AudioGraph, Node as PwNode, PortKind, ToPipewireMessage};
use crate::pw::identifier::NodeIdentifier;

/// Detect source type from PipeWire node properties.
fn detect_source_type(id: &NodeIdentifier) -> SourceType {
    let node_name = id.node_name().unwrap_or("");
    // Our own nodes
    if node_name.starts_with("osg.") {
        return SourceType::AppStream;
    }
    // EasyEffects virtual source
    if node_name.starts_with("easyeffects_") {
        return SourceType::VirtualSource;
    }
    // Hardware ALSA input
    if id.device_api.as_deref() == Some("alsa") && node_name.starts_with("alsa_input") {
        return match id.device_form_factor.as_deref() {
            Some("microphone") | Some("headset") | Some("webcam") | Some("internal") => {
                SourceType::HardwareMic
            }
            _ => SourceType::HardwareLineIn,
        };
    }
    SourceType::AppStream
}

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

    /// Auto-create a logical channel for each discovered app.
    pub(super) fn auto_create_app_channels(
        &mut self,
        graph: &crate::pw::AudioGraph,
    ) -> Vec<ToPipewireMessage> {
        let messages = Vec::new();
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
            // Detect source type from PW node properties
            let source_type = graph
                .nodes
                .values()
                .find(|n| {
                    n.identifier.application_name.as_deref() == Some(&app_name)
                        && n.identifier.binary_name.as_deref() == Some(&binary)
                })
                .map(|n| detect_source_type(&n.identifier))
                .unwrap_or_default();

            // ADR-007: App channels are logical-only — no PW node.
            self.channels.insert(
                id,
                Channel {
                    id,
                    kind,
                    source_type,
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
    #[allow(clippy::too_many_arguments, clippy::unused_self)]
    /// Find PW node IDs of cell sinks for a (source, sink) endpoint pair.
    /// ADR-007: Uses ULID-based naming, no resolve_endpoint needed.
    pub(super) fn find_cell_node_ids(
        &self,
        source: EndpointDescriptor,
        sink: EndpointDescriptor,
        graph: &AudioGraph,
        _settings: &ReconcileSettings,
    ) -> Vec<u32> {
        let src_u = match source {
            EndpointDescriptor::Channel(id) => id.inner().to_string(),
            _ => return Vec::new(),
        };
        let snk_u = match sink {
            EndpointDescriptor::Channel(id) => id.inner().to_string(),
            _ => return Vec::new(),
        };
        let cell_name = format!("osg.cell.{src_u}-to-{snk_u}");
        graph
            .nodes
            .iter()
            .filter(|(_, n)| n.identifier.node_name() == Some(&cell_name))
            .map(|(&id, _)| id)
            .collect()
    }
}
