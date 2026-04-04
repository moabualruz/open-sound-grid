// RuntimeState — transient fields that are NOT part of the domain aggregate.
//
// MixerSession is the DDD aggregate root and must be pure domain state:
// serializable, deserializable, and free of PipeWire runtime handles.
// All fields that are runtime-only (not persisted, not domain intent) live here.
//
// The reducer owns both MixerSession and RuntimeState and passes RuntimeState
// to reconciliation and handler functions that need it.

use std::collections::{HashMap, HashSet};

use super::endpoint::EndpointDescriptor;
use super::identifiers::{AppId, ChannelId};
use super::link::LinkKey;
use super::node_identity::NodeIdentity;
use super::port_kind::PortKind;

#[derive(Debug, Default)]
pub struct RuntimeState {
    // ---- MixerSession-level runtime fields --------------------------------
    /// PW nodes not claimed by any endpoint — offered as candidates in the UI.
    pub candidates: Vec<(u32, PortKind, NodeIdentity)>,

    /// PipeWire node ID of the OS default audio sink.
    /// Updated from PipeWire metadata `default.audio.sink`.
    pub default_output_node_id: Option<u32>,

    /// Cell node names already created (prevents duplicate reconciliation).
    pub created_cells: HashSet<String>,

    /// Monotonically increasing counter bumped on every `update()` call.
    /// Used by the reducer to skip reconciliation when only graph events arrive.
    pub generation: u64,

    /// ULID of the current OSG instance. Stamped on all PW nodes for
    /// ownership tracking. Stale nodes from a crashed previous instance
    /// are reaped on startup.
    pub instance_id: ulid::Ulid,

    /// PW node ID of the staging sink (vol=0, for glitch-free rerouting).
    pub staging_node_id: Option<u32>,

    // ---- Per-Endpoint runtime fields (keyed by EndpointDescriptor) --------
    /// True while a volume/mute command is in-flight to PipeWire.
    pub endpoint_volume_pending: HashMap<EndpointDescriptor, bool>,

    /// Cached volume before mute (restored on unmute).
    /// Null-audio-sinks don't honor SPA_PROP_mute, so mute = volume → 0.
    pub endpoint_pre_mute_volume: HashMap<EndpointDescriptor, Option<(f32, f32)>>,

    // ---- Per-Channel runtime fields (keyed by ChannelId) ------------------
    /// PipeWire node ID for sink (mix) channels. None until PW confirms creation.
    pub channel_pipewire_id: HashMap<ChannelId, Option<u32>>,

    /// True while a create-group-node command is in-flight for this channel.
    pub channel_pending: HashMap<ChannelId, bool>,

    // ---- Per-App runtime fields (keyed by AppId) --------------------------
    /// True when the app currently has active streams in the PW graph.
    pub app_is_active: HashMap<AppId, bool>,

    // ---- Per-Link runtime fields (keyed by LinkKey) -----------------------
    /// True while a link command is in-flight to PipeWire.
    pub link_pending: HashMap<LinkKey, bool>,
}

impl RuntimeState {
    /// Remove all runtime state associated with an endpoint.
    pub fn remove_endpoint(&mut self, ep: EndpointDescriptor) {
        self.endpoint_volume_pending.remove(&ep);
        self.endpoint_pre_mute_volume.remove(&ep);
    }

    /// Remove all runtime state associated with a channel.
    pub fn remove_channel(&mut self, id: ChannelId) {
        self.channel_pipewire_id.remove(&id);
        self.channel_pending.remove(&id);
    }

    /// Remove all runtime state associated with an app.
    pub fn remove_app(&mut self, id: AppId) {
        self.app_is_active.remove(&id);
    }

    /// Remove all runtime state associated with a link.
    pub fn remove_link(&mut self, key: &LinkKey) {
        self.link_pending.remove(key);
    }

    // ---- Accessors with ergonomic defaults --------------------------------

    pub fn volume_pending(&self, ep: &EndpointDescriptor) -> bool {
        self.endpoint_volume_pending
            .get(ep)
            .copied()
            .unwrap_or(false)
    }

    pub fn set_volume_pending(&mut self, ep: EndpointDescriptor, pending: bool) {
        if pending {
            self.endpoint_volume_pending.insert(ep, true);
        } else {
            self.endpoint_volume_pending.remove(&ep);
        }
    }

    pub fn pre_mute_volume(&self, ep: &EndpointDescriptor) -> Option<(f32, f32)> {
        self.endpoint_pre_mute_volume.get(ep).copied().flatten()
    }

    pub fn set_pre_mute_volume(&mut self, ep: EndpointDescriptor, vol: Option<(f32, f32)>) {
        if let Some(v) = vol {
            self.endpoint_pre_mute_volume.insert(ep, Some(v));
        } else {
            self.endpoint_pre_mute_volume.remove(&ep);
        }
    }

    pub fn channel_pipewire_id(&self, id: &ChannelId) -> Option<u32> {
        self.channel_pipewire_id.get(id).copied().flatten()
    }

    pub fn set_channel_pipewire_id(&mut self, id: ChannelId, pw_id: Option<u32>) {
        self.channel_pipewire_id.insert(id, pw_id);
    }

    pub fn channel_pending(&self, id: &ChannelId) -> bool {
        self.channel_pending.get(id).copied().unwrap_or(false)
    }

    pub fn set_channel_pending(&mut self, id: ChannelId, pending: bool) {
        if pending {
            self.channel_pending.insert(id, true);
        } else {
            self.channel_pending.remove(&id);
        }
    }

    pub fn app_is_active(&self, id: &AppId) -> bool {
        self.app_is_active.get(id).copied().unwrap_or(false)
    }

    pub fn set_app_active(&mut self, id: AppId, active: bool) {
        if active {
            self.app_is_active.insert(id, true);
        } else {
            self.app_is_active.remove(&id);
        }
    }

    pub fn link_pending(&self, key: &LinkKey) -> bool {
        self.link_pending.get(key).copied().unwrap_or(false)
    }

    pub fn set_link_pending(&mut self, key: LinkKey, pending: bool) {
        if pending {
            self.link_pending.insert(key, true);
        } else {
            self.link_pending.remove(&key);
        }
    }
}
