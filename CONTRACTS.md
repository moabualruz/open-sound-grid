# OpenSoundGrid — Agent Interface Contracts

This file defines the boundaries between parallel implementation agents.
DO NOT DEVIATE from these signatures without explicit instruction.

## Shared Types (DO NOT REDEFINE)

All agents use types from these existing files:
- `src/plugin/api.rs` — SourceId, MixId, ChannelId, AppId, OutputId, RouteState, ChannelInfo, MixInfo, AudioApplication, HardwareInput, HardwareOutput, MixerSnapshot, PluginCommand, PluginResponse, PluginEvent
- `src/engine/state.rs` — MixerState
- `src/error.rs` — OsgError, Result
- `src/ui/theme.rs` — all color constants

## Contract A: PulseAudio Connection (`plugins/pulseaudio/connection.rs`)

```rust
use libpulse_binding::context::Context;
use libpulse_binding::mainloop::threaded::Mainloop;
use std::sync::{Arc, Mutex};

/// Wraps PA mainloop + context. Created once, shared by all PA sub-modules.
pub struct PulseConnection {
    pub mainloop: Arc<Mutex<Mainloop>>,
    pub context: Arc<Mutex<Context>>,
}

impl PulseConnection {
    /// Connect to PA server. Blocks until ready or timeout.
    pub fn connect() -> crate::error::Result<Self>;
    /// Disconnect and clean up.
    pub fn disconnect(&self);
    /// Check if connected.
    pub fn is_connected(&self) -> bool;
}
```

## Contract B: PA Module Manager (`plugins/pulseaudio/modules.rs`)

```rust
/// Manages null sinks and loopback modules.
pub struct ModuleManager {
    // internal tracking of loaded module IDs
}

impl ModuleManager {
    pub fn new() -> Self;
    /// Load a null sink, return its PA module ID.
    pub fn create_null_sink(&mut self, conn: &PulseConnection, name: &str, description: &str) -> crate::error::Result<u32>;
    /// Load a loopback module, return its PA module ID.
    pub fn create_loopback(&mut self, conn: &PulseConnection, source_monitor: &str, sink: &str, latency_ms: u32) -> crate::error::Result<u32>;
    /// Unload a module by ID.
    pub fn unload_module(&mut self, conn: &PulseConnection, module_id: u32) -> crate::error::Result<()>;
    /// Unload all modules we've created.
    pub fn unload_all(&mut self, conn: &PulseConnection);
    /// Find the sink-input index for a loopback module (by pulse.module.id match).
    pub fn find_loopback_sink_input(&self, conn: &PulseConnection, module_id: u32) -> crate::error::Result<Option<u32>>;
    /// Set volume on a sink-input (0.0-1.0).
    pub fn set_sink_input_volume(&self, conn: &PulseConnection, sink_input_idx: u32, volume: f32) -> crate::error::Result<()>;
    /// Mute/unmute a sink-input.
    pub fn set_sink_input_mute(&self, conn: &PulseConnection, sink_input_idx: u32, muted: bool) -> crate::error::Result<()>;
    /// Move a sink-input to a different sink.
    pub fn move_sink_input(&self, conn: &PulseConnection, sink_input_idx: u32, sink_name: &str) -> crate::error::Result<()>;
}
```

## Contract C: PA App Detector (`plugins/pulseaudio/apps.rs`)

```rust
use crate::plugin::api::AudioApplication;

/// Detects running audio applications via PA sink-input introspection.
pub struct AppDetector;

impl AppDetector {
    pub fn new() -> Self;
    /// List all current audio applications (sink-inputs with application.name).
    pub fn list_applications(&self, conn: &PulseConnection) -> Vec<AudioApplication>;
}
```

## Contract D: PA Peak Monitor (`plugins/pulseaudio/peaks.rs`)

```rust
use crate::plugin::api::SourceId;
use std::collections::HashMap;

/// Monitors peak levels via PA monitor sources.
pub struct PeakMonitor {
    // internal state
}

impl PeakMonitor {
    pub fn new() -> Self;
    /// Start monitoring a sink's output level.
    pub fn monitor_sink(&mut self, conn: &PulseConnection, sink_name: &str, source_id: SourceId) -> crate::error::Result<()>;
    /// Stop monitoring a sink.
    pub fn stop_monitoring(&mut self, source_id: &SourceId);
    /// Get latest peak levels for all monitored sources.
    pub fn get_levels(&self) -> HashMap<SourceId, f32>;
    /// Stop all monitoring.
    pub fn stop_all(&mut self);
}
```

## Contract E: PA Device Enumerator (`plugins/pulseaudio/devices.rs`)

```rust
use crate::plugin::api::{HardwareInput, HardwareOutput};

/// Enumerates hardware audio devices.
pub struct DeviceEnumerator;

impl DeviceEnumerator {
    /// List hardware output sinks (filters out virtual/null sinks).
    pub fn list_outputs(conn: &PulseConnection) -> Vec<HardwareOutput>;
    /// List hardware input sources (filters out monitor sources).
    pub fn list_inputs(conn: &PulseConnection) -> Vec<HardwareInput>;
}
```

## Contract F: App Name/Icon Resolver (`src/resolve.rs`)

```rust
/// Resolves PA application metadata to friendly names and icon paths.
pub struct AppResolver {
    // caches desktop entries
}

impl AppResolver {
    pub fn new() -> Self;
    /// Given a binary name (e.g., "chromium"), return (display_name, icon_path).
    pub fn resolve(&self, binary: &str, pa_app_name: Option<&str>) -> (String, Option<std::path::PathBuf>);
}
```

## Contract G: UI Widget Signatures

All widgets live in `src/ui/` and return `Element<'a, crate::app::Message>`.

```rust
// src/ui/vu_meter.rs
/// Canvas-based horizontal VU meter with green/amber/red gradient.
pub fn vu_meter<'a>(level: f32, width: f32, height: f32) -> Element<'a, Message>;

// src/ui/audio_slider.rs
/// Horizontal volume slider with dB label. on_change fires continuously during drag.
pub fn audio_slider<'a>(value: f32, on_change: impl Fn(f32) -> Message + 'a) -> Element<'a, Message>;

// src/ui/sidebar.rs
pub struct SidebarState { pub collapsed: bool, pub active_section: SidebarSection }
pub enum SidebarSection { Devices, Mixes, Settings }
pub fn sidebar<'a>(state: &SidebarState, devices: &[HardwareInput]) -> Element<'a, Message>;

// src/ui/matrix.rs — the big one
/// Full matrix grid: input rows × mix columns with sliders, mutes, VU meters.
pub fn matrix_grid<'a>(state: &MixerState) -> Element<'a, Message>;
```

## Contract H: Message Enum Extensions

Agents adding new UI interactions must document their Message variants here.
The integration agent will merge them into `src/app.rs`.

```rust
// Already defined:
RouteVolumeChanged { source, mix, volume }
RouteToggled { source, mix }
MixMasterVolumeChanged { mix, volume }
MixMuteToggled(MixId)
SourceMuteToggled(SourceId)
AppRouteChanged { app_index, channel_index }
RefreshApps
PluginError(String)
SettingsToggled
Tick

// New variants agents may need:
SidebarToggleCollapse
SidebarSectionChanged(SidebarSection)
CreateChannel(String)
RemoveChannel(ChannelId)
CreateMix(String)
RemoveMix(MixId)
OutputDeviceChanged(OutputId)
RouteEnable { source: SourceId, mix: MixId }
```

## Registration: `src/ui/mod.rs`

Each UI agent adds their module here. Assigned slots:
```rust
pub mod app_list;      // existing
pub mod audio_slider;  // Agent: slider
pub mod matrix;        // Agent: matrix
pub mod sidebar;       // Agent: sidebar
pub mod theme;         // existing
pub mod vu_meter;      // Agent: vu_meter
```

## Registration: `src/plugins/pulseaudio/`

```rust
// mod.rs (integration agent rewrites this)
mod apps;
mod connection;
mod devices;
mod modules;
mod peaks;
```
