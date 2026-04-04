// Tests for CommandHandler trait and HandlerRegistry dispatch.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use osg_core::graph::events::MixerEvent;
use osg_core::graph::{
    AppId, ChannelId, ChannelKind, EffectsConfig, EndpointDescriptor, EqConfig, MixerSession,
    ReconcileSettings, RuntimeState,
};
use osg_core::pw::AudioGraph;
use osg_core::routing::handler::CommandHandler;
use osg_core::routing::handler_registry::HandlerRegistry;
use osg_core::routing::messages::{StateMsg, StateOutputMsg};

// ---------------------------------------------------------------------------
// Mock handler — proves Liskov substitutability
// ---------------------------------------------------------------------------

struct MockHandler {
    called: Arc<AtomicBool>,
}

impl CommandHandler for MockHandler {
    fn handles(&self, msg: &StateMsg) -> bool {
        matches!(msg, StateMsg::SetChannelOrder(..))
    }

    fn handle(
        &self,
        _session: &mut MixerSession,
        _msg: StateMsg,
        _graph: &AudioGraph,
        _rt: &mut RuntimeState,
        _settings: &ReconcileSettings,
    ) -> (Option<StateOutputMsg>, Vec<MixerEvent>) {
        self.called.store(true, Ordering::SeqCst);
        (None, vec![MixerEvent::RequestReconciliation])
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn mock_handler_is_interchangeable() {
    // Liskov: a mock implementing CommandHandler works identically to real ones.
    let called = Arc::new(AtomicBool::new(false));
    let mock = MockHandler {
        called: called.clone(),
    };

    let mut session = MixerSession::default();
    let graph = AudioGraph::default();
    let mut rt = RuntimeState::default();
    let settings = ReconcileSettings::default();
    let msg = StateMsg::SetChannelOrder(vec![]);

    assert!(mock.handles(&msg));
    let (output, events) = mock.handle(&mut session, msg, &graph, &mut rt, &settings);
    assert!(called.load(Ordering::SeqCst));
    assert!(output.is_none());
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], MixerEvent::RequestReconciliation));
}

#[test]
fn registry_dispatches_volume_commands() {
    let registry = HandlerRegistry::new();
    let mut session = MixerSession::default();
    let graph = AudioGraph::default();
    let mut rt = RuntimeState::default();
    let settings = ReconcileSettings::default();

    // SetVolume on a nonexistent endpoint is a no-op but should not panic.
    let ep = EndpointDescriptor::Channel(ChannelId::new());
    let msg = StateMsg::SetVolume(ep, 0.5);
    let (output, _events) = registry.dispatch(&mut session, msg, &graph, &mut rt, &settings);
    assert!(output.is_none());
}

#[test]
fn registry_dispatches_link_commands() {
    let registry = HandlerRegistry::new();
    let mut session = MixerSession::default();
    let graph = AudioGraph::default();
    let mut rt = RuntimeState::default();
    let settings = ReconcileSettings::default();

    let src = EndpointDescriptor::Channel(ChannelId::new());
    let sink = EndpointDescriptor::Channel(ChannelId::new());
    let msg = StateMsg::SetLinkLocked(src, sink, true);
    let (output, _events) = registry.dispatch(&mut session, msg, &graph, &mut rt, &settings);
    assert!(output.is_none());
}

#[test]
fn registry_dispatches_endpoint_commands() {
    let registry = HandlerRegistry::new();
    let mut session = MixerSession::default();
    let graph = AudioGraph::default();
    let mut rt = RuntimeState::default();
    let settings = ReconcileSettings::default();

    let msg = StateMsg::AddChannel("Test".to_string(), ChannelKind::Source);
    let (output, _events) = registry.dispatch(&mut session, msg, &graph, &mut rt, &settings);
    assert!(matches!(output, Some(StateOutputMsg::EndpointAdded(_))));
}

#[test]
fn registry_dispatches_eq_commands() {
    let registry = HandlerRegistry::new();
    let mut session = MixerSession::default();
    let graph = AudioGraph::default();
    let mut rt = RuntimeState::default();
    let settings = ReconcileSettings::default();

    let ep = EndpointDescriptor::Channel(ChannelId::new());
    let msg = StateMsg::SetEq(ep, EqConfig::default());
    let (output, _events) = registry.dispatch(&mut session, msg, &graph, &mut rt, &settings);
    assert!(output.is_none());
}

#[test]
fn registry_dispatches_output_commands() {
    let registry = HandlerRegistry::new();
    let mut session = MixerSession::default();
    let graph = AudioGraph::default();
    let mut rt = RuntimeState::default();
    let settings = ReconcileSettings::default();

    let ch_id = ChannelId::new();
    let msg = StateMsg::SetMixOutput(ch_id, None);
    let (output, _events) = registry.dispatch(&mut session, msg, &graph, &mut rt, &settings);
    assert!(output.is_none());
}

#[test]
fn registry_dispatches_order_commands() {
    let registry = HandlerRegistry::new();
    let mut session = MixerSession::default();
    let graph = AudioGraph::default();
    let mut rt = RuntimeState::default();
    let settings = ReconcileSettings::default();

    let msg = StateMsg::SetChannelOrder(vec![]);
    let (output, events) = registry.dispatch(&mut session, msg, &graph, &mut rt, &settings);
    assert!(output.is_none());
    assert!(events.is_empty());

    let msg = StateMsg::SetMixOrder(vec![]);
    let (output, events) = registry.dispatch(&mut session, msg, &graph, &mut rt, &settings);
    assert!(output.is_none());
    assert!(events.is_empty());

    let msg = StateMsg::SetDefaultOutputNode(Some(42));
    let (output, events) = registry.dispatch(&mut session, msg, &graph, &mut rt, &settings);
    assert!(output.is_none());
    assert!(events.is_empty());
    assert_eq!(rt.default_output_node_id, Some(42));
}

/// Exhaustiveness: every StateMsg variant must be handled by the registry.
/// This test creates a representative instance of each variant and asserts
/// that at least one handler claims it.
#[test]
fn registry_handles_all_state_msg_variants() {
    let registry = HandlerRegistry::new();
    let ch_id = ChannelId::new();
    let ep = EndpointDescriptor::Channel(ch_id);
    let ep2 = EndpointDescriptor::Channel(ChannelId::new());

    let all_msgs: Vec<StateMsg> = vec![
        // Endpoint
        StateMsg::AddEphemeralNode(1, osg_core::pw::PortKind::Source),
        StateMsg::AddChannel("x".into(), ChannelKind::Source),
        StateMsg::RemoveEndpoint(ep),
        StateMsg::RenameEndpoint(ep, None),
        StateMsg::ChangeChannelKind(ch_id, ChannelKind::Sink),
        StateMsg::SetEndpointVisible(ep, true),
        // Volume
        StateMsg::SetVolume(ep, 0.5),
        StateMsg::SetStereoVolume(ep, 0.5, 0.5),
        StateMsg::SetMute(ep, false),
        StateMsg::SetVolumeLocked(ep, false),
        StateMsg::SetLinkVolume(ep, ep2, 0.5),
        StateMsg::SetLinkStereoVolume(ep, ep2, 0.5, 0.5),
        // Link
        StateMsg::Link(ep, ep2),
        StateMsg::RemoveLink(ep, ep2),
        StateMsg::SetLinkLocked(ep, ep2, false),
        // App
        StateMsg::AddApp(AppId::new(), osg_core::pw::PortKind::Source),
        StateMsg::AssignApp(
            ch_id,
            osg_core::graph::AppAssignment {
                application_name: "app".into(),
                binary_name: "bin".into(),
            },
        ),
        StateMsg::UnassignApp(
            ch_id,
            osg_core::graph::AppAssignment {
                application_name: "app".into(),
                binary_name: "bin".into(),
            },
        ),
        // EQ / Effects
        StateMsg::SetEq(ep, EqConfig::default()),
        StateMsg::SetCellEq(ep, ep2, EqConfig::default()),
        StateMsg::SetEffects(ep, EffectsConfig::default()),
        StateMsg::SetCellEffects(ep, ep2, EffectsConfig::default()),
        // Output
        StateMsg::SetMixOutput(ch_id, None),
        // Order
        StateMsg::SetChannelOrder(vec![]),
        StateMsg::SetMixOrder(vec![]),
        StateMsg::SetDefaultOutputNode(None),
    ];

    for msg in &all_msgs {
        assert!(registry.handles(msg), "No handler registered for {:?}", msg);
    }
}
