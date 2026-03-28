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
        let prev_channel_count = self.channels.len();
        let prev_mix_count = self.mixes.len();
        let new_channel_count = snap.channels.len();
        let new_mix_count = snap.mixes.len();

        tracing::info!(
            channels = new_channel_count,
            channels_delta = new_channel_count as i64 - prev_channel_count as i64,
            mixes = new_mix_count,
            mixes_delta = new_mix_count as i64 - prev_mix_count as i64,
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
            tracing::trace!(source = ?source, mix, enabled = route.enabled, volume = route.volume, "route state applied");
            self.routes.insert((source, mix), route);
        }
    }

    /// Update peak levels without replacing everything else.
    pub fn update_peaks(&mut self, levels: HashMap<SourceId, f32>) {
        let nonzero = levels.values().filter(|&&v| v > 0.0).count();
        tracing::trace!(count = levels.len(), nonzero, "updating peak levels");
        if nonzero > 0 {
            tracing::debug!(nonzero, "non-zero peak levels present");
        }
        self.peak_levels = levels;
    }

    /// Update application list.
    pub fn update_applications(&mut self, apps: Vec<AudioApplication>) {
        let names: Vec<&str> = apps.iter().map(|a| a.name.as_str()).collect();
        tracing::debug!(count = apps.len(), ?names, "updating application list");
        self.applications = apps;
    }
}
