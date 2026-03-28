//! Audio backend plugins.
//!
//! Each plugin implements the `AudioPlugin` trait from `crate::plugin`.
//! Plugins are selected at compile time via Cargo features,
//! or at runtime based on OS detection.

pub mod pulseaudio;

use crate::plugin::AudioPlugin;

/// Create the default plugin for the current platform.
pub fn create_default_plugin() -> Box<dyn AudioPlugin> {
    // TODO: runtime detection (check if PulseAudio server is running, etc.)
    // For now, always use PulseAudio on Linux.
    Box::new(pulseaudio::PulseAudioPlugin::new())
}
