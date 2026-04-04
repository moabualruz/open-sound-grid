// Translates domain events (MixerEvent) into PipeWire infrastructure messages.
//
// This is the ONLY place where domain events cross into infrastructure.
// The reducer calls translate_all() after HandlerRegistry::dispatch() returns
// domain events, converting them to ToPipewireMessage for the PW sender.

use crate::graph::events::MixerEvent;
use crate::pw::ToPipewireMessage;

/// Convert a single domain event into zero or more PipeWire messages.
#[allow(clippy::too_many_lines)]
pub fn translate(event: &MixerEvent) -> Vec<ToPipewireMessage> {
    match event {
        MixerEvent::RequestReconciliation => {
            vec![ToPipewireMessage::Update]
        }
        MixerEvent::VolumeChanged { node_id, channels } => {
            vec![ToPipewireMessage::NodeVolume(*node_id, channels.clone())]
        }
        MixerEvent::MuteChanged { node_id, muted } => {
            vec![ToPipewireMessage::NodeMute(*node_id, *muted)]
        }
        MixerEvent::CreatePortLink { start_id, end_id } => {
            vec![ToPipewireMessage::CreatePortLink {
                start_id: *start_id,
                end_id: *end_id,
            }]
        }
        MixerEvent::CreateNodeLinks { start_id, end_id } => {
            vec![ToPipewireMessage::CreateNodeLinks {
                start_id: *start_id,
                end_id: *end_id,
            }]
        }
        MixerEvent::RemovePortLink { start_id, end_id } => {
            vec![ToPipewireMessage::RemovePortLink {
                start_id: *start_id,
                end_id: *end_id,
            }]
        }
        MixerEvent::RemoveNodeLinks { start_id, end_id } => {
            vec![ToPipewireMessage::RemoveNodeLinks {
                start_id: *start_id,
                end_id: *end_id,
            }]
        }
        MixerEvent::CreateGroupNode {
            name,
            ulid,
            kind,
            instance_id,
        } => {
            vec![ToPipewireMessage::CreateGroupNode(
                name.clone(),
                *ulid,
                (*kind).into(),
                *instance_id,
            )]
        }
        MixerEvent::RemoveGroupNode { ulid } => {
            vec![ToPipewireMessage::RemoveGroupNode(*ulid)]
        }
        MixerEvent::SetDefaultSink {
            node_name,
            pipewire_node_id,
        } => {
            vec![ToPipewireMessage::SetDefaultSink(
                node_name.clone(),
                *pipewire_node_id,
            )]
        }
        MixerEvent::CreateCellNode {
            name,
            cell_id,
            channel_ulid,
            mix_ulid,
            instance_id,
        } => {
            vec![ToPipewireMessage::CreateCellNode {
                name: name.clone(),
                cell_id: cell_id.clone(),
                channel_ulid: channel_ulid.clone(),
                mix_ulid: mix_ulid.clone(),
                instance_id: *instance_id,
            }]
        }
        MixerEvent::RemoveCellNode { cell_node_id } => {
            vec![ToPipewireMessage::RemoveCellNode {
                cell_node_id: *cell_node_id,
            }]
        }
        MixerEvent::RedirectStream {
            stream_node_id,
            target_node_id,
        } => {
            vec![ToPipewireMessage::RedirectStream {
                stream_node_id: *stream_node_id,
                target_node_id: *target_node_id,
            }]
        }
        MixerEvent::ClearRedirect {
            stream_node_id,
            target_node_id,
        } => {
            vec![ToPipewireMessage::ClearRedirect {
                stream_node_id: *stream_node_id,
                target_node_id: *target_node_id,
            }]
        }
        MixerEvent::CreateStagingSink { instance_id } => {
            vec![ToPipewireMessage::CreateStagingSink {
                instance_id: *instance_id,
            }]
        }
        MixerEvent::CreateFilter { filter_key, name } => {
            vec![ToPipewireMessage::CreateFilter {
                filter_key: filter_key.clone(),
                name: name.clone(),
            }]
        }
        MixerEvent::RemoveFilter { filter_key } => {
            vec![ToPipewireMessage::RemoveFilter {
                filter_key: filter_key.clone(),
            }]
        }
        MixerEvent::UpdateFilterEq { filter_key, eq } => {
            vec![ToPipewireMessage::UpdateFilterEq {
                filter_key: filter_key.clone(),
                eq: eq.clone(),
            }]
        }
        MixerEvent::UpdateFilterEffects {
            filter_key,
            effects,
        } => {
            vec![ToPipewireMessage::UpdateFilterEffects {
                filter_key: filter_key.clone(),
                effects: effects.clone(),
            }]
        }
        MixerEvent::StatePersistRequested => {
            // No PipeWire message — persistence is handled by the reducer.
            vec![]
        }
        MixerEvent::Exit => {
            vec![ToPipewireMessage::Exit]
        }
    }
}

/// Convert a batch of domain events into PipeWire messages.
pub fn translate_all(events: &[MixerEvent]) -> Vec<ToPipewireMessage> {
    events.iter().flat_map(translate).collect()
}
