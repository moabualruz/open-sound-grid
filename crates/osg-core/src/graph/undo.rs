//! Snapshot-based undo/redo for destructive `MixerSession` operations.
//!
//! `UndoStack` holds a ring buffer of up to 50 `MixerSession` clones.
//! Callers are responsible for:
//! 1. Calling `push()` with the **current** state BEFORE applying a destructive command.
//! 2. Calling `undo(current)` / `redo(current)` and replacing the live state with the result.

use std::collections::VecDeque;

use crate::graph::MixerSession;

/// Maximum number of undo snapshots retained.
const MAX_UNDO_DEPTH: usize = 50;

/// Ring-buffer undo/redo stack for `MixerSession` snapshots.
#[derive(Debug, Default)]
pub struct UndoStack {
    undo_stack: VecDeque<MixerSession>,
    redo_stack: VecDeque<MixerSession>,
}

impl UndoStack {
    /// Create an empty `UndoStack`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a snapshot of the current state before a destructive command.
    ///
    /// Clears the redo stack (standard undo behavior: new action invalidates
    /// any undone future).
    pub fn push(&mut self, snapshot: MixerSession) {
        self.redo_stack.clear();
        self.undo_stack.push_back(snapshot);
        // Drop oldest entry when over capacity.
        if self.undo_stack.len() > MAX_UNDO_DEPTH {
            self.undo_stack.pop_front();
        }
    }

    /// Undo: pop the most-recent snapshot, push `current` onto the redo stack,
    /// and return the snapshot to restore. Returns `None` when the stack is empty.
    pub fn undo(&mut self, current: MixerSession) -> Option<MixerSession> {
        let snapshot = self.undo_stack.pop_back()?;
        self.redo_stack.push_back(current);
        // F3-P0-1: Cap redo stack the same as undo stack.
        if self.redo_stack.len() > MAX_UNDO_DEPTH {
            self.redo_stack.pop_front();
        }
        Some(snapshot)
    }

    /// Redo: pop the most-recent redo snapshot, push `current` onto the undo
    /// stack, and return the snapshot to restore. Returns `None` when the redo
    /// stack is empty.
    pub fn redo(&mut self, current: MixerSession) -> Option<MixerSession> {
        let snapshot = self.redo_stack.pop_back()?;
        self.undo_stack.push_back(current);
        // Cap undo stack (same as push).
        if self.undo_stack.len() > MAX_UNDO_DEPTH {
            self.undo_stack.pop_front();
        }
        Some(snapshot)
    }

    /// Returns `true` when there is at least one snapshot to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Returns `true` when there is at least one snapshot to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
}
