//! Plugin system for audio backends.
//!
//! The core application communicates with audio backends through the `AudioPlugin` trait.
//! Each plugin (PulseAudio, PipeWire, CoreAudio, etc.) implements this trait.
//!
//! Communication is command/event based — no shared state between core and plugin.
//! The plugin runs in its own thread; commands and events flow through channels.

pub mod api;
pub mod manager;

pub use api::*;

use crate::error::Result;

/// Current plugin API version.
pub const API_VERSION: u32 = 1;

/// Declares what a plugin can do.
/// Core adapts the UI to available capabilities.
#[derive(Debug, Clone)]
pub struct PluginCapabilities {
    /// Can create virtual sinks/channels for app routing.
    pub can_create_virtual_sinks: bool,
    /// Can move application audio streams between sinks.
    pub can_route_applications: bool,
    /// Can provide real-time peak level monitoring.
    pub can_monitor_peaks: bool,
    /// Can apply audio effects inline (future).
    pub can_apply_effects: bool,
    /// Can lock device selection (survive OS changes).
    pub can_lock_devices: bool,
    /// Maximum number of software channels (None = unlimited).
    pub max_channels: Option<u32>,
    /// Maximum number of output mixes (None = unlimited).
    pub max_mixes: Option<u32>,
}

/// Plugin identity and metadata.
#[derive(Debug, Clone)]
pub struct PluginInfo {
    /// Unique identifier: "pulseaudio", "pipewire", "coreaudio"
    pub id: &'static str,
    /// Human-readable name: "PulseAudio"
    pub name: &'static str,
    /// Plugin version: "0.1.0"
    pub version: &'static str,
    /// API version this plugin was built against.
    pub api_version: u32,
    /// Target OS: "linux", "macos", "windows"
    pub os: &'static str,
}

/// The core plugin trait.
///
/// Implement this for each audio backend. The plugin runs in its own thread.
/// All methods are called from that thread — no Send/Sync concerns for internal state.
pub trait AudioPlugin: Send {
    /// Return plugin identity and metadata.
    fn info(&self) -> PluginInfo;

    /// Declare capabilities.
    fn capabilities(&self) -> PluginCapabilities;

    /// Initialize: connect to audio server, discover devices.
    fn init(&mut self) -> Result<()>;

    /// Handle a command from the core mixer engine.
    fn handle_command(&mut self, cmd: PluginCommand) -> Result<PluginResponse>;

    /// Poll for asynchronous events (called periodically from the plugin thread).
    fn poll_events(&mut self) -> Vec<PluginEvent>;

    /// Clean up: unload modules, disconnect.
    fn cleanup(&mut self) -> Result<()>;
}
