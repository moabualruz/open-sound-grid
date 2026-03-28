pub mod bridge;
pub mod pulse;
pub mod types;

pub use types::*;

use crate::error::Result;

/// Abstraction over audio backends (PulseAudio, PipeWire).
///
/// All methods that mutate state are `&mut self`.
/// The backend runs in its own thread; communication with the UI
/// happens through the `bridge` module's mpsc channels.
pub trait AudioBackend: Send {
    /// Initialize the backend: connect to the audio server, discover devices.
    fn init(&mut self) -> Result<()>;

    /// Get a full snapshot of the current mixer state.
    fn get_state(&self) -> Result<MixerState>;

    /// List hardware audio inputs (mics, interfaces).
    fn list_hardware_inputs(&self) -> Result<Vec<HardwareInput>>;

    /// List hardware audio outputs (speakers, headphones).
    fn list_hardware_outputs(&self) -> Result<Vec<HardwareOutput>>;

    /// List currently running applications with audio streams.
    fn list_applications(&self) -> Result<Vec<AudioApplication>>;

    /// Create a new software channel (creates a PA null sink).
    fn create_channel(&mut self, name: &str) -> Result<ChannelId>;

    /// Remove a software channel (unloads its PA modules).
    fn remove_channel(&mut self, id: ChannelId) -> Result<()>;

    /// Create a new output mix (creates a PA virtual sink + loopbacks).
    fn create_mix(&mut self, name: &str) -> Result<MixId>;

    /// Remove an output mix.
    fn remove_mix(&mut self, id: MixId) -> Result<()>;

    /// Set volume for a source in a specific mix (controls a loopback sink-input).
    fn set_route_volume(&mut self, source: SourceId, mix: MixId, volume: f32) -> Result<()>;

    /// Enable/disable routing of a source to a mix.
    fn set_route_enabled(&mut self, source: SourceId, mix: MixId, enabled: bool) -> Result<()>;

    /// Route an application's audio to a channel.
    fn route_app_to_channel(&mut self, app: AppId, channel: ChannelId) -> Result<()>;

    /// Set the hardware output device for a mix.
    fn set_mix_output(&mut self, mix: MixId, output: OutputId) -> Result<()>;

    /// Set master volume for a mix.
    fn set_mix_master_volume(&mut self, mix: MixId, volume: f32) -> Result<()>;

    /// Clean up: unload all modules, disconnect.
    fn cleanup(&mut self) -> Result<()>;
}
