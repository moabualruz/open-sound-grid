// Tests for Link constructors added in Task 1B.

use osg_core::graph::{EffectsConfig, EndpointDescriptor, EqConfig, Link, LinkState, PortKind};

fn source() -> EndpointDescriptor {
    EndpointDescriptor::EphemeralNode(1, PortKind::Source)
}

fn sink() -> EndpointDescriptor {
    EndpointDescriptor::EphemeralNode(2, PortKind::Sink)
}

#[test]
fn connected_unlocked_has_correct_state() {
    let link = Link::connected_unlocked(source(), sink());
    assert_eq!(link.state, LinkState::ConnectedUnlocked);
}

#[test]
fn connected_unlocked_has_default_volumes() {
    let link = Link::connected_unlocked(source(), sink());
    assert_eq!(link.cell_volume, 1.0);
    assert_eq!(link.cell_volume_left, 1.0);
    assert_eq!(link.cell_volume_right, 1.0);
}

#[test]
fn connected_unlocked_has_no_cell_node_id() {
    let link = Link::connected_unlocked(source(), sink());
    assert_eq!(link.cell_node_id, None);
}

#[test]
fn connected_unlocked_has_default_eq_and_effects() {
    let link = Link::connected_unlocked(source(), sink());
    assert_eq!(link.cell_eq, EqConfig::default());
    assert_eq!(link.cell_effects, EffectsConfig::default());
}

#[test]
fn connected_unlocked_preserves_endpoints() {
    let s = source();
    let k = sink();
    let link = Link::connected_unlocked(s, k);
    assert_eq!(link.start, s);
    assert_eq!(link.end, k);
}

#[test]
fn disconnected_locked_has_correct_state() {
    let link = Link::disconnected_locked(source(), sink());
    assert_eq!(link.state, LinkState::DisconnectedLocked);
}

#[test]
fn disconnected_locked_has_default_volumes() {
    let link = Link::disconnected_locked(source(), sink());
    assert_eq!(link.cell_volume, 1.0);
    assert_eq!(link.cell_volume_left, 1.0);
    assert_eq!(link.cell_volume_right, 1.0);
}

#[test]
fn disconnected_locked_has_default_eq_and_effects() {
    let link = Link::disconnected_locked(source(), sink());
    assert_eq!(link.cell_eq, EqConfig::default());
    assert_eq!(link.cell_effects, EffectsConfig::default());
}

#[test]
fn disconnected_locked_preserves_endpoints() {
    let s = source();
    let k = sink();
    let link = Link::disconnected_locked(s, k);
    assert_eq!(link.start, s);
    assert_eq!(link.end, k);
}
