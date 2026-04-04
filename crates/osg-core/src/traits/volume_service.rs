//! VolumeService — focused trait for volume and mute mutations.

use crate::graph::EndpointDescriptor;

/// Focused trait for volume and mute mutations on any endpoint.
///
/// Consumers depend on this trait, not on OsgCore or ReducerHandle directly.
pub trait VolumeService {
    /// Set a mono (equal L/R) volume on an endpoint. Range: 0.0–1.0+.
    fn set_volume(&self, endpoint: EndpointDescriptor, volume: f32);

    /// Set independent left/right channel volumes on an endpoint.
    fn set_stereo_volume(&self, endpoint: EndpointDescriptor, left: f32, right: f32);

    /// Mute or unmute an endpoint.
    fn set_mute(&self, endpoint: EndpointDescriptor, muted: bool);
}
