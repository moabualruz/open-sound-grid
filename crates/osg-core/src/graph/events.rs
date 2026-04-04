// Domain events emitted by MixerSession::update() and ReconciliationService::diff().
//
// An event translator converts these to ToPipewireMessage at the infrastructure
// boundary. Domain code never references PipeWire types directly.

use serde::{Deserialize, Serialize};
use ulid::Ulid;

use super::channel::ChannelKind;
use super::effects_config::EffectsConfig;
use super::eq_config::EqConfig;

/// Domain events produced by aggregate mutations and reconciliation.
///
/// Each variant corresponds to a single side-effect that the infrastructure
/// layer must perform. The event translator maps these 1:1 to
/// `ToPipewireMessage` variants, resolving any PipeWire-specific IDs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum MixerEvent {
    /// PipeWire graph changed — re-run reconciliation.
    RequestReconciliation,

    /// Set per-channel volume on a node (L/R or arbitrary channel count).
    VolumeChanged {
        node_id: u32,
        channels: Vec<f32>,
    },

    /// Mute or unmute a node.
    MuteChanged {
        node_id: u32,
        muted: bool,
    },

    /// Create a port-level link between two ports.
    CreatePortLink {
        start_id: u32,
        end_id: u32,
    },

    /// Create all matching port links between two nodes.
    CreateNodeLinks {
        start_id: u32,
        end_id: u32,
    },

    /// Remove a port-level link.
    RemovePortLink {
        start_id: u32,
        end_id: u32,
    },

    /// Remove all links between two nodes.
    RemoveNodeLinks {
        start_id: u32,
        end_id: u32,
    },

    /// Create a channel/mix group node (virtual audio bus) in PipeWire.
    CreateGroupNode {
        name: String,
        ulid: Ulid,
        kind: ChannelKind,
        instance_id: Ulid,
    },

    /// Remove a channel/mix group node from PipeWire.
    RemoveGroupNode {
        ulid: Ulid,
    },

    /// Set the OS default audio sink.
    SetDefaultSink {
        node_name: String,
        pipewire_node_id: u32,
    },

    /// Create a per-cell null-audio-sink for matrix routing (ADR-007).
    CreateCellNode {
        name: String,
        cell_id: String,
        channel_ulid: String,
        mix_ulid: String,
        instance_id: Ulid,
    },

    /// Remove a per-cell volume node and its links.
    RemoveCellNode {
        cell_node_id: u32,
    },

    /// Redirect an app stream to a channel's virtual sink via direct PW links.
    RedirectStream {
        stream_node_id: u32,
        target_node_id: u32,
    },

    /// Remove links between a stream and a channel node.
    ClearRedirect {
        stream_node_id: u32,
        target_node_id: u32,
    },

    /// Create the staging sink for glitch-free rerouting (ADR-007).
    CreateStagingSink {
        instance_id: Ulid,
    },

    /// Create an inline pw_filter for EQ + peak metering.
    CreateFilter {
        filter_key: String,
        name: String,
    },

    /// Remove an inline pw_filter by key.
    RemoveFilter {
        filter_key: String,
    },

    /// Update EQ parameters on an existing filter.
    UpdateFilterEq {
        filter_key: String,
        eq: EqConfig,
    },

    /// Update effects chain parameters on an existing filter.
    UpdateFilterEffects {
        filter_key: String,
        effects: EffectsConfig,
    },

    /// Persist current state to disk.
    StatePersistRequested,

    /// Shut down the application.
    Exit,
}
