// diff_properties — volume / mute reconciliation

use std::collections::HashMap;

use crate::graph::{
    EndpointDescriptor, MixerEvent, MixerSession, RuntimeState, VolumeLockMuteState,
    average_volumes, volumes_mixed,
};
use crate::pw::Node as PwNode;

impl MixerSession {
    /// Compare backend node properties against desired endpoints.
    /// Locked endpoints push their values to PW; unlocked endpoints pull from PW.
    pub fn diff_properties(
        &mut self,
        endpoint_nodes: &HashMap<EndpointDescriptor, Vec<&PwNode>>,
        rt: &mut RuntimeState,
    ) -> Vec<MixerEvent> {
        let mut messages = Vec::new();

        for (ep_desc, nodes) in endpoint_nodes {
            let Some(endpoint) = self.endpoints.get_mut(ep_desc) else {
                continue;
            };
            let num_messages_before = messages.len();

            if rt.volume_pending(ep_desc) {
                // While a command is in-flight, just check if PW has converged.
                let volumes_match = if endpoint.volume_locked_muted.is_locked() {
                    nodes
                        .iter()
                        .flat_map(|n| &n.channel_volumes)
                        .all(|vol| *vol == endpoint.volume)
                } else {
                    average_volumes(nodes.iter().flat_map(|n| &n.channel_volumes))
                        == endpoint.volume
                };
                let mute_match = endpoint.volume_locked_muted.is_muted()
                    == crate::graph::aggregate_bools(nodes.iter().map(|n| &n.mute));
                if volumes_match && mute_match {
                    rt.set_volume_pending(*ep_desc, false);
                }
            } else if endpoint.volume_locked_muted.is_locked() {
                // Locked: push desired volume to any divergent nodes.
                endpoint.volume_mixed = false;
                messages.extend(
                    nodes
                        .iter()
                        .filter(|n| n.channel_volumes.iter().any(|cv| *cv != endpoint.volume))
                        .map(|n| MixerEvent::VolumeChanged {
                            node_id: n.id,
                            channels: vec![endpoint.volume; n.channel_volumes.len()],
                        }),
                );
                // Push desired mute state.
                // Locked endpoints cannot be in MuteMixed state (lock() returns None for it)
                #[allow(clippy::expect_used)]
                let endpoint_muted = endpoint
                    .volume_locked_muted
                    .is_muted()
                    .expect("locked endpoint cannot be MuteMixed");
                messages.extend(nodes.iter().filter(|n| n.mute != endpoint_muted).map(|n| {
                    MixerEvent::MuteChanged {
                        node_id: n.id,
                        muted: endpoint_muted,
                    }
                }));
            } else if endpoint.volume_locked_muted.is_muted() != Some(true) {
                // Unlocked + unmuted: pull volume/mute from PW nodes into desired state.
                // Skip pull when muted — we implement mute as volume=0 on null-audio-sinks
                // and don't want the reconciler to overwrite pre_mute_volume.
                endpoint.volume_locked_muted =
                    VolumeLockMuteState::from_bools_unlocked(nodes.iter().map(|n| &n.mute));
                endpoint.set_volume(average_volumes(
                    nodes.iter().flat_map(|n| &n.channel_volumes),
                ));
                for node in nodes {
                    endpoint.volume_mixed = volumes_mixed(&node.channel_volumes);
                }
            }

            if messages.len() > num_messages_before {
                rt.set_volume_pending(*ep_desc, true);
            }
        }

        messages
    }
}
