//! Effects command handlers: set params, set enabled.

use crate::effects::EffectsParams;
use crate::error::Result;
use crate::plugin::api::*;

use super::PulseAudioPlugin;

impl PulseAudioPlugin {
    pub(crate) fn handle_set_effects_params(
        &mut self,
        channel: ChannelId,
        params: EffectsParams,
    ) -> Result<PluginResponse> {
        tracing::debug!(
            channel_id = channel,
            enabled = params.enabled,
            eq_freq = params.eq_freq_hz,
            comp_threshold = params.comp_threshold_db,
            "setting effects params for channel"
        );
        if let Some(chain) = self.effects_chains.get_mut(&channel) {
            chain.set_params(params.clone());
        } else {
            tracing::warn!(
                channel_id = channel,
                "SetEffectsParams: no effects chain found for channel"
            );
        }
        // Sync params into ChannelInfo so snapshots reflect current state
        if let Some(ch) = self.channels.iter_mut().find(|c| c.id == channel) {
            ch.effects = params;
        }
        Ok(PluginResponse::Ok)
    }

    pub(crate) fn handle_set_effects_enabled(
        &mut self,
        channel: ChannelId,
        enabled: bool,
    ) -> Result<PluginResponse> {
        tracing::debug!(
            channel_id = channel,
            enabled,
            "setting effects enabled for channel"
        );
        if let Some(chain) = self.effects_chains.get_mut(&channel) {
            chain.set_enabled(enabled);
        } else {
            tracing::warn!(
                channel_id = channel,
                "SetEffectsEnabled: no effects chain found for channel"
            );
        }
        // Sync into ChannelInfo
        if let Some(ch) = self.channels.iter_mut().find(|c| c.id == channel) {
            ch.effects.enabled = enabled;
        }
        Ok(PluginResponse::Ok)
    }
}
