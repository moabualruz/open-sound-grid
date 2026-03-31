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

// ---------------------------------------------------------------------------
// Peak stream creation
// ---------------------------------------------------------------------------

use pipewire::{
    core::CoreRc,
    keys::*,
    properties::properties,
    spa::{
        param::ParamType,
        pod::{Pod, Value, serialize::PodSerializer},
    },
    stream::{StreamFlags, StreamRc},
};
use tracing::debug;

use super::PwError;

/// Build a serialized SPA pod for F32LE audio format negotiation.
fn build_f32le_format_pod() -> Vec<u8> {
    let mut info = pipewire::spa::param::audio::AudioInfoRaw::new();
    info.set_format(pipewire::spa::param::audio::AudioFormat::F32LE);
    let obj = pipewire::spa::pod::Object {
        type_: pipewire::spa::utils::SpaTypes::ObjectParamFormat.as_raw(),
        id: ParamType::EnumFormat.as_raw(),
        properties: info.into(),
    };
    #[allow(clippy::expect_used)]
    PodSerializer::serialize(std::io::Cursor::new(Vec::new()), &Value::Object(obj))
        .expect("F32LE format pod serialization")
        .0
        .into_inner()
}

/// Create a peak-monitoring capture stream for a node.
/// Returns `(stream, listener, peak_name)` on success.
/// The stream is NOT auto-connected — caller must manually link
/// the target's monitor ports via `pending_peak_links`.
pub fn create_peak_stream(
    pw_core: CoreRc,
    node_id: u32,
    peak_store: &PeakStore,
) -> Result<(StreamRc, pipewire::stream::StreamListener<()>, String), PwError> {
    let peak_name = format!("osg.peak.{node_id}");
    let props = properties! {
        *NODE_NAME => &*peak_name,
        *NODE_PASSIVE => "true",
        *NODE_VIRTUAL => "true",
        *MEDIA_TYPE => "Audio",
        *MEDIA_CATEGORY => "Capture",
        *MEDIA_ROLE => "DSP",
        "stream.monitor" => "true",
        "stream.capture.sink" => "true",
        "node.rate" => "1/25",
        "node.latency" => "1/25",
        "resample.peaks" => "true",
    };
    let stream = StreamRc::new(pw_core, "osg-peak-detect", props)
        .map_err(|e| PwError::ConnectionFailed(format!("peak stream: {e}")))?;
    let peak_data = peak_store.get_or_insert(node_id);
    let listener = stream
        .add_local_listener_with_user_data(())
        .process(move |stream, _| {
            let Some(mut buffer) = stream.dequeue_buffer() else {
                return;
            };
            let datas = buffer.datas_mut();
            let mut peaks = [0.0_f32; 2];
            for (i, peak) in peaks.iter_mut().enumerate() {
                if let Some(d) = datas.get_mut(i)
                    && let Some(bytes) = d.data()
                    && bytes.len() >= 4
                {
                    *peak = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
                        .abs()
                        .clamp(0.0, 1.0);
                }
            }
            if peaks[1] == 0.0 && peaks[0] > 0.0 {
                peaks[1] = peaks[0];
            }
            peak_data.store(peaks[0], peaks[1]);
        })
        .register()
        .map_err(|e| PwError::ConnectionFailed(format!("peak listener: {e}")))?;

    let format_bytes = build_f32le_format_pod();
    let Some(pod) = Pod::from_bytes(&format_bytes) else {
        return Err(PwError::ConnectionFailed("invalid peak pod bytes".into()));
    };
    let mut params = [pod];
    stream
        .connect(
            pipewire::spa::utils::Direction::Input,
            None,
            StreamFlags::MAP_BUFFERS | StreamFlags::RT_PROCESS,
            &mut params,
        )
        .map_err(|e| PwError::ConnectionFailed(format!("peak connect: {e}")))?;

    debug!("[PW] peak monitor started for node {node_id}");
    Ok((stream, listener, peak_name))
}
