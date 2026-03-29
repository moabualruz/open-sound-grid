//! Deferred restore logic called during `handle_plugin_state_refreshed`.
//!
//! Applies pending output-device, route, effects, and mix restores that were
//! queued at startup and must wait until the engine state is fully populated.

use crate::effects::EffectsParams;
use crate::plugin::api::{PluginCommand, SourceId};

use crate::app::state::App;

impl App {
    /// Apply all deferred restores and auto-create missing routes.
    ///
    /// Called from `handle_plugin_state_refreshed` once the snapshot has been
    /// applied. `state_ready` is `true` when all expected channels and mixes
    /// from config are present in the engine state.
    pub(crate) fn apply_deferred_restores(&mut self, state_ready: bool) {
        if !state_ready
            && (!self.pending_output_restores.is_empty()
                || !self.pending_route_restores.is_empty()
                || !self.pending_effects_restores.is_empty()
                || !self.pending_mix_restores.is_empty())
        {
            tracing::debug!(
                channels = self.engine.state.channels.len(),
                mixes = self.engine.state.mixes.len(),
                "deferring pending restores — state not ready yet"
            );
            // Skip all restores this round; they'll fire on the next StateRefreshed
            // when channels and mixes have been created.
        }

        // Apply any pending output device restores from config.
        // Keep unresolved entries for the next snapshot (don't drain if hw not found).
        let mut _output_restore_applied = false;
        if state_ready
            && !self.pending_output_restores.is_empty()
            && !self.engine.state.hardware_outputs.is_empty()
        {
            _output_restore_applied = true;
            let restores = std::mem::take(&mut self.pending_output_restores);
            let mut deferred = Vec::new();
            for (mix_name, device_name) in restores {
                let mix_id = self
                    .engine
                    .state
                    .mixes
                    .iter()
                    .find(|m| m.name == mix_name)
                    .map(|m| m.id);
                let hw_id = self
                    .engine
                    .state
                    .hardware_outputs
                    .iter()
                    .find(|o| o.name == device_name)
                    .map(|o| o.id);
                match (mix_id, hw_id) {
                    (Some(mix), Some(output)) => {
                        tracing::info!(
                            %mix_name,
                            %device_name,
                            mix_id = mix,
                            output_id = output,
                            "restoring output device from config"
                        );
                        self.engine
                            .send_command(PluginCommand::SetMixOutput { mix, output });
                    }
                    _ => {
                        tracing::warn!(
                            %mix_name,
                            %device_name,
                            mix_found = mix_id.is_some(),
                            device_found = hw_id.is_some(),
                            hw_outputs_available = self.engine.state.hardware_outputs.len(),
                            "output device restore deferred — will retry on next snapshot"
                        );
                        deferred.push((mix_name, device_name));
                    }
                }
            }
            // Keep unresolved restores for the next snapshot
            self.pending_output_restores = deferred;
        }

        // Default: assign system default output to Main/Monitor ONLY if:
        // 1. The mix has no output set in the engine state AND
        // 2. No config entry for this mix has a saved output_device AND
        // 3. No pending output restores exist for this mix
        // This prevents overriding the user's saved output device choice.
        // Default output: removed. User controls output device via the per-mix
        // dropdown. Config restores handle startup. No auto-assignment.

        // Apply any pending route restores (from LoadPreset)
        if state_ready && !self.pending_route_restores.is_empty() {
            let restores = std::mem::take(&mut self.pending_route_restores);
            tracing::info!(count = restores.len(), "applying pending route restores");
            for route in restores {
                let ch_id = self
                    .engine
                    .state
                    .channels
                    .iter()
                    .find(|c| c.name == route.channel_name)
                    .map(|c| c.id);
                let mix_id = self
                    .engine
                    .state
                    .mixes
                    .iter()
                    .find(|m| m.name == route.mix_name)
                    .map(|m| m.id);
                match (ch_id, mix_id) {
                    (Some(ch), Some(mix)) => {
                        let source = SourceId::Channel(ch);
                        tracing::debug!(
                            channel = %route.channel_name,
                            mix = %route.mix_name,
                            volume = route.volume,
                            enabled = route.enabled,
                            muted = route.muted,
                            "restoring route from preset"
                        );
                        if route.enabled {
                            self.engine.send_command(PluginCommand::SetRouteEnabled {
                                source,
                                mix,
                                enabled: true,
                            });

                            // WL3: route.volume from config is the effective PA volume.
                            // Recover the cell ratio and store it for linked slider behavior.
                            let ch_master = self
                                .channel_master_volumes
                                .get(&ch)
                                .copied()
                                .unwrap_or(1.0);
                            let ratio = if ch_master > 0.001 {
                                (route.volume / ch_master).clamp(0.0, 1.0)
                            } else {
                                1.0
                            };
                            self.engine
                                .state
                                .route_ratios
                                .insert((source, mix), ratio);
                            tracing::debug!(
                                channel = %route.channel_name,
                                mix = %route.mix_name,
                                saved_vol = route.volume,
                                ch_master,
                                ratio,
                                "route restore: recovered ratio from saved effective volume"
                            );

                            // Send the effective volume to PA
                            let effective = (ratio * ch_master).clamp(0.0, 1.0);
                            self.engine.send_command(PluginCommand::SetRouteVolume {
                                source,
                                mix,
                                volume: effective,
                            });
                            if route.muted {
                                self.engine.send_command(PluginCommand::SetRouteMuted {
                                    source,
                                    mix,
                                    muted: true,
                                });
                            }
                        }
                    }
                    _ => {
                        tracing::warn!(
                            channel = %route.channel_name,
                            mix = %route.mix_name,
                            channel_found = ch_id.is_some(),
                            mix_found = mix_id.is_some(),
                            "pending route restore: channel or mix not found"
                        );
                    }
                }
            }
        }

        // Auto-create routes for any channel that has NO routes to any mix.
        // This handles both first startup AND newly created channels (solo apps).
        if state_ready {
            let mut new_routes = 0u32;
            for ch in &self.engine.state.channels {
                let has_any_route = self.engine.state.mixes.iter().any(|mx| {
                    self.engine
                        .state
                        .routes
                        .contains_key(&(SourceId::Channel(ch.id), mx.id))
                });
                if !has_any_route {
                    for mx in &self.engine.state.mixes {
                        self.engine.send_command(PluginCommand::SetRouteEnabled {
                            source: SourceId::Channel(ch.id),
                            mix: mx.id,
                            enabled: true,
                        });
                        new_routes += 1;
                    }
                }
            }
            if new_routes > 0 {
                tracing::info!(new_routes, "auto-enabled routes for channels without routes");
            }
        }

        // Apply any pending effects/muted restores from config
        if state_ready && !self.pending_effects_restores.is_empty() {
            let restores = std::mem::take(&mut self.pending_effects_restores);
            tracing::info!(count = restores.len(), "applying pending effects restores");
            for (name, effects, muted) in restores {
                if let Some(ch) = self.engine.state.channels.iter().find(|c| c.name == name)
                {
                    if muted {
                        tracing::debug!(channel = %name, "restoring channel mute from config");
                        self.engine.send_command(PluginCommand::SetSourceMuted {
                            source: SourceId::Channel(ch.id),
                            muted: true,
                        });
                    }
                    if effects.enabled || effects != EffectsParams::default() {
                        tracing::debug!(
                            channel = %name,
                            enabled = effects.enabled,
                            "restoring effects params from config"
                        );
                        self.engine.send_command(PluginCommand::SetEffectsParams {
                            channel: ch.id,
                            params: effects,
                        });
                    }
                } else {
                    tracing::warn!(channel = %name, "pending effects restore: channel not found");
                }
            }
        }

        // Apply any pending mix volume/muted restores from config
        if state_ready && !self.pending_mix_restores.is_empty() {
            let restores = std::mem::take(&mut self.pending_mix_restores);
            tracing::info!(
                count = restores.len(),
                "applying pending mix volume restores"
            );
            for (name, volume, muted) in restores {
                if let Some(m) = self.engine.state.mixes.iter().find(|m| m.name == name) {
                    if (volume - 1.0).abs() > 0.001 {
                        tracing::debug!(mix = %name, volume, "restoring mix master volume from config");
                        self.engine.send_command(PluginCommand::SetMixMasterVolume {
                            mix: m.id,
                            volume,
                        });
                    }
                    if muted {
                        tracing::debug!(mix = %name, "restoring mix mute from config");
                        self.engine.send_command(PluginCommand::SetMixMuted {
                            mix: m.id,
                            muted: true,
                        });
                    }
                } else {
                    tracing::warn!(mix = %name, "pending mix restore: mix not found");
                }
            }
        }
    }
}
