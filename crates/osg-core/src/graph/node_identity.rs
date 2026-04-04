// Domain projection of a PipeWire node's identifying properties.
//
// This is the domain-owned subset of `pw::NodeIdentifier`. It carries only
// the fields the graph layer needs for matching and display. Infrastructure
// layers convert the full `NodeIdentifier` into this type via `From`.

use serde::{Deserialize, Serialize};

/// Lightweight, domain-owned node identity used for matching and display.
///
/// Unlike the infrastructure `NodeIdentifier`, this type is free of
/// PipeWire-specific cached fields and `OnceLock` internals.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeIdentity {
    pub node_name: Option<String>,
    pub node_nick: Option<String>,
    pub node_description: Option<String>,
    pub object_path: Option<String>,
    pub application_name: Option<String>,
    pub binary_name: Option<String>,
    /// `media.class` — "Audio/Source", "Stream/Output/Audio", etc.
    pub media_class: Option<String>,
    /// `device.api` — "alsa" for hardware, absent for virtual/app.
    pub device_api: Option<String>,
    /// `device.form-factor` — "microphone", "headset", "webcam", "internal", etc.
    pub device_form_factor: Option<String>,
    /// `osg.instance` — ULID stamped by the OSG instance that created this node.
    pub osg_instance: Option<String>,
}

impl NodeIdentity {
    /// Create an empty identity for tests.
    pub fn new_test() -> Self {
        Self::default()
    }

    /// Best human-readable identifier: node_name > object_path > node_description > node_nick.
    pub fn identifier(&self) -> &str {
        self.node_name
            .as_deref()
            .or(self.object_path.as_deref())
            .or(self.node_description.as_deref())
            .or(self.node_nick.as_deref())
            .unwrap_or_default()
    }

    /// Match two identities by comparing the first property that exists on both.
    pub fn matches(&self, other: &NodeIdentity) -> bool {
        let ids = self
            .node_name
            .as_ref()
            .zip(other.node_name.as_ref())
            .or_else(|| self.object_path.as_ref().zip(other.object_path.as_ref()))
            .or_else(|| {
                self.node_description
                    .as_ref()
                    .zip(other.node_description.as_ref())
            })
            .or_else(|| self.node_nick.as_ref().zip(other.node_nick.as_ref()));

        if let Some((left, right)) = ids {
            left == right
        } else {
            false
        }
    }
}
