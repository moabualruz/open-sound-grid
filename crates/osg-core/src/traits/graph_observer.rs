//! GraphObserver — focused trait for reading the PipeWire audio graph.

use tokio::sync::broadcast;

use crate::pw::AudioGraph;

/// Focused trait for read access to the PipeWire audio graph (read model).
///
/// Consumers depend on this trait, not on OsgCore directly.
pub trait GraphObserver {
    /// Get a point-in-time snapshot of the current PipeWire graph state.
    fn snapshot(&self) -> AudioGraph;

    /// Subscribe to graph change events. Each graph update is broadcast
    /// to all active receivers.
    fn subscribe(&self) -> broadcast::Receiver<AudioGraph>;
}
