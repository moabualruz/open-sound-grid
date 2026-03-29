//! Application state container and initialization.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use tokio::sync::mpsc;

use crate::config::AppConfig;
use crate::effects::EffectsParams;
use crate::engine::MixerEngine;
use crate::plugin::api::{ChannelId, MixId, PluginEvent};
use crate::resolve::AppResolver;
use crate::config::RouteConfig;

use super::messages::ChannelPanelTab;

/// Global slot for the plugin event receiver.
/// Set once during boot, consumed once by the subscription stream.
pub(crate) static EVENT_RX: OnceLock<Mutex<Option<mpsc::UnboundedReceiver<PluginEvent>>>> = OnceLock::new();

/// Application state.
pub struct App {
    pub config: AppConfig,
    pub engine: MixerEngine,
    pub app_resolver: AppResolver,
    pub settings_open: bool,
    pub sidebar_collapsed: bool,
    /// (mix_name, device_name) pairs waiting to be applied after first StateRefreshed.
    pub pending_output_restores: Vec<(String, String)>,
    /// Routes waiting to be replayed after next StateRefreshed (used by LoadPreset).
    pub pending_route_restores: Vec<RouteConfig>,
    /// (channel_name, effects, muted) restores deferred until after first StateRefreshed.
    pub pending_effects_restores: Vec<(String, EffectsParams, bool)>,
    /// (mix_name, master_volume, muted) restores deferred until after first StateRefreshed.
    pub pending_mix_restores: Vec<(String, f32, bool)>,
    /// Keyboard focus: channel row index.
    pub focused_row: Option<usize>,
    /// Keyboard focus: mix column index.
    pub focused_col: Option<usize>,
    /// Text input for preset name entry.
    pub preset_name_input: String,
    /// List of saved preset names (refreshed after save/load).
    pub available_presets: Vec<String>,
    /// Currently selected channel for effects panel display.
    pub selected_channel: Option<ChannelId>,
    /// Stream index of the app currently being routed (two-step click workflow).
    /// Set when the user clicks an app; cleared after they click a channel label.
    pub routing_app: Option<u32>,
    /// Per-channel FFT spectrum data (populated when SpectrumData plugin events arrive).
    pub spectrum_data: HashMap<ChannelId, Vec<(f32, f32)>>,
    /// Whether the channel type picker is visible.
    pub show_channel_picker: bool,
    /// Last deleted item for undo support (name, was_channel).
    /// Cleared after 10 seconds or after undo is triggered.
    pub undo_buffer: Option<(String, bool)>,
    /// Channel currently being renamed (inline edit mode).
    pub editing_channel: Option<ChannelId>,
    /// Mix currently being renamed (inline edit mode).
    pub editing_mix: Option<MixId>,
    /// Current text in the rename input field.
    pub editing_text: String,
    /// Active tab in the channel side panel (Apps or Effects).
    pub channel_panel_tab: ChannelPanelTab,
    /// v0.4.0: Whether the channel creation dropdown is open.
    pub show_channel_dropdown: bool,
    /// v0.4.0: Search text in the channel creation dropdown.
    pub channel_search_text: String,
    /// v0.4.0: Whether the matrix is in compact (shrunk) single-mix view.
    pub compact_mix_view: bool,
    /// v0.4.0: Which mix to show in compact view (None = all channels).
    pub compact_selected_mix: Option<MixId>,
    /// v0.4.0: Copied effects chain for paste between channels.
    pub copied_effects: Option<crate::effects::EffectsParams>,
    /// v0.4.0: Channel name text in the settings panel name field.
    pub channel_settings_name: String,
    /// Which mix is currently monitored (heard in headphones). None = first mix.
    pub monitored_mix: Option<MixId>,
    /// Sound check buffer for mic record/playback.
    pub sound_check: crate::sound_check::SoundCheckBuffer,
    /// Whether auto-route creation has been sent (prevents feedback loop).
    pub(crate) auto_routes_sent: bool,
    /// Per-channel master volumes (UI-side, survives snapshot rebuilds).
    pub channel_master_volumes: HashMap<ChannelId, f32>,
    /// Per-channel stereo master volumes: (left, right). When absent, falls back to mono.
    pub channel_master_stereo: HashMap<ChannelId, (f32, f32)>,
    /// Channels whose routes have been initialized (prevents route churn on StateRefreshed).
    pub routes_initialized: std::collections::HashSet<ChannelId>,
    /// App binaries suppressed from auto-solo channel creation (after explicit unassign).
    pub suppressed_solo_apps: std::collections::HashSet<String>,
}

impl App {
    pub fn new() -> Self {
        tracing::info!("initializing App");
        let config = AppConfig::load();
        let app_resolver = AppResolver::new();

        let sidebar_collapsed = config.ui.compact_mode;
        tracing::debug!(
            compact_mode = config.ui.compact_mode,
            "applying compact_mode from config"
        );

        Self {
            config,
            engine: MixerEngine::new(),
            app_resolver,
            settings_open: false,
            sidebar_collapsed,
            pending_output_restores: Vec::new(),
            pending_route_restores: Vec::new(),
            pending_effects_restores: Vec::new(),
            pending_mix_restores: Vec::new(),
            focused_row: None,
            focused_col: None,
            preset_name_input: String::new(),
            available_presets: crate::presets::MixerPreset::list(),
            selected_channel: None,
            routing_app: None,
            spectrum_data: HashMap::new(),
            show_channel_picker: false,
            undo_buffer: None,
            editing_channel: None,
            editing_mix: None,
            editing_text: String::new(),
            channel_panel_tab: ChannelPanelTab::Apps,
            show_channel_dropdown: false,
            channel_search_text: String::new(),
            compact_mix_view: false,
            compact_selected_mix: None,
            copied_effects: None,
            channel_settings_name: String::new(),
            monitored_mix: None,
            sound_check: crate::sound_check::SoundCheckBuffer::new(5.0),
            auto_routes_sent: false,
            channel_master_volumes: HashMap::new(),
            channel_master_stereo: HashMap::new(),
            routes_initialized: std::collections::HashSet::new(),
            suppressed_solo_apps: std::collections::HashSet::new(),
        }
    }

    /// Store the plugin event receiver for the subscription to consume.
    pub fn set_event_receiver(rx: mpsc::UnboundedReceiver<PluginEvent>) {
        tracing::debug!("storing plugin event receiver in global slot");
        let _ = EVENT_RX.set(Mutex::new(Some(rx)));
    }
}
