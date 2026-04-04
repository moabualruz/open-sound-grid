// Channel, App, Device, SourceType, AppAssignment domain types.

use serde::{Deserialize, Serialize};

use super::endpoint::EndpointDescriptor;
use super::identifiers::AppId;
use super::node_identity::NodeIdentity;
use super::port_kind::PortKind;

/// The kind of virtual audio bus. Owned by the domain layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChannelKind {
    Source,
    Sink,
    Duplex,
}

fn default_true() -> bool {
    true
}

// ---------------------------------------------------------------------------
// AppAssignment — persistent app-to-channel binding
// ---------------------------------------------------------------------------

/// Identifies an application for routing. Matched against PipeWire node properties
/// `application.name` and `application.process.binary`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppAssignment {
    pub application_name: String,
    pub binary_name: String,
}

// ---------------------------------------------------------------------------
// SourceType — detected from PipeWire node properties
// ---------------------------------------------------------------------------

/// Classifies the audio source type for contextual EQ/effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceType {
    /// Real ALSA microphone (device.api=alsa, form-factor=microphone/headset/webcam).
    HardwareMic,
    /// ALSA line-in (not a microphone).
    HardwareLineIn,
    /// Virtual source (EasyEffects, loopback).
    VirtualSource,
    /// App playback stream (browser, game, music).
    #[default]
    AppStream,
}

// ---------------------------------------------------------------------------
// Channel — user-created virtual audio bus
// ---------------------------------------------------------------------------

/// Virtual audio bus — either user-created or auto-created for an app.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub id: super::identifiers::ChannelId,
    pub kind: ChannelKind,
    /// Detected source type — determines which EQ presets and effects are shown.
    #[serde(default)]
    pub source_type: SourceType,
    /// For sink channels (mixes): the assigned output device node ID.
    pub output_node_id: Option<u32>,
    /// Apps assigned to this channel. Their PW streams are redirected here.
    #[serde(default)]
    pub assigned_apps: Vec<AppAssignment>,
    /// True if this channel was auto-created for a single app.
    /// Auto-channels: no grouping, no manual app assignment from UI,
    /// dissolved when the app is assigned to a user channel.
    #[serde(default)]
    pub auto_app: bool,
    /// False for input device channels and EasyEffects channels —
    /// prevents users from assigning apps to them.
    #[serde(default = "default_true")]
    pub allow_app_assignment: bool,
}

// ---------------------------------------------------------------------------
// App — running application emitting audio
// ---------------------------------------------------------------------------

/// Running application emitting audio. PipeWire: Client/Stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct App {
    pub id: AppId,
    pub kind: PortKind,
    pub name: String,
    pub binary: String,
    pub icon_name: String,
    pub exceptions: Vec<EndpointDescriptor>,
}

impl App {
    pub fn new_inactive(
        application_name: String,
        binary: String,
        icon_name: String,
        kind: PortKind,
    ) -> Self {
        Self {
            id: AppId::new(),
            kind,
            name: application_name,
            binary,
            icon_name,
            exceptions: Vec::new(),
        }
    }

    pub fn matches(&self, identifier: &NodeIdentity, kind: PortKind) -> bool {
        self.kind == kind
            && identifier.application_name.as_ref() == Some(&self.name)
            && identifier.binary_name.as_ref() == Some(&self.binary)
    }

    pub fn name_with_tag(&self) -> String {
        format!("[App] {}", self.name)
    }
}

// ---------------------------------------------------------------------------
// Device — placeholder for future use
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device;
