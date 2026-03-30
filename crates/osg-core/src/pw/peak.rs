//! Real-time peak level monitoring via PipeWire monitor streams.
//!
//! Creates one capture stream per monitored node that reads audio data
//! from the node's monitor port and computes per-channel peak levels.
//! Peaks are stored via atomics and read by the WebSocket broadcast layer.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};

use serde::Serialize;

// ---------------------------------------------------------------------------
// Atomic f32 helpers (no external crate needed)
// ---------------------------------------------------------------------------

fn atomic_store_f32(atom: &AtomicU32, val: f32) {
    atom.store(val.to_bits(), Ordering::Relaxed);
}

fn atomic_load_f32(atom: &AtomicU32) -> f32 {
    f32::from_bits(atom.load(Ordering::Relaxed))
}

// ---------------------------------------------------------------------------
// Peak data per node
// ---------------------------------------------------------------------------

/// Thread-safe peak level data for a single node (L/R stereo).
#[derive(Debug)]
pub struct PeakData {
    left: AtomicU32,
    right: AtomicU32,
}

impl Default for PeakData {
    fn default() -> Self {
        Self {
            left: AtomicU32::new(0_f32.to_bits()),
            right: AtomicU32::new(0_f32.to_bits()),
        }
    }
}

impl PeakData {
    pub fn store(&self, left: f32, right: f32) {
        atomic_store_f32(&self.left, left);
        atomic_store_f32(&self.right, right);
    }

    pub fn load(&self) -> (f32, f32) {
        (atomic_load_f32(&self.left), atomic_load_f32(&self.right))
    }
}

// ---------------------------------------------------------------------------
// Peak store — shared between PW thread and WebSocket broadcast
// ---------------------------------------------------------------------------

/// Shared store of peak levels for all monitored nodes.
/// PW thread writes, WebSocket endpoint reads.
#[derive(Debug, Default)]
pub struct PeakStore {
    #[allow(clippy::type_complexity)]
    nodes: RwLock<HashMap<u32, Arc<PeakData>>>,
}

impl PeakStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get or create peak data for a node.
    pub fn get_or_insert(&self, node_id: u32) -> Arc<PeakData> {
        #[allow(clippy::unwrap_used)]
        {
            let read = self.nodes.read().unwrap();
            if let Some(data) = read.get(&node_id) {
                return data.clone();
            }
        }
        let data = Arc::new(PeakData::default());
        #[allow(clippy::unwrap_used)]
        self.nodes.write().unwrap().insert(node_id, data.clone());
        data
    }

    /// Remove peak data for a node.
    pub fn remove(&self, node_id: u32) {
        #[allow(clippy::unwrap_used)]
        self.nodes.write().unwrap().remove(&node_id);
    }

    /// Snapshot all current peak levels for WebSocket broadcast.
    pub fn snapshot(&self) -> Vec<NodePeakLevel> {
        #[allow(clippy::unwrap_used)]
        self.nodes
            .read()
            .unwrap()
            .iter()
            .map(|(&node_id, data)| {
                let (left, right) = data.load();
                NodePeakLevel {
                    node_id,
                    left,
                    right,
                }
            })
            .collect()
    }
}

/// JSON-serializable peak level for one node.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodePeakLevel {
    pub node_id: u32,
    pub left: f32,
    pub right: f32,
}
