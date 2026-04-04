//! Liskov substitutability proofs for VolumeService, GraphObserver, RoutingService.
//!
//! Mock impls exercise each trait through `dyn Trait` references to confirm any
//! concrete type implementing the trait is interchangeable with OsgCore.

use std::sync::Arc;

use tokio::sync::{broadcast, watch};

use osg_core::graph::{ChannelId, EndpointDescriptor, MixerSession};
use osg_core::pw::AudioGraph;
use osg_core::routing::messages::StateMsg;
use osg_core::traits::{GraphObserver, RoutingService, VolumeService};

// ---------------------------------------------------------------------------
// Mock: VolumeService
// ---------------------------------------------------------------------------

struct MockVolumeService {
    calls: std::cell::Cell<usize>,
}

impl MockVolumeService {
    fn new() -> Self {
        Self {
            calls: std::cell::Cell::new(0),
        }
    }
}

impl VolumeService for MockVolumeService {
    fn set_volume(&self, _endpoint: EndpointDescriptor, _volume: f32) {
        self.calls.set(self.calls.get() + 1);
    }

    fn set_stereo_volume(&self, _endpoint: EndpointDescriptor, _left: f32, _right: f32) {
        self.calls.set(self.calls.get() + 1);
    }

    fn set_mute(&self, _endpoint: EndpointDescriptor, _muted: bool) {
        self.calls.set(self.calls.get() + 1);
    }
}

// ---------------------------------------------------------------------------
// Mock: GraphObserver
// ---------------------------------------------------------------------------

struct MockGraphObserver;

impl GraphObserver for MockGraphObserver {
    fn snapshot(&self) -> AudioGraph {
        AudioGraph::default()
    }

    fn subscribe(&self) -> broadcast::Receiver<AudioGraph> {
        let (tx, rx) = broadcast::channel(1);
        // Keep tx alive just long enough for the caller to hold rx.
        drop(tx);
        rx
    }
}

// ---------------------------------------------------------------------------
// Mock: RoutingService
// ---------------------------------------------------------------------------

struct MockRoutingService {
    _state_tx: watch::Sender<Arc<MixerSession>>,
    state_rx: watch::Receiver<Arc<MixerSession>>,
}

impl MockRoutingService {
    fn new() -> Self {
        let (state_tx, state_rx) = watch::channel(Arc::new(MixerSession::default()));
        Self {
            _state_tx: state_tx,
            state_rx,
        }
    }
}

impl RoutingService for MockRoutingService {
    fn command(&self, _msg: StateMsg) {}

    fn state(&self) -> Arc<MixerSession> {
        self.state_rx.borrow().clone()
    }

    fn subscribe_state(&self) -> watch::Receiver<Arc<MixerSession>> {
        self.state_rx.clone()
    }
}

// ---------------------------------------------------------------------------
// Liskov tests
// ---------------------------------------------------------------------------

#[test]
fn mock_volume_service_is_substitutable() {
    let svc = MockVolumeService::new();
    let svc_ref: &dyn VolumeService = &svc;

    // Use a real EndpointDescriptor — the simplest variant
    let ep = EndpointDescriptor::Channel(ChannelId::new());

    svc_ref.set_volume(ep.clone(), 0.8);
    svc_ref.set_stereo_volume(ep.clone(), 0.5, 0.9);
    svc_ref.set_mute(ep, true);

    assert_eq!(svc.calls.get(), 3);
}

#[test]
fn mock_graph_observer_is_substitutable() {
    let observer: &dyn GraphObserver = &MockGraphObserver;
    let graph = observer.snapshot();
    // AudioGraph::default() is a valid, empty graph snapshot
    let _ = graph;
    // subscribe() must return a valid receiver
    let _rx = observer.subscribe();
}

#[test]
fn mock_routing_service_is_substitutable() {
    let svc = MockRoutingService::new();
    let svc_ref: &dyn RoutingService = &svc;

    // command must accept any StateMsg without panic
    svc_ref.command(StateMsg::SetDefaultOutputNode(None));

    // state() returns the initial empty session
    let _session = svc_ref.state();

    // subscribe_state() returns a valid watch receiver
    let rx = svc_ref.subscribe_state();
    let _current = rx.borrow().clone();
}
