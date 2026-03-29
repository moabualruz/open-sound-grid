//! Snapshot building and sink-name helpers.

use crate::plugin::api::*;

use super::devices::DeviceEnumerator;
use super::PulseAudioPlugin;

impl PulseAudioPlugin {
    pub(crate) fn build_snapshot(&mut self) -> MixerSnapshot {
        tracing::debug!(
            channels = self.channels.len(),
            mixes = self.mixes.len(),
            routes = self.routes.len(),
            "building mixer snapshot"
        );
        let hardware_inputs = {
            let v = DeviceEnumerator::list_inputs(self.connection.as_mut());
            if v.is_empty() {
                tracing::warn!(
                    "build_snapshot: list_inputs returned empty (PA may be disconnected)"
                );
            }
            v
        };
        let hardware_outputs = {
            let v = DeviceEnumerator::list_outputs(self.connection.as_mut());
            if v.is_empty() {
                tracing::warn!(
                    "build_snapshot: list_outputs returned empty (PA may be disconnected)"
                );
            }
            v
        };
        let mut applications = match self.apps.list_applications(self.connection.as_mut()) {
            Ok(apps) => apps,
            Err(e) => {
                tracing::warn!(err = %e, "build_snapshot: list_applications failed — returning empty list");
                Vec::new()
            }
        };

        // Populate AudioApplication.channel from channel.apps
        for app in &mut applications {
            for channel in &self.channels {
                if channel.apps.contains(&app.stream_index) {
                    app.channel = Some(channel.id);
                    tracing::trace!(app_name = %app.name, channel_id = channel.id, "app routed to channel in snapshot");
                    break;
                }
            }
        }

        // Poll current peak levels from PA before reading the shared atomics.
        // read_peaks() queries `pactl get-sink-volume` for each registered sink
        // and stores the result in SharedPeak atomics. get_levels() then reads
        // them lock-free. This ensures VU meters show real data instead of zeros.
        tracing::trace!("polling VU peak levels before snapshot");
        self.peaks.read_peaks();
        let peak_levels = self.peaks.get_levels();
        let nonzero_peaks = peak_levels.values().filter(|&&v| v > 0.0).count();
        if nonzero_peaks > 0 {
            tracing::debug!(
                nonzero_peaks, total = peak_levels.len(),
                "VU meters: non-zero peak levels captured"
            );
        }
        MixerSnapshot {
            channels: self.channels.clone(),
            mixes: self.mixes.clone(),
            routes: self.routes.clone(),
            hardware_inputs,
            hardware_outputs,
            applications,
            peak_levels,
        }
    }

    /// Get the PA sink name for a channel.
    pub(crate) fn channel_sink_name(name: &str) -> String {
        format!("osg_{}_ch", name.replace(' ', "_"))
    }

    /// Get the PA sink name for a mix.
    pub(crate) fn mix_sink_name(name: &str) -> String {
        format!("osg_{}_mix", name.replace(' ', "_"))
    }
}
