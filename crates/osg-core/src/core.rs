//! OsgCore — the public entry point for PipeWire orchestration.
//!
//! Owns the PipeWire connection and the MixerSession reducer.
//! Exposes the AudioGraph read model, MixerSession write model,
//! and a command API for mutations.

use std::sync::{Arc, Mutex, mpsc};

use tokio::sync::broadcast;
use tracing::debug;

use crate::graph::ReconcileSettings;
use crate::pw::{AudioGraph, PipewireHandle, PwError};
use crate::routing::{ReducerHandle, StateMsg, debounced_graph_sender, run_reducer};

/// The public entry point for PipeWire orchestration.
///
/// Connects to the running PipeWire daemon, runs the MixerSession
/// reducer, and exposes state via snapshot + broadcast channels.
#[allow(missing_debug_implementations)]
pub struct OsgCore {
    _pw_handle: PipewireHandle,
    graph: Arc<Mutex<AudioGraph>>,
    graph_tx: broadcast::Sender<AudioGraph>,
    reducer: ReducerHandle,
}

impl OsgCore {
    /// Connect to PipeWire and start the reducer.
    pub async fn new() -> Result<Self, PwError> {
        let (graph_tx, _) = broadcast::channel(64);
        let graph = Arc::new(Mutex::new(AudioGraph::default()));

        let (pw_sender, pw_receiver) = mpsc::channel();

        // Start reducer — processes StateMsg commands and reconciles with PipeWire
        let (reducer, msg_tx) = run_reducer(pw_sender.clone(), ReconcileSettings::default())
            .await
            .map_err(|e| PwError::ConnectionFailed(format!("reducer init: {e}")))?;

        // Create graph update callback that feeds both the broadcast channel and the reducer
        let graph_clone = graph.clone();
        let tx_clone = graph_tx.clone();
        let debounced_send = debounced_graph_sender(msg_tx);

        let pw_sender_clone = pw_sender.clone();

        let pw_handle = PipewireHandle::init((pw_sender_clone, pw_receiver), move |new_graph| {
            let snapshot = *new_graph.clone();
            #[allow(clippy::unwrap_used)]
            {
                *graph_clone.lock().unwrap() = snapshot.clone();
            }
            let _ = tx_clone.send(snapshot);
            // Feed graph update to reducer for reconciliation
            debounced_send(new_graph);
        })?;

        debug!("OsgCore initialized, PipeWire connected, reducer started");

        Ok(Self {
            _pw_handle: pw_handle,
            graph,
            graph_tx,
            reducer,
        })
    }

    /// Get a snapshot of the current PipeWire graph state (read model).
    pub fn snapshot(&self) -> AudioGraph {
        #[allow(clippy::unwrap_used)]
        self.graph.lock().unwrap().clone()
    }

    /// Subscribe to graph change events (read model).
    pub fn subscribe(&self) -> broadcast::Receiver<AudioGraph> {
        self.graph_tx.subscribe()
    }

    /// Get the reducer handle for sending commands and subscribing to session state.
    pub fn reducer(&self) -> &ReducerHandle {
        &self.reducer
    }

    /// Send a command to mutate the MixerSession (write model).
    pub fn command(&self, msg: StateMsg) {
        self.reducer.emit(msg);
    }
}
