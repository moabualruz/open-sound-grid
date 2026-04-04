//! OsgCore — the public entry point for PipeWire orchestration.
//!
//! Owns the PipeWire connection and the MixerSession reducer.
//! Exposes the AudioGraph read model, MixerSession write model,
//! and a command API for mutations.

use std::sync::{
    Arc, Mutex,
    mpsc::{self, Sender},
};

use tokio::sync::{broadcast, watch};
use tracing::debug;

use crate::graph::{MixerSession, ReconcileSettings};
use crate::pw::{
    AudioGraph, FilterHandleStore, PipewireHandle, PwError, ToPipewireMessage, peak::PeakStore,
};
use crate::routing::{ReducerHandle, StateMsg, debounced_graph_sender, run_reducer};
use crate::traits::{GraphObserver, RoutingService, VolumeService};

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
    peak_store: Arc<PeakStore>,
    filter_store: FilterHandleStore,
    _pw_sender: Sender<ToPipewireMessage>,
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

        let peak_store = Arc::new(PeakStore::new());
        let filter_store = FilterHandleStore::new();

        let pw_handle = PipewireHandle::init(
            (pw_sender_clone, pw_receiver),
            move |new_graph| {
                let snapshot = *new_graph.clone();
                #[allow(clippy::unwrap_used)]
                {
                    *graph_clone.lock().unwrap() = snapshot.clone();
                }
                let _ = tx_clone.send(snapshot);
                // Feed graph update to reducer for reconciliation
                debounced_send(new_graph);
            },
            peak_store.clone(),
            filter_store.clone(),
        )?;

        // Generate a unique instance ID for this process lifetime.
        // Stamped on all PW nodes for ownership tracking.
        let instance_id = ulid::Ulid::new();

        // ADR-007: Create staging sink — always-alive, vol=0, for glitch-free rerouting
        let _ = pw_sender.send(ToPipewireMessage::CreateStagingSink { instance_id });

        // Propagate instance_id to the reducer so reconciliation can stamp new nodes.
        reducer.set_instance_id(instance_id);

        debug!(
            %instance_id,
            "OsgCore initialized, PipeWire connected, reducer started"
        );

        Ok(Self {
            _pw_handle: pw_handle,
            graph,
            graph_tx,
            reducer,
            peak_store,
            filter_store,
            _pw_sender: pw_sender.clone(),
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

    /// Get the shared peak level store for WebSocket broadcast.
    pub fn peak_store(&self) -> &Arc<PeakStore> {
        &self.peak_store
    }

    /// Get the shared filter handle store for EQ control and peak reading.
    pub fn filter_store(&self) -> &FilterHandleStore {
        &self.filter_store
    }
}

// ---------------------------------------------------------------------------
// Service trait implementations
// ---------------------------------------------------------------------------

impl VolumeService for OsgCore {
    fn set_volume(&self, endpoint: crate::graph::EndpointDescriptor, volume: f32) {
        self.reducer.emit(StateMsg::SetVolume(endpoint, volume));
    }

    fn set_stereo_volume(
        &self,
        endpoint: crate::graph::EndpointDescriptor,
        left: f32,
        right: f32,
    ) {
        self.reducer
            .emit(StateMsg::SetStereoVolume(endpoint, left, right));
    }

    fn set_mute(&self, endpoint: crate::graph::EndpointDescriptor, muted: bool) {
        self.reducer.emit(StateMsg::SetMute(endpoint, muted));
    }
}

impl GraphObserver for OsgCore {
    fn snapshot(&self) -> AudioGraph {
        self.snapshot()
    }

    fn subscribe(&self) -> broadcast::Receiver<AudioGraph> {
        self.subscribe()
    }
}

impl RoutingService for OsgCore {
    fn command(&self, msg: StateMsg) {
        self.reducer.emit(msg);
    }

    fn state(&self) -> Arc<MixerSession> {
        self.reducer.state()
    }

    fn subscribe_state(&self) -> watch::Receiver<Arc<MixerSession>> {
        self.reducer.subscribe_state()
    }
}
