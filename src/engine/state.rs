//! Mixer state owned by the engine.
//!
//! This is the UI-facing state. It mirrors what the plugin reports
//! but is owned by the main thread for safe access from iced.

use std::collections::HashMap;

use crate::plugin::api::{
    AudioApplication, ChannelInfo, HardwareInput, HardwareOutput, MixInfo, MixerSnapshot, RouteState,
    SourceId,
};

/// UI-facing mixer state.
#[derive(Debug, Clone, Default)]
pub struct MixerState {
    pub channels: Vec<ChannelInfo>,
    pub mixes: Vec<MixInfo>,
    pub routes: HashMap<(SourceId, u32), RouteState>,
    pub hardware_inputs: Vec<HardwareInput>,
    pub hardware_outputs: Vec<HardwareOutput>,
    pub applications: Vec<AudioApplication>,
    pub peak_levels: HashMap<SourceId, f32>,
    pub connected: bool,
}

impl MixerState {
    /// Apply a plugin snapshot, replacing all state.
    pub fn apply_snapshot(&mut self, snap: MixerSnapshot) {
        tracing::info!(
            channels = snap.channels.len(),
            mixes = snap.mixes.len(),
            routes = snap.routes.len(),
            hardware_inputs = snap.hardware_inputs.len(),
            hardware_outputs = snap.hardware_outputs.len(),
            applications = snap.applications.len(),
            "applied snapshot"
        );
        self.channels = snap.channels;
        self.mixes = snap.mixes;
        self.hardware_inputs = snap.hardware_inputs;
        self.hardware_outputs = snap.hardware_outputs;
        self.applications = snap.applications;
        self.peak_levels = snap.peak_levels;
        self.connected = true;

        // Convert route keys from (SourceId, MixId) to (SourceId, u32)
        self.routes.clear();
        for ((source, mix), route) in snap.routes {
            self.routes.insert((source, mix), route);
        }
    }

    /// Update peak levels without replacing everything else.
    pub fn update_peaks(&mut self, levels: HashMap<SourceId, f32>) {
        tracing::trace!(count = levels.len(), "updating peak levels");
        self.peak_levels = levels;
    }

    /// Update application list.
    pub fn update_applications(&mut self, apps: Vec<AudioApplication>) {
        tracing::debug!(count = apps.len(), "updating application list");
        self.applications = apps;
    }
}
