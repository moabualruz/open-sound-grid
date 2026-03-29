//! All UI message types for the application.

use crate::effects::EffectsParams;
use crate::plugin::api::{ChannelId, MixId, MixerSnapshot, SourceId};

/// Tab selection for the channel side panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelPanelTab {
    Apps,
    Effects,
}

/// All possible UI messages.
#[derive(Debug, Clone)]
pub enum Message {
    // Matrix interactions
    RouteVolumeChanged {
        source: SourceId,
        mix: MixId,
        volume: f32,
    },
    RouteToggled {
        source: SourceId,
        mix: MixId,
    },

    // Mix controls
    MixMasterVolumeChanged {
        mix: MixId,
        volume: f32,
    },
    MixMuteToggled(MixId),

    // Source controls (mutes channel across ALL mixes)
    SourceMuteToggled(SourceId),

    // Route-level mute (mutes one source in one specific mix)
    RouteMuteToggled {
        source: SourceId,
        mix: MixId,
    },

    // Application routing
    /// Assign an app to a channel (checkbox checked in channel settings panel).
    AssignApp {
        channel: ChannelId,
        stream_index: u32,
    },
    /// Unassign an app from a channel (checkbox unchecked in channel settings panel).
    UnassignApp {
        channel: ChannelId,
        stream_index: u32,
    },
    /// Switch between Apps/Effects tabs in the channel side panel.
    ChannelPanelTab(ChannelPanelTab),
    #[allow(dead_code)]
    AppRouteChanged {
        app_index: u32,
        channel_index: u32,
    },
    /// User clicked an app entry to begin routing — next channel click will assign it.
    AppRoutingStarted(u32),
    #[allow(dead_code)]
    RefreshApps,

    // Channel/mix creation
    CreateChannel(String),
    CreateMix(String),
    /// Toggle the channel type picker visibility.
    ToggleChannelPicker,

    // Channel/mix removal (with undo support)
    RemoveChannel(ChannelId),
    RemoveMix(MixId),
    /// Move a channel up in the list.
    MoveChannelUp(ChannelId),
    /// Move a channel down in the list.
    MoveChannelDown(ChannelId),
    /// Undo the last delete operation.
    UndoDelete,
    /// Clear the undo buffer (called by timer).
    ClearUndo,

    // Inline rename (double-click)
    StartRenameChannel(ChannelId),
    StartRenameMix(MixId),
    RenameInput(String),
    ConfirmRename,
    CancelRename,
    RenameChannel {
        id: ChannelId,
        name: String,
    },
    RenameMix {
        id: MixId,
        name: String,
    },

    // Plugin events (from async subscription — zero latency)
    PluginStateRefreshed(MixerSnapshot),
    PluginDevicesChanged,
    PluginAppsChanged(Vec<crate::plugin::api::AudioApplication>),
    PluginPeakLevels(std::collections::HashMap<SourceId, f32>),
    /// FFT spectrum data received from plugin (future use).
    PluginSpectrumData {
        channel: ChannelId,
        bins: Vec<(f32, f32)>,
    },
    PluginError(String),
    PluginConnectionLost,
    PluginConnectionRestored,

    // Tray commands
    TrayShow,
    TrayQuit,
    TrayMuteAll,

    // Hotkey events (from global shortcut subscription)
    HotkeyMuteAll,

    // Window events
    WindowResized(u32, u32),

    // Keyboard
    KeyPressed(iced::keyboard::Key, iced::keyboard::Modifiers),

    // Output device selection
    MixOutputDeviceSelected {
        mix: MixId,
        device_name: String,
    },

    // Effects
    EffectsToggled {
        channel: ChannelId,
        enabled: bool,
    },
    EffectsParamChanged {
        channel: ChannelId,
        param: String,
        value: f32,
    },
    #[allow(dead_code)]
    SelectedChannel(Option<ChannelId>),

    // UI
    SettingsToggled,
    SidebarToggleCollapse,
    ThemeToggled,

    /// Set which mix is currently being monitored (heard through headphones).
    MonitorMix(MixId),

    // v0.4.0: Channel creation dropdown
    /// Toggle the channel creation dropdown visibility.
    ToggleChannelDropdown,
    /// User typed in the channel creation search field.
    ChannelSearchInput(String),
    /// Create a channel directly from a detected app (by stream_index).
    CreateChannelFromApp(u32),

    // v0.4.0: Shrink/expand mixes view
    /// Toggle between full matrix and single-mix compact view.
    ToggleMixesView,
    /// In compact view, select which single mix to show.
    SelectCompactMix(Option<MixId>),

    // v0.4.0: Effects copy/paste
    /// Copy the selected channel's effects chain to clipboard.
    CopyEffects(ChannelId),
    /// Paste the copied effects chain to the selected channel.
    PasteEffects(ChannelId),

    // v0.4.0: Channel name editing in settings panel
    /// User edited the channel name in the settings panel text input.
    ChannelSettingsNameInput(String),
    /// Confirm channel name change from settings panel.
    ChannelSettingsNameConfirm(ChannelId),

    // Presets
    SavePreset(String),
    LoadPreset(String),
    PresetNameInput(String),

    // Channel master volume (scales all routes for a channel proportionally)
    ChannelMasterVolumeChanged {
        source: SourceId,
        volume: f32,
    },

    // Settings
    ToggleStereoSliders,

    // Latency setting
    LatencyInput(String),

    // Sound Check
    SoundCheckStart,
    SoundCheckStop,
    SoundCheckPlayback,
    SoundCheckStopPlayback,
    SoundCheckSamples(Vec<f32>),
}
