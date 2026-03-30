//! Volume and mute command handlers.

use std::process::Command;

use crate::error::{OsgError, Result};
use crate::plugin::api::*;

use super::introspect_control;
use super::PulseAudioPlugin;

impl PulseAudioPlugin {
    pub(crate) fn handle_set_route_volume(
        &mut self,
        source: SourceId,
        mix: MixId,
        volume: f32,
    ) -> Result<PluginResponse> {
        let volume = volume.clamp(0.0, 1.0);
        tracing::debug!(source = ?source, mix = mix, volume = volume, "setting route volume");
        let route = self.routes.entry((source, mix)).or_default();
        route.volume = volume;
        route.volume_left = volume;
        route.volume_right = volume;

        // Apply volume via PA sink-input on the loopback
        if let Some(&sink_input_idx) = self.loopback_sink_inputs.get(&(source, mix)) {
            tracing::debug!(
                source = ?source, mix = mix, sink_input_idx, volume,
                "applying route volume to PA sink-input"
            );
            if let Err(e) = self.modules.set_sink_input_volume(
                self.connection.as_mut(),
                sink_input_idx,
                volume,
            ) {
                tracing::warn!(source = ?source, mix = mix, err = %e, "failed to set route volume via PA");
            }
        } else {
            tracing::warn!(
                source = ?source, mix = mix, volume,
                loopback_count = self.loopback_sink_inputs.len(),
                "SetRouteVolume: no sink-input found for route — volume change lost"
            );
        }

        Ok(PluginResponse::Ok)
    }

    pub(crate) fn handle_set_route_stereo_volume(
        &mut self,
        source: SourceId,
        mix: MixId,
        left: f32,
        right: f32,
    ) -> Result<PluginResponse> {
        let left = left.clamp(0.0, 1.0);
        let right = right.clamp(0.0, 1.0);
        tracing::debug!(source = ?source, mix = mix, left, right, "setting route stereo volume");
        let route = self.routes.entry((source, mix)).or_default();
        route.volume_left = left;
        route.volume_right = right;
        route.volume = (left + right) / 2.0;

        // Apply: PA sink-input (module-loopback) or wpctl (pw-link)
        if let Some(&sink_input_idx) = self.loopback_sink_inputs.get(&(source, mix)) {
            tracing::debug!(
                source = ?source, mix = mix, sink_input_idx, left, right,
                "applying stereo volume to PA sink-input"
            );
            if let Err(e) = self.modules.set_sink_input_stereo_volume(
                self.connection.as_mut(),
                sink_input_idx,
                left,
                right,
            ) {
                tracing::warn!(source = ?source, mix = mix, err = %e, "failed to set stereo route volume via PA");
            }
        } else {
            tracing::warn!(
                source = ?source, mix = mix, left, right,
                "SetRouteStereoVolume: no sink-input found for route — volume change lost"
            );
        }

        Ok(PluginResponse::Ok)
    }

    pub(crate) fn handle_set_route_muted(
        &mut self,
        source: SourceId,
        mix: MixId,
        muted: bool,
    ) -> Result<PluginResponse> {
        tracing::debug!(source = ?source, mix = mix, muted = muted, "setting route muted");
        self.routes.entry((source, mix)).or_default().muted = muted;

        if let Some(&sink_input_idx) = self.loopback_sink_inputs.get(&(source, mix)) {
            if let Err(e) = self.modules.set_sink_input_mute(
                self.connection.as_mut(),
                sink_input_idx,
                muted,
            ) {
                tracing::warn!(source = ?source, mix = mix, err = %e, "failed to set route mute via PA");
            }
        }

        Ok(PluginResponse::Ok)
    }

    pub(crate) fn handle_set_mix_master_volume(
        &mut self,
        mix: MixId,
        volume: f32,
    ) -> Result<PluginResponse> {
        tracing::debug!(mix_id = mix, volume = volume, "setting mix master volume");
        if let Some(m) = self.mixes.iter_mut().find(|m| m.id == mix) {
            m.master_volume = volume.clamp(0.0, 1.0);
            let clamped = m.master_volume;
            // Apply via PA: set volume on the mix null sink itself.
            if let Some(sink_name) = self.mix_sinks.get(&mix).cloned() {
                if let Some(conn) = self.connection.as_mut() {
                    if let Err(e) =
                        introspect_control::set_sink_volume_by_name_sync(conn, &sink_name, clamped)
                    {
                        tracing::warn!(mix_id = mix, err = %e, "set_sink_volume_by_name_sync failed");
                    } else {
                        tracing::debug!(mix_id = mix, volume = clamped, sink = %sink_name, "PA set-sink-volume applied via introspect");
                    }
                } else {
                    let percent = (clamped * 100.0) as u32;
                    let output = Command::new("pactl")
                        .args(["set-sink-volume", &sink_name, &format!("{percent}%")])
                        .output();
                    match output {
                        Ok(o) if o.status.success() => {
                            tracing::debug!(mix_id = mix, percent, sink = %sink_name, "PA set-sink-volume applied via pactl (fallback)");
                        }
                        Ok(o) => {
                            tracing::warn!(mix_id = mix, stderr = %String::from_utf8_lossy(&o.stderr), "PA set-sink-volume failed");
                        }
                        Err(e) => {
                            tracing::warn!(mix_id = mix, err = %e, "PA set-sink-volume command error");
                        }
                    }
                }
            }
            Ok(PluginResponse::Ok)
        } else {
            tracing::error!(mix_id = mix, "mix not found for SetMixMasterVolume");
            Err(OsgError::MixNotFound(mix))
        }
    }

    pub(crate) fn handle_set_mix_muted(
        &mut self,
        mix: MixId,
        muted: bool,
    ) -> Result<PluginResponse> {
        tracing::debug!(mix_id = mix, muted = muted, "setting mix muted");
        if let Some(m) = self.mixes.iter_mut().find(|m| m.id == mix) {
            m.muted = muted;
            // Apply via PA: mute the mix null sink.
            if let Some(sink_name) = self.mix_sinks.get(&mix).cloned() {
                if let Some(conn) = self.connection.as_mut() {
                    if let Err(e) =
                        introspect_control::set_sink_mute_by_name_sync(conn, &sink_name, muted)
                    {
                        tracing::warn!(mix_id = mix, err = %e, "set_sink_mute_by_name_sync failed");
                    } else {
                        tracing::debug!(mix_id = mix, muted, sink = %sink_name, "PA set-sink-mute applied to mix via introspect");
                    }
                } else {
                    let mute_val = if muted { "1" } else { "0" };
                    let output = Command::new("pactl")
                        .args(["set-sink-mute", &sink_name, mute_val])
                        .output();
                    match output {
                        Ok(o) if o.status.success() => {
                            tracing::debug!(mix_id = mix, muted, sink = %sink_name, "PA set-sink-mute applied to mix via pactl (fallback)");
                        }
                        Ok(o) => {
                            tracing::warn!(mix_id = mix, stderr = %String::from_utf8_lossy(&o.stderr), "PA set-sink-mute failed on mix");
                        }
                        Err(e) => {
                            tracing::warn!(mix_id = mix, err = %e, "PA set-sink-mute command error on mix");
                        }
                    }
                }
            }
            Ok(PluginResponse::Ok)
        } else {
            tracing::error!(mix_id = mix, "mix not found for SetMixMuted");
            Err(OsgError::MixNotFound(mix))
        }
    }

    pub(crate) fn handle_set_source_muted(
        &mut self,
        source: SourceId,
        muted: bool,
    ) -> Result<PluginResponse> {
        tracing::debug!(source = ?source, muted = muted, "setting source muted across all routes");
        // Update in-memory state AND apply to loopback sink-inputs
        let sink_input_keys: Vec<(SourceId, MixId)> = self
            .loopback_sink_inputs
            .keys()
            .filter(|(src, _)| *src == source)
            .cloned()
            .collect();
        for key in &sink_input_keys {
            if let Some(&idx) = self.loopback_sink_inputs.get(key) {
                if let Err(e) = self.modules.set_sink_input_mute(
                    self.connection.as_mut(),
                    idx,
                    muted,
                ) {
                    tracing::warn!(source = ?source, sink_input = idx, err = %e, "failed to set sink-input mute via PA");
                }
            }
        }
        for ((src, _), route) in &mut self.routes {
            if *src == source {
                route.muted = muted;
            }
        }
        // Apply via PA: mute the channel's null sink directly.
        if let SourceId::Channel(id) = source {
            if let Some(sink_name) = self.channel_sinks.get(&id).cloned() {
                if let Some(conn) = self.connection.as_mut() {
                    if let Err(e) =
                        introspect_control::set_sink_mute_by_name_sync(conn, &sink_name, muted)
                    {
                        tracing::warn!(source = ?source, err = %e, "set_sink_mute_by_name_sync failed for channel");
                    } else {
                        tracing::debug!(source = ?source, muted, sink = %sink_name, "PA set-sink-mute applied via introspect");
                    }
                } else {
                    let mute_val = if muted { "1" } else { "0" };
                    let output = Command::new("pactl")
                        .args(["set-sink-mute", &sink_name, mute_val])
                        .output();
                    match output {
                        Ok(o) if o.status.success() => {
                            tracing::debug!(source = ?source, muted, sink = %sink_name, "PA set-sink-mute applied via pactl (fallback)");
                        }
                        Ok(o) => {
                            tracing::warn!(source = ?source, stderr = %String::from_utf8_lossy(&o.stderr), "PA set-sink-mute failed");
                        }
                        Err(e) => {
                            tracing::warn!(source = ?source, err = %e, "PA set-sink-mute command error");
                        }
                    }
                }
            }
        }
        // Also update channel muted state
        if let SourceId::Channel(id) = source {
            if let Some(ch) = self.channels.iter_mut().find(|c| c.id == id) {
                ch.muted = muted;
            }
        }
        Ok(PluginResponse::Ok)
    }
}
