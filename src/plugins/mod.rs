//! Audio backend plugins.
//!
//! Each plugin implements the `AudioPlugin` trait from `crate::plugin`.
//! Backend selection: PipeWire native if available, PulseAudio fallback.

pub mod pulseaudio;

#[cfg(feature = "pipewire-backend")]
pub mod pipewire;

use crate::plugin::AudioPlugin;

/// Detect whether PipeWire daemon is running.
#[allow(dead_code)]
fn is_pipewire_available() -> bool {
    let dir = std::env::var("PIPEWIRE_RUNTIME_DIR")
        .or_else(|_| std::env::var("XDG_RUNTIME_DIR"))
        .unwrap_or_default();
    let socket = std::path::Path::new(&dir).join("pipewire-0");
    let available = socket.exists();
    tracing::debug!(
        socket = %socket.display(),
        available,
        "PipeWire daemon detection"
    );
    available
}

/// Create the default plugin for the current platform.
///
/// Prefers PipeWire native when available (and compiled with `pipewire-backend` feature).
/// Falls back to PulseAudio (works through pipewire-pulse compatibility layer too).
pub fn create_default_plugin() -> Box<dyn AudioPlugin> {
    #[cfg(feature = "pipewire-backend")]
    {
        if is_pipewire_available() {
            tracing::info!("PipeWire detected — using native PipeWire backend");
            return Box::new(pipewire::PipeWirePlugin::new());
        }
        tracing::info!("PipeWire not detected — falling back to PulseAudio backend");
    }

    #[cfg(not(feature = "pipewire-backend"))]
    {
        tracing::info!("PipeWire backend not compiled — using PulseAudio backend");
    }

    Box::new(pulseaudio::PulseAudioPlugin::new())
}
