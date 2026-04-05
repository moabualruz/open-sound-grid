// Tests for reconciliation correction loop safety.
//
// The reconciler (MixerSession::diff) must terminate and produce a stable
// result within a bounded number of iterations.  A naive loop that always
// reports "something changed" would reconcile forever; these tests verify
// that the diff step is idempotent after the first pass and that calling it
// repeatedly converges rather than diverges.

use osg_core::graph::{
    Channel, ChannelId, ChannelKind, Endpoint, EndpointDescriptor, MixerSession, ReconcileSettings,
    RuntimeState,
};
use osg_core::pw::AudioGraph;
use osg_core::routing::reconcile::ReconciliationService;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn default_settings() -> ReconcileSettings {
    ReconcileSettings::default()
}

fn make_session_with_channel() -> (MixerSession, ChannelId) {
    let mut session = MixerSession::default();
    let ch_id = ChannelId::new();
    session.channels.insert(
        ch_id,
        Channel {
            id: ch_id,
            kind: ChannelKind::Source,
            source_type: Default::default(),
            output_node_id: None,
            assigned_apps: Vec::new(),
            auto_app: false,
            allow_app_assignment: true,
        },
    );
    (session, ch_id)
}

// ---------------------------------------------------------------------------
// Test 1: empty session terminates on first call and produces no messages
// ---------------------------------------------------------------------------

#[test]
fn reconcile_empty_session_terminates_and_produces_no_events() {
    let mut session = MixerSession::default();
    let graph = AudioGraph::default();
    let mut rt = RuntimeState::default();
    let settings = default_settings();

    let events = ReconciliationService::reconcile(&mut session, &graph, &settings, &mut rt);

    // An empty session has nothing to reconcile.
    assert!(events.is_empty(), "expected no events for empty session");
}

// ---------------------------------------------------------------------------
// Test 2: diff is idempotent — calling it twice on the same state does not
// produce more work than the first call
// ---------------------------------------------------------------------------

#[test]
fn reconcile_is_stable_on_second_call() {
    let (mut session, _ch_id) = make_session_with_channel();
    let graph = AudioGraph::default();
    let mut rt = RuntimeState::default();
    let settings = default_settings();

    // First pass: may produce setup events (create group node, etc.)
    let first_events = ReconciliationService::reconcile(&mut session, &graph, &settings, &mut rt);

    // Second pass on unchanged state: must not grow unboundedly.
    // It may still produce the same events (PW hasn't confirmed creation yet),
    // but the count must not exceed the first pass — proving no amplification.
    let second_events = ReconciliationService::reconcile(&mut session, &graph, &settings, &mut rt);

    assert!(
        second_events.len() <= first_events.len(),
        "second reconcile pass produced MORE events ({}) than first ({}), indicating oscillation",
        second_events.len(),
        first_events.len(),
    );
}

// ---------------------------------------------------------------------------
// Test 3: bounded iteration — running diff in a loop converges within
// MAX_ITERATIONS passes.  This is the loop-detection property: the total
// number of events must not grow monotonically.
// ---------------------------------------------------------------------------

const MAX_ITERATIONS: usize = 10;

#[test]
fn reconcile_converges_within_bounded_iterations() {
    let (mut session, _ch_id) = make_session_with_channel();
    let graph = AudioGraph::default();
    let mut rt = RuntimeState::default();
    let settings = default_settings();

    let mut prev_count = usize::MAX;
    let mut converged = false;

    for i in 0..MAX_ITERATIONS {
        let events = ReconciliationService::reconcile(&mut session, &graph, &settings, &mut rt);
        let count = events.len();

        // Convergence: the event count must not keep growing.
        assert!(
            count <= prev_count,
            "reconcile iteration {} produced more events ({}) than previous iteration ({}) — loop detected",
            i,
            count,
            prev_count,
        );

        if count == 0 || count == prev_count {
            converged = true;
            break;
        }
        prev_count = count;
    }

    assert!(
        converged,
        "reconcile did not converge within {} iterations",
        MAX_ITERATIONS
    );
}

// ---------------------------------------------------------------------------
// Test 4: reconcile with a session that has an endpoint but no PW nodes
// produces a placeholder, not an infinite stream of create-node commands
// ---------------------------------------------------------------------------

#[test]
fn reconcile_with_unresolved_endpoint_does_not_grow_unboundedly() {
    let mut session = MixerSession::default();
    let graph = AudioGraph::default();
    let mut rt = RuntimeState::default();
    let settings = default_settings();

    // Add a source channel (will be unresolved since graph is empty).
    let ch_id = ChannelId::new();
    let ch_desc = EndpointDescriptor::Channel(ch_id);
    session.channels.insert(
        ch_id,
        Channel {
            id: ch_id,
            kind: ChannelKind::Sink,
            source_type: Default::default(),
            output_node_id: None,
            assigned_apps: Vec::new(),
            auto_app: false,
            allow_app_assignment: true,
        },
    );
    session.endpoints.insert(
        ch_desc,
        Endpoint::new(ch_desc).with_display_name("Test Channel".to_owned()),
    );

    let first = ReconciliationService::reconcile(&mut session, &graph, &settings, &mut rt).len();
    let second = ReconciliationService::reconcile(&mut session, &graph, &settings, &mut rt).len();
    let third = ReconciliationService::reconcile(&mut session, &graph, &settings, &mut rt).len();

    // Each successive call must not produce more events than the previous.
    assert!(
        second <= first,
        "call 2 ({second}) > call 1 ({first}) — oscillation"
    );
    assert!(
        third <= second,
        "call 3 ({third}) > call 2 ({second}) — oscillation"
    );
}

// ---------------------------------------------------------------------------
// Test 5: reconcile with active_sources populated does not loop
// ---------------------------------------------------------------------------

#[test]
fn reconcile_with_active_sources_terminates() {
    let mut session = MixerSession::default();
    session
        .active_sources
        .push(EndpointDescriptor::EphemeralNode(
            1,
            osg_core::graph::PortKind::Source,
        ));

    let graph = AudioGraph::default();
    let mut rt = RuntimeState::default();
    let settings = default_settings();

    // Must not panic or diverge.
    for _ in 0..5 {
        ReconciliationService::reconcile(&mut session, &graph, &settings, &mut rt);
    }
}
