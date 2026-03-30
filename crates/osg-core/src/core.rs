//! OsgCore — the public entry point for PipeWire orchestration.
//!
//! Owns the PipeWire connection, exposes the AudioGraph read model
//! and a broadcast channel for graph change events.

use std::sync::{Arc, Mutex, mpsc};

use tokio::sync::broadcast;
use tracing::debug;

use crate::pw::{AudioGraph, PipewireHandle, PwError, ToPipewireMessage};

/// The public entry point for PipeWire orchestration.
///
/// Connects to the running PipeWire daemon, watches the graph,
/// and exposes state via snapshot + broadcast channel.
#[allow(missing_debug_implementations)] // Contains PipewireHandle which holds thread handles
pub struct OsgCore {
    _pw_handle: PipewireHandle,
    graph: Arc<Mutex<AudioGraph>>,
    graph_tx: broadcast::Sender<AudioGraph>,
    _pw_sender: mpsc::Sender<ToPipewireMessage>,
}

impl OsgCore {
    /// Connect to PipeWire and start watching the graph.
    ///
    /// The `update_fn` callback fires on every graph change from PipeWire.
    /// Graph snapshots are published to the broadcast channel.
    pub fn new() -> Result<Self, PwError> {
        let (graph_tx, _) = broadcast::channel(64);
        let graph = Arc::new(Mutex::new(AudioGraph::default()));

        let (pw_sender, pw_receiver) = mpsc::channel();

        let graph_clone = graph.clone();
        let tx_clone = graph_tx.clone();
        let pw_sender_clone = pw_sender.clone();

        let pw_handle = PipewireHandle::init((pw_sender_clone, pw_receiver), move |new_graph| {
            let snapshot = *new_graph;
            // Mutex should never be poisoned — only held briefly for snapshot assignment
            #[allow(clippy::unwrap_used)]
            {
                *graph_clone.lock().unwrap() = snapshot.clone();
            }
            // Broadcast to all subscribers (ignore error if no receivers)
            let _ = tx_clone.send(snapshot);
        })?;

        debug!("OsgCore initialized, PipeWire connected");

        Ok(Self {
            _pw_handle: pw_handle,
            graph,
            graph_tx,
            _pw_sender: pw_sender,
        })
    }

    /// Get a snapshot of the current PipeWire graph state.
    pub fn snapshot(&self) -> AudioGraph {
        // Mutex should never be poisoned — only held briefly for snapshot reads
        #[allow(clippy::unwrap_used)]
        self.graph.lock().unwrap().clone()
    }

    /// Subscribe to graph change events.
    /// Each receiver gets every AudioGraph update as it arrives from PipeWire.
    pub fn subscribe(&self) -> broadcast::Receiver<AudioGraph> {
        self.graph_tx.subscribe()
    }

    /// Send a command to the PipeWire thread.
    pub fn send(&self, msg: ToPipewireMessage) -> Result<(), PwError> {
        self._pw_sender.send(msg).map_err(|_| PwError::ThreadExited)
    }
}
