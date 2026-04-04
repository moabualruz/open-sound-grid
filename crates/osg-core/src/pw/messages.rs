use thiserror::Error;
use ulid::Ulid;

use super::GroupNodeKind;

#[derive(Debug, PartialEq)]
pub enum ToPipewireMessage {
    Update,
    NodeVolume(u32, Vec<f32>),
    NodeMute(u32, bool),
    #[rustfmt::skip]
    CreatePortLink { start_id: u32, end_id: u32 },
    #[rustfmt::skip]
    CreateNodeLinks { start_id: u32, end_id: u32 },
    #[rustfmt::skip]
    RemovePortLink { start_id: u32, end_id: u32 },
    #[rustfmt::skip]
    RemoveNodeLinks { start_id: u32, end_id: u32 },
    /// Domain: AddChannel. Creates a virtual audio bus (Channel) in PipeWire.
    /// Fields: (name, ulid, kind, instance_id).
    CreateGroupNode(String, Ulid, GroupNodeKind, Ulid),
    /// Domain: RemoveChannel. Removes a virtual audio bus (Channel) from PipeWire.
    RemoveGroupNode(Ulid),
    /// Set the OS default audio sink via PipeWire metadata.
    /// (node_name, pipewire_node_id) — tries metadata first, falls back to wpctl.
    SetDefaultSink(String, u32),
    /// Create a per-cell null-audio-sink for matrix routing (ADR-007).
    /// App streams link directly to this sink. Monitor → [filter] → mix.
    CreateCellNode {
        name: String,
        /// Full node name: `osg.cell.{channel_ulid}-to-{mix_ulid}`
        cell_id: String,
        channel_ulid: String,
        mix_ulid: String,
        /// OSG instance ULID stamped on the PW node for ownership tracking.
        instance_id: Ulid,
    },
    /// Remove a per-cell volume node and its links.
    RemoveCellNode {
        cell_node_id: u32,
    },
    /// Redirect an app stream to a channel's virtual sink via direct PW links.
    /// Disconnects the stream from its current target and links to the channel node.
    RedirectStream {
        stream_node_id: u32,
        target_node_id: u32,
    },
    /// Remove links between a stream and a channel node. WirePlumber will
    /// auto-link the stream back to the default sink.
    ClearRedirect {
        stream_node_id: u32,
        target_node_id: u32,
    },
    /// Create the staging sink — always-alive, vol=0, for glitch-free rerouting.
    /// ADR-007: Apps transit through this sink during reassignment to avoid
    /// audio glitches from having no output destination.
    CreateStagingSink {
        instance_id: Ulid,
    },
    /// Create an inline pw_filter for EQ + peak metering.
    /// Inserts between source_node → target_node in the graph.
    /// The filter_key is used to store/retrieve the FilterHandle.
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
        eq: crate::graph::EqConfig,
    },
    /// Update effects chain parameters on an existing filter.
    UpdateFilterEffects {
        filter_key: String,
        effects: crate::graph::EffectsConfig,
    },
    Exit,
}

#[derive(Debug)]
pub(crate) enum FromPipewireMessage {}

#[derive(Error, Debug)]
#[error("failed to send message to Pipewire: {0:?}")]
pub(crate) struct PipewireChannelError(pub(crate) ToPipewireMessage);
