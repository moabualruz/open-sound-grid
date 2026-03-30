//! SOLID interface traits. Handlers and consumers depend on these, not concrete types.

use crate::pw::PwError;

/// Single-responsibility handler for one event category (SOLID: S + O).
/// New command categories = new handler implementing this trait.
pub trait EventHandler<E>: Send + Sync {
    /// Process one event. Returns Ok on success, Err on unrecoverable failure.
    fn handle(&self, event: E) -> Result<(), PwError>;
}

/// Manages virtual audio bus creation/destruction (PipeWire: null-audio-sink).
pub trait SinkManager: Send + Sync {
    fn create_sink(&self, name: &str, kind: crate::graph::ChannelKind) -> Result<u32, PwError>;
    fn destroy_sink(&self, id: u32) -> Result<(), PwError>;
}

/// Manages connections between channels and mixes (PipeWire: links).
pub trait Router: Send + Sync {
    fn create_route(&self, source: u32, target: u32) -> Result<u32, PwError>;
    fn remove_route(&self, link_id: u32) -> Result<(), PwError>;
    fn redirect_stream(&self, stream_id: u32, sink_id: u32) -> Result<(), PwError>;
}

/// Per-channel L/R stereo volume control (PipeWire: SPA Props channelVolumes).
pub trait VolumeControl: Send + Sync {
    fn set_volume(&self, node_id: u32, left: f32, right: f32) -> Result<(), PwError>;
    fn set_mute(&self, node_id: u32, muted: bool) -> Result<(), PwError>;
}

/// Subscribe to PipeWire graph state changes.
pub trait GraphObserver: Send + Sync {
    type Event;
    fn subscribe(&self) -> tokio::sync::broadcast::Receiver<Self::Event>;
    fn snapshot(&self) -> crate::pw::AudioGraph;
}

// ---------------------------------------------------------------------------
// Transport-agnostic service traits
// ---------------------------------------------------------------------------

/// Transport-agnostic volume service. WebSocket, JSON-RPC, or REST adapters wrap this.
pub trait VolumeService: Send + Sync {
    fn set_volume(&self, node_id: u32, left: f32, right: f32) -> Result<(), PwError>;
    fn subscribe_volume_events(&self) -> tokio::sync::broadcast::Receiver<VolumeEvent>;
}

/// Transport-agnostic graph service.
pub trait GraphService: Send + Sync {
    fn snapshot(&self) -> crate::pw::AudioGraph;
    fn subscribe_graph_events(&self) -> tokio::sync::broadcast::Receiver<GraphEvent>;
}

/// Transport-agnostic routing service.
pub trait RoutingService: Send + Sync {
    fn create_route(&self, source: u32, target: u32) -> Result<u32, PwError>;
    fn remove_route(&self, link_id: u32) -> Result<(), PwError>;
}

// ---------------------------------------------------------------------------
// Event types used by service traits
// ---------------------------------------------------------------------------

/// Volume change event broadcast to UI.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeEvent {
    pub node_id: u32,
    pub left: f32,
    pub right: f32,
    pub muted: bool,
}

/// Graph change event broadcast to UI.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEvent {
    pub kind: GraphEventKind,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GraphEventKind {
    NodeAdded { id: u32 },
    NodeRemoved { id: u32 },
    LinkAdded { id: u32 },
    LinkRemoved { id: u32 },
}
