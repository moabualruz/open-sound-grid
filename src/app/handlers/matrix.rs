//! Matrix interaction handlers: route volume, route toggle, mute toggles, channel master volume.

use crate::plugin::api::{PluginCommand, SourceId};

use super::super::state::App;

impl App {
    pub fn handle_route_volume_changed(
        &mut self,
        source: SourceId,
        mix: crate::plugin::api::MixId,
        volume: f32,
    ) {
        let ch_master = match source {
            SourceId::Channel(ch_id) => self
                .channel_master_volumes
                .get(&ch_id)
                .copied()
                .unwrap_or(1.0),
            _ => 1.0,
        };
        let effective = (volume * ch_master).clamp(0.0, 1.0);
        tracing::debug!(
            ?source, ?mix, cell_ratio = volume, ch_master, effective,
            "route volume changed (WL3 model)"
        );
        self.engine.send_command(PluginCommand::SetRouteVolume {
            source,
            mix,
            volume: effective,
        });
        // Also sync the L/R values to match mono (prevents stale stereo values
        // from interfering when switching between mono and stereo modes)
        if let Some(route) = self.engine.state.routes.get_mut(&(source, mix)) {
            route.volume = effective;
            route.volume_left = effective;
            route.volume_right = effective;
        }
        let new_ratio = volume;
        self.engine
            .state
            .route_ratios
            .insert((source, mix), new_ratio);
        tracing::debug!(?source, ?mix, new_ratio, effective, "linked slider: mono ratio updated");
    }

    pub fn handle_route_toggled(
        &mut self,
        source: SourceId,
        mix: crate::plugin::api::MixId,
    ) {
        tracing::debug!(?source, ?mix, "route toggled");
        let currently_enabled = self
            .engine
            .state
            .routes
            .get(&(source, mix))
            .map_or(true, |r| r.enabled);
        if !currently_enabled {
            self.engine.state.route_ratios.insert((source, mix), 1.0);
            tracing::debug!(?source, ?mix, "linked slider: new route ratio initialised to 1.0");
        } else {
            self.engine.state.route_ratios.remove(&(source, mix));
            tracing::debug!(?source, ?mix, "linked slider: ratio removed for disabled route");
        }
        self.engine.send_command(PluginCommand::SetRouteEnabled {
            source,
            mix,
            enabled: !currently_enabled,
        });
    }

    pub fn handle_route_stereo_volume_changed(
        &mut self,
        source: SourceId,
        mix: crate::plugin::api::MixId,
        left: f32,
        right: f32,
    ) {
        let ch_master = match source {
            SourceId::Channel(ch_id) => self
                .channel_master_volumes
                .get(&ch_id)
                .copied()
                .unwrap_or(1.0),
            _ => 1.0,
        };
        let effective_left = (left * ch_master).clamp(0.0, 1.0);
        let effective_right = (right * ch_master).clamp(0.0, 1.0);
        tracing::debug!(
            ?source, ?mix, left, right, ch_master, effective_left, effective_right,
            "route stereo volume changed (WL3 model)"
        );
        self.engine.send_command(PluginCommand::SetRouteStereoVolume {
            source,
            mix,
            left: effective_left,
            right: effective_right,
        });
        // Store EFFECTIVE values in route state (must match what PA reports
        // in the next snapshot, or the slider position will ping-pong).
        if let Some(route) = self.engine.state.routes.get_mut(&(source, mix)) {
            route.volume_left = effective_left;
            route.volume_right = effective_right;
            route.volume = (effective_left + effective_right) / 2.0;
        }
        // Mono ratio = average of L/R ratios (for linked slider compat)
        let avg_ratio = (left + right) / 2.0;
        self.engine
            .state
            .route_ratios
            .insert((source, mix), avg_ratio);
        tracing::debug!(?source, ?mix, avg_ratio, "linked slider: stereo ratio updated");
    }

    pub fn handle_channel_master_volume_changed(
        &mut self,
        source: SourceId,
        volume: f32,
    ) {
        tracing::debug!(?source, master = volume, "channel master volume changed (WL3 model)");
        if let SourceId::Channel(ch_id) = source {
            self.channel_master_volumes.insert(ch_id, volume);
            if let Some(ch) = self
                .engine
                .state
                .channels
                .iter()
                .find(|c| c.id == ch_id)
            {
                if let Some(cfg) = self
                    .config
                    .channels
                    .iter_mut()
                    .find(|c| c.name == ch.name)
                {
                    cfg.master_volume = volume;
                }
            }
        }
        let mix_ids: Vec<_> = self.engine.state.mixes.iter().map(|m| m.id).collect();
        for mix_id in mix_ids {
            let key = (source, mix_id);
            if self.engine.state.routes.contains_key(&key) {
                let ratio = self
                    .engine
                    .state
                    .route_ratios
                    .get(&key)
                    .copied()
                    .unwrap_or(1.0);
                let effective = (volume * ratio).clamp(0.0, 1.0);
                tracing::trace!(?source, mix_id, ratio, effective, "scaling cell by new master");
                self.engine.send_command(PluginCommand::SetRouteVolume {
                    source,
                    mix: mix_id,
                    volume: effective,
                });
            }
        }
    }

    pub fn handle_channel_master_stereo_volume_changed(
        &mut self,
        source: SourceId,
        left: f32,
        right: f32,
    ) {
        let avg = (left + right) / 2.0;
        tracing::debug!(?source, left, right, avg, "channel master stereo volume changed");
        if let SourceId::Channel(ch_id) = source {
            self.channel_master_stereo.insert(ch_id, (left, right));
            // Keep mono master as average for backward compat
            self.channel_master_volumes.insert(ch_id, avg);
            if let Some(ch) = self
                .engine
                .state
                .channels
                .iter()
                .find(|c| c.id == ch_id)
            {
                if let Some(cfg) = self
                    .config
                    .channels
                    .iter_mut()
                    .find(|c| c.name == ch.name)
                {
                    cfg.master_volume = avg;
                }
            }
        }
        // Scale all routes using independent L/R masters via stereo volume command
        let mix_ids: Vec<_> = self.engine.state.mixes.iter().map(|m| m.id).collect();
        for mix_id in mix_ids {
            let key = (source, mix_id);
            if let Some(route) = self.engine.state.routes.get(&key) {
                // Derive L/R ratios from effective PA values / avg master
                let ratio_l = if avg > 0.001 {
                    (route.volume_left / avg).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let ratio_r = if avg > 0.001 {
                    (route.volume_right / avg).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let eff_l = (ratio_l * left).clamp(0.0, 1.0);
                let eff_r = (ratio_r * right).clamp(0.0, 1.0);
                tracing::trace!(
                    ?source, mix_id, ratio_l, ratio_r, eff_l, eff_r,
                    "scaling cell by stereo master"
                );
                self.engine.send_command(PluginCommand::SetRouteStereoVolume {
                    source,
                    mix: mix_id,
                    left: eff_l,
                    right: eff_r,
                });
            }
        }
    }
}
