// Tests for the snapshot-based undo/redo system.

use osg_core::commands::Command;
use osg_core::graph::{MixerSession, undo::UndoStack};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn push_channels(session: &mut MixerSession, count: usize) {
    use osg_core::graph::{Channel, ChannelId, ChannelKind};
    for _ in 0..count {
        let id = ChannelId::new();
        session.channels.insert(
            id,
            Channel {
                id,
                kind: ChannelKind::Source,
                source_type: Default::default(),
                output_node_id: None,
                assigned_apps: Vec::new(),
                auto_app: false,
                allow_app_assignment: true,
            },
        );
    }
}

// ---------------------------------------------------------------------------
// Test 1: push snapshot, undo restores previous state
// ---------------------------------------------------------------------------

#[test]
fn undo_restores_previous_state() {
    let mut stack = UndoStack::new();

    // State 0 (empty)
    let state0 = MixerSession::default();

    // Push snapshot of state0 before mutating to state1
    stack.push(state0.clone());

    // "Mutate" to state1 (add a channel)
    let mut state1 = state0.clone();
    push_channels(&mut state1, 1);

    // Undo: should return state0
    let restored = stack.undo(state1.clone());
    assert!(
        restored.is_some(),
        "undo should return Some on non-empty stack"
    );
    let restored = restored.unwrap();
    assert_eq!(
        restored.channels.len(),
        0,
        "undo should restore state with 0 channels"
    );
}

// ---------------------------------------------------------------------------
// Test 2: redo after undo restores the forward state
// ---------------------------------------------------------------------------

#[test]
fn redo_after_undo_restores_forward_state() {
    let mut stack = UndoStack::new();

    let state0 = MixerSession::default();
    stack.push(state0.clone());

    let mut state1 = state0.clone();
    push_channels(&mut state1, 1);

    // Undo: push state1 onto redo stack, return state0
    let restored = stack.undo(state1.clone()).unwrap();
    assert_eq!(restored.channels.len(), 0);

    // Redo: push restored onto undo stack, return state1
    let redone = stack.redo(restored.clone());
    assert!(redone.is_some(), "redo should return Some after undo");
    let redone = redone.unwrap();
    assert_eq!(
        redone.channels.len(),
        1,
        "redo should restore state with 1 channel"
    );
}

// ---------------------------------------------------------------------------
// Test 3: new command clears redo stack
// ---------------------------------------------------------------------------

#[test]
fn new_command_clears_redo_stack() {
    let mut stack = UndoStack::new();

    let state0 = MixerSession::default();
    stack.push(state0.clone());

    let mut state1 = state0.clone();
    push_channels(&mut state1, 1);

    // Undo to state0
    let _restored = stack.undo(state1.clone()).unwrap();

    // Now push a new snapshot (simulating a new destructive command)
    stack.push(MixerSession::default());

    // Redo stack should be cleared — redo returns None
    let result = stack.redo(MixerSession::default());
    assert!(
        result.is_none(),
        "redo stack should be cleared after new command"
    );
}

// ---------------------------------------------------------------------------
// Test 4: stack caps at 50 (oldest dropped)
// ---------------------------------------------------------------------------

#[test]
fn stack_caps_at_50_oldest_dropped() {
    let mut stack = UndoStack::new();

    // Push 55 snapshots
    for _ in 0..55 {
        stack.push(MixerSession::default());
    }

    // After capping at 50, undo should work exactly 50 times then return None
    let mut current = MixerSession::default();
    let mut undo_count = 0;
    loop {
        match stack.undo(current.clone()) {
            Some(prev) => {
                current = prev;
                undo_count += 1;
            }
            None => break,
        }
    }
    assert_eq!(undo_count, 50, "stack should hold exactly 50 snapshots");
}

// ---------------------------------------------------------------------------
// Test 5: undo on empty stack is no-op (returns None)
// ---------------------------------------------------------------------------

#[test]
fn undo_on_empty_stack_returns_none() {
    let mut stack = UndoStack::new();
    let result = stack.undo(MixerSession::default());
    assert!(result.is_none(), "undo on empty stack should return None");
}

// ---------------------------------------------------------------------------
// Test 6: Undo/Redo Command variants serialize correctly
// ---------------------------------------------------------------------------

#[test]
fn command_undo_serializes_type_field() {
    let cmd = Command::Undo;
    let json = serde_json::to_string(&cmd).expect("serialize undo");
    let v: serde_json::Value = serde_json::from_str(&json).expect("parse undo");
    assert_eq!(v["type"].as_str().unwrap(), "undo");
}

#[test]
fn command_redo_serializes_type_field() {
    let cmd = Command::Redo;
    let json = serde_json::to_string(&cmd).expect("serialize redo");
    let v: serde_json::Value = serde_json::from_str(&json).expect("parse redo");
    assert_eq!(v["type"].as_str().unwrap(), "redo");
}

#[test]
fn command_undo_round_trips() {
    let cmd = Command::Undo;
    let json = serde_json::to_string(&cmd).unwrap();
    let rt: Command = serde_json::from_str(&json).unwrap();
    let json2 = serde_json::to_string(&rt).unwrap();
    assert_eq!(json, json2);
}

#[test]
fn command_redo_round_trips() {
    let cmd = Command::Redo;
    let json = serde_json::to_string(&cmd).unwrap();
    let rt: Command = serde_json::from_str(&json).unwrap();
    let json2 = serde_json::to_string(&rt).unwrap();
    assert_eq!(json, json2);
}

// ---------------------------------------------------------------------------
// Test 9: can_undo / can_redo report correct state
// ---------------------------------------------------------------------------

#[test]
fn can_undo_can_redo_reflect_stack_state() {
    let mut stack = UndoStack::new();

    assert!(!stack.can_undo(), "empty stack: can_undo should be false");
    assert!(!stack.can_redo(), "empty stack: can_redo should be false");

    stack.push(MixerSession::default());
    assert!(stack.can_undo(), "after push: can_undo should be true");
    assert!(!stack.can_redo(), "after push: can_redo should be false");

    let current = MixerSession::default();
    let _restored = stack.undo(current).unwrap();
    assert!(
        !stack.can_undo(),
        "after undo all: can_undo should be false"
    );
    assert!(stack.can_redo(), "after undo: can_redo should be true");

    let _redone = stack.redo(MixerSession::default()).unwrap();
    assert!(stack.can_undo(), "after redo: can_undo should be true");
    assert!(
        !stack.can_redo(),
        "after redo all: can_redo should be false"
    );
}

// ---------------------------------------------------------------------------
// Test 10: acceptance — create 3 channels, undo 3x, redo 3x
// ---------------------------------------------------------------------------

#[test]
fn create_three_channels_undo_three_redo_three() {
    let mut stack = UndoStack::new();
    let mut state = MixerSession::default();

    // Create 3 channels, snapshotting before each
    for i in 0..3 {
        stack.push(state.clone());
        push_channels(&mut state, 1);
        assert_eq!(state.channels.len(), i + 1);
    }

    // Undo 3 times — all channels removed
    for i in (0..3).rev() {
        let restored = stack.undo(state.clone()).expect("undo should succeed");
        assert_eq!(
            restored.channels.len(),
            i,
            "after undo, expected {i} channels"
        );
        state = restored;
    }
    assert_eq!(state.channels.len(), 0, "after 3 undos, no channels");
    assert!(stack.undo(state.clone()).is_none(), "no more undos");

    // Redo 3 times — all channels restored one by one
    for i in 1..=3 {
        let redone = stack.redo(state.clone()).expect("redo should succeed");
        assert_eq!(
            redone.channels.len(),
            i,
            "after redo, expected {i} channels"
        );
        state = redone;
    }
    assert_eq!(state.channels.len(), 3, "after 3 redos, 3 channels");
    assert!(stack.redo(state.clone()).is_none(), "no more redos");
}
