//! Typed domain events. Each category has its own channel with independent backpressure.

use serde::Serialize;

/// Volume mutations — high frequency, debounced at 16ms.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VolumeCommand {
    SetVolume { node_id: u32, left: f32, right: f32 },
    SetMute { node_id: u32, muted: bool },
}

/// Link/route mutations — medium frequency, no debounce.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LinkCommand {
    CreateRoute { source: u32, target: u32 },
    RemoveRoute { link_id: u32 },
}

/// Node lifecycle — low frequency, no debounce.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NodeCommand {
    CreateSink {
        name: String,
        kind: crate::graph::ChannelKind,
    },
    DestroySink {
        id: u32,
    },
}

/// Stream routing via PipeWire metadata — low frequency.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MetadataCommand {
    RedirectStream {
        stream_id: u32,
        target_sink_id: u32,
    },
    ClearRedirect {
        stream_id: u32,
    },
}

/// Config persistence events — low frequency, debounced at 30s.
#[derive(Debug, Clone)]
pub enum ConfigEvent {
    StateChanged,
    SettingsChanged,
}

/// Domain events emitted by MixerSession. Handlers translate these to PW commands.
/// TODO: Refactor MixerSession::update() to return Vec<MixerEvent> instead of Vec<ToPipewireMessage>.
#[derive(Debug, Clone)]
pub enum MixerEvent {
    VolumeChanged(VolumeCommand),
    LinkRequested(LinkCommand),
    NodeRequested(NodeCommand),
    StreamRedirected(MetadataCommand),
    ConfigChanged(ConfigEvent),
}
