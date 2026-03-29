//! Configuration persistence: restore channels, mixes, routes from saved config.

use crate::config::RouteConfig;
use crate::effects::EffectsParams;
use crate::plugin::api::PluginCommand;

use super::state::App;

impl App {
    /// Recreate channels and mixes from persisted config.
    /// Call after the plugin bridge is attached.
    pub fn restore_from_config(&mut self) {
        for ch in &self.config.channels {
            tracing::debug!(name = %ch.name, "restoring channel from config");
            self.engine.send_command(PluginCommand::CreateChannel {
                name: ch.name.clone(),
            });
        }
        for mx in &self.config.mixes {
            tracing::debug!(name = %mx.name, "restoring mix from config");
            self.engine.send_command(PluginCommand::CreateMix {
                name: mx.name.clone(),
            });
        }
        if !self.config.channels.is_empty() || !self.config.mixes.is_empty() {
            tracing::debug!(
                channels = self.config.channels.len(),
                mixes = self.config.mixes.len(),
                "config restore complete, requesting state refresh"
            );
            self.engine.send_command(PluginCommand::GetState);
        }

        // Output device, effects, and mix volume restores happen after the first
        // StateRefreshed arrives because channel/mix IDs aren't known until the
        // plugin creates them.
        self.pending_output_restores = self
            .config
            .mixes
            .iter()
            .filter_map(|m| {
                m.output_device
                    .as_ref()
                    .map(|d| (m.name.clone(), d.clone()))
            })
            .collect();
        tracing::debug!(
            count = self.pending_output_restores.len(),
            "config restore: queued output devices for restoration after first state refresh"
        );

        self.pending_effects_restores = self
            .config
            .channels
            .iter()
            .map(|c| (c.name.clone(), c.effects.clone(), c.muted))
            .collect();
        tracing::debug!(
            count = self.pending_effects_restores.len(),
            "config restore: queued effects params for restoration after first state refresh"
        );

        // Restore routes: prefer config.routes (always saved), fall back to _last_session preset
        if !self.config.routes.is_empty() {
            tracing::info!(
                count = self.config.routes.len(),
                "restoring routes from config"
            );
            self.pending_route_restores = self.config.routes.clone();
        } else if crate::presets::MixerPreset::list().contains(&"_last_session".to_string()) {
            if let Ok(last) = crate::presets::MixerPreset::load("_last_session") {
                tracing::info!("restoring _last_session preset routes (config had none)");
                self.pending_route_restores = last
                    .routes
                    .iter()
                    .map(|r| RouteConfig {
                        channel_name: r.channel_name.clone(),
                        mix_name: r.mix_name.clone(),
                        volume: r.volume,
                        enabled: r.enabled,
                        muted: r.muted,
                        volume_left: r.volume_left,
                        volume_right: r.volume_right,
                    })
                    .collect();
            }
        }

        self.pending_mix_restores = self
            .config
            .mixes
            .iter()
            .map(|m| (m.name.clone(), m.master_volume, m.muted))
            .collect();
        tracing::debug!(
            count = self.pending_mix_restores.len(),
            "config restore: queued mix volumes for restoration after first state refresh"
        );
    }
}
