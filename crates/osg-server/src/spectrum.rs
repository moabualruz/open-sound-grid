use osg_core::pw::fft::SPECTRUM_BINS;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpectrumMessage {
    pub node_id: String,
    pub bins: Vec<f32>,
}

impl SpectrumMessage {
    pub fn new(node_id: String, bins: [f32; SPECTRUM_BINS]) -> Self {
        Self {
            node_id,
            bins: bins.to_vec(),
        }
    }
}
