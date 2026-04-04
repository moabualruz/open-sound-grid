// Volume command handlers extracted from update.rs.
//
// Handles: SetVolume, SetStereoVolume, SetMute, SetVolumeLocked

use crate::graph::events::MixerEvent;
use crate::graph::{EndpointDescriptor, MixerSession, ReconcileSettings, RuntimeState};
use crate::pw::AudioGraph;
use crate::routing::handler::CommandHandler;
use crate::routing::messages::{StateMsg, StateOutputMsg};

/// Command handler for volume-related messages.
pub struct VolumeCommandHandler;

impl CommandHandler for VolumeCommandHandler {
    fn handles(&self, msg: &StateMsg) -> bool {
        matches!(
            msg,
            StateMsg::SetVolume(..)
                | StateMsg::SetStereoVolume(..)
                | StateMsg::SetMute(..)
                | StateMsg::SetVolumeLocked(..)
                | StateMsg::SetLinkVolume(..)
                | StateMsg::SetLinkStereoVolume(..)
        )
    }

    fn handle(
        &self,
        session: &mut MixerSession,
        msg: StateMsg,
        graph: &AudioGraph,
        rt: &mut RuntimeState,
        settings: &ReconcileSettings,
    ) -> (Option<StateOutputMsg>, Vec<MixerEvent>) {
        let mut events = Vec::new();
        match msg {
            StateMsg::SetVolume(ep_desc, volume) => {
                session.handle_set_volume(ep_desc, volume, graph, settings, rt, &mut events);
            }
            StateMsg::SetStereoVolume(ep_desc, left, right) => {
                session.handle_set_stereo_volume(ep_desc, left, right, graph, settings, rt, &mut events);
            }
            StateMsg::SetMute(ep_desc, muted) => {
                session.handle_set_mute(ep_desc, muted, graph, settings, rt, &mut events);
            }
            StateMsg::SetVolumeLocked(ep_desc, locked) => {
                session.handle_set_volume_locked(ep_desc, locked, graph, settings, rt, &mut events);
            }
            StateMsg::SetLinkVolume(source, sink, volume) => {
                session.handle_set_link_volume(source, sink, volume, graph, settings, &mut events);
            }
            StateMsg::SetLinkStereoVolume(source, sink, left, right) => {
                session.handle_set_link_stereo_volume(source, sink, left, right, graph, settings, &mut events);
            }
            _ => unreachable!(),
        }
        (None, events)
    }
}

impl MixerSession {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn handle_set_volume(
        &mut self,
        ep_desc: EndpointDescriptor,
        volume: f32,
        graph: &AudioGraph,
        settings: &ReconcileSettings,
        rt: &mut RuntimeState,
        events: &mut Vec<MixerEvent>,
    ) -> bool {
        let nodes = self.resolve_endpoint(ep_desc, graph, settings);
        let Some(endpoint) = self.endpoints.get_mut(&ep_desc) else {
            return false;
        };
        endpoint.volume = volume;
        endpoint.volume_left = volume;
        endpoint.volume_right = volume;
        endpoint.volume_mixed = false;

        if let Some(nodes) = nodes {
            // Mix channels: set volume directly on PW node
            let msgs: Vec<_> = nodes
                .into_iter()
                .map(|n| {
                    let len = n.channel_volumes.len().max(2);
                    MixerEvent::VolumeChanged {
                        node_id: n.id,
                        channels: vec![volume; len],
                    }
                })
                .collect();
            if !msgs.is_empty() {
                rt.set_volume_pending(ep_desc, true);
            }
            events.extend(msgs);
        } else if let EndpointDescriptor::Channel(ch_id) = ep_desc {
            // ADR-007: Source channel volume → fan out effective to all cells
            for link in &self.links {
                if link.start == ep_desc {
                    let eff_l = volume * link.cell_volume_left;
                    let eff_r = volume * link.cell_volume_right;
                    for cell_id in self.find_cell_node_ids(ep_desc, link.end, graph, settings) {
                        events.push(MixerEvent::VolumeChanged {
                            node_id: cell_id,
                            channels: vec![eff_l, eff_r],
                        });
                    }
                }
            }
            let _ = ch_id; // used in ep_desc match
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn handle_set_stereo_volume(
        &mut self,
        ep_desc: EndpointDescriptor,
        left: f32,
        right: f32,
        graph: &AudioGraph,
        settings: &ReconcileSettings,
        rt: &mut RuntimeState,
        events: &mut Vec<MixerEvent>,
    ) {
        let nodes = self.resolve_endpoint(ep_desc, graph, settings);
        let Some(endpoint) = self.endpoints.get_mut(&ep_desc) else {
            return;
        };
        endpoint.volume = (left + right) / 2.0;
        endpoint.volume_left = left;
        endpoint.volume_right = right;
        endpoint.volume_mixed = (left - right).abs() > f32::EPSILON;

        if let Some(nodes) = nodes {
            let msgs: Vec<_> = nodes
                .into_iter()
                .map(|n| {
                    let vols = if n.channel_volumes.len() >= 2 {
                        vec![left, right]
                    } else {
                        vec![(left + right) / 2.0]
                    };
                    MixerEvent::VolumeChanged {
                        node_id: n.id,
                        channels: vols,
                    }
                })
                .collect();
            if !msgs.is_empty() {
                rt.set_volume_pending(ep_desc, true);
            }
            events.extend(msgs);
        } else if matches!(ep_desc, EndpointDescriptor::Channel(_)) {
            // ADR-007: Source channel stereo volume → fan out to cells
            for link in &self.links {
                if link.start == ep_desc {
                    let eff_l = left * link.cell_volume_left;
                    let eff_r = right * link.cell_volume_right;
                    for cell_id in self.find_cell_node_ids(ep_desc, link.end, graph, settings) {
                        events.push(MixerEvent::VolumeChanged {
                            node_id: cell_id,
                            channels: vec![eff_l, eff_r],
                        });
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn handle_set_mute(
        &mut self,
        ep_desc: EndpointDescriptor,
        muted: bool,
        graph: &AudioGraph,
        settings: &ReconcileSettings,
        rt: &mut RuntimeState,
        events: &mut Vec<MixerEvent>,
    ) -> bool {
        // Update endpoint state
        {
            let Some(endpoint) = self.endpoints.get_mut(&ep_desc) else {
                return false;
            };
            endpoint.volume_locked_muted = endpoint.volume_locked_muted.with_mute(muted);
            if muted {
                rt.set_pre_mute_volume(
                    ep_desc,
                    Some((endpoint.volume_left, endpoint.volume_right)),
                );
                endpoint.volume_left = 0.0;
                endpoint.volume_right = 0.0;
                endpoint.volume = 0.0;
            } else if let Some((left, right)) = rt.pre_mute_volume(&ep_desc) {
                rt.set_pre_mute_volume(ep_desc, None);
                endpoint.volume_left = left;
                endpoint.volume_right = right;
                endpoint.volume = (left + right) / 2.0;
            }
        }
        // Read back volumes after dropping mutable borrow
        let (vol_l, vol_r) = self
            .endpoints
            .get(&ep_desc)
            .map(|ep| (ep.volume_left, ep.volume_right))
            .unwrap_or((0.0, 0.0));

        // Push to PW
        let is_device = matches!(ep_desc, EndpointDescriptor::Device(..));
        if is_device {
            if let Some(nodes) = self.resolve_endpoint(ep_desc, graph, settings) {
                events.extend(
                    nodes
                        .into_iter()
                        .map(|n| MixerEvent::MuteChanged {
                            node_id: n.id,
                            muted,
                        }),
                );
            }
        } else if let Some(nodes) = self.resolve_endpoint(ep_desc, graph, settings) {
            // Mix: set volume on PW node directly
            let vols = vec![vol_l, vol_r];
            events.extend(
                nodes
                    .into_iter()
                    .map(|n| MixerEvent::VolumeChanged {
                        node_id: n.id,
                        channels: vols.clone(),
                    }),
            );
        } else if matches!(ep_desc, EndpointDescriptor::Channel(_)) {
            // Source channel: fan out effective to cells
            for link in &self.links {
                if link.start == ep_desc {
                    let eff_l = vol_l * link.cell_volume_left;
                    let eff_r = vol_r * link.cell_volume_right;
                    for cell_id in self.find_cell_node_ids(ep_desc, link.end, graph, settings) {
                        events.push(MixerEvent::VolumeChanged {
                            node_id: cell_id,
                            channels: vec![eff_l, eff_r],
                        });
                    }
                }
            }
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn handle_set_volume_locked(
        &mut self,
        ep_desc: EndpointDescriptor,
        locked: bool,
        graph: &AudioGraph,
        settings: &ReconcileSettings,
        rt: &mut RuntimeState,
        events: &mut Vec<MixerEvent>,
    ) -> bool {
        let nodes = self.resolve_endpoint(ep_desc, graph, settings);
        let Some(endpoint) = self.endpoints.get_mut(&ep_desc) else {
            return false;
        };
        if endpoint.volume_locked_muted.is_locked() == locked {
            return false;
        }

        if locked {
            if let Some(new_state) = endpoint.volume_locked_muted.lock() {
                endpoint.volume_locked_muted = new_state;
            } else {
                return false;
            }

            let Some(nodes) = nodes else {
                return false;
            };

            if !rt.volume_pending(&ep_desc)
                && nodes
                    .iter()
                    .all(|n| n.channel_volumes.iter().all(|v| *v == endpoint.volume))
            {
                return false;
            }

            endpoint.volume_mixed = false;
            let msgs: Vec<_> = nodes
                .iter()
                .map(|n| MixerEvent::VolumeChanged {
                    node_id: n.id,
                    channels: vec![endpoint.volume; n.channel_volumes.len()],
                })
                .collect();
            if !msgs.is_empty() {
                rt.set_volume_pending(ep_desc, true);
            }
            events.extend(msgs);
        } else {
            endpoint.volume_locked_muted = endpoint.volume_locked_muted.unlock();
        }
        true
    }
}
