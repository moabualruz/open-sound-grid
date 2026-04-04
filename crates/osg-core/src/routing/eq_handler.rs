// EQ and effects command handlers extracted from eq_handlers.rs.
//
// Handles: SetEq, SetCellEq, SetEffects, SetCellEffects

use crate::graph::{ChannelKind, EffectsConfig, EndpointDescriptor, EqConfig, MixerSession};
use crate::pw::ToPipewireMessage;
use crate::routing::filter_lifecycle;

impl MixerSession {
    /// Handle `StateMsg::SetEq` — update endpoint EQ and dispatch to PW filter.
    pub(crate) fn handle_set_eq(
        &mut self,
        ep_desc: EndpointDescriptor,
        eq: EqConfig,
    ) -> Vec<ToPipewireMessage> {
        let mut pw_messages = Vec::new();
        if let Some(ep) = self.endpoints.get_mut(&ep_desc) {
            ep.eq = eq.clone();
        }
        // Dispatch EQ to PW filter — mix filters keyed as "mix.{ulid}"
        let filter_key = match ep_desc {
            EndpointDescriptor::Channel(id) => {
                let ch = self.channels.get(&id);
                if ch.is_some_and(|c| c.kind == ChannelKind::Sink) {
                    format!("mix.{}", id.inner())
                } else {
                    String::new() // source channels have no direct filter
                }
            }
            _ => String::new(),
        };
        if !filter_key.is_empty() {
            filter_lifecycle::update_eq(&mut pw_messages, &filter_key, eq);
        }
        pw_messages
    }

    /// Handle `StateMsg::SetCellEq` — update link EQ and dispatch to cell filter.
    pub(crate) fn handle_set_cell_eq(
        &mut self,
        source: EndpointDescriptor,
        sink: EndpointDescriptor,
        eq: EqConfig,
    ) -> Vec<ToPipewireMessage> {
        let mut pw_messages = Vec::new();
        if let Some(l) = self
            .links
            .iter_mut()
            .find(|l| l.start == source && l.end == sink)
        {
            l.cell_eq = eq.clone();
        }
        // Dispatch EQ to cell's PW filter
        let filter_key = match (&source, &sink) {
            (EndpointDescriptor::Channel(ch), EndpointDescriptor::Channel(mx)) => {
                format!("{}-to-{}", ch.inner(), mx.inner())
            }
            _ => String::new(),
        };
        if !filter_key.is_empty() {
            filter_lifecycle::update_eq(&mut pw_messages, &filter_key, eq);
        }
        pw_messages
    }

    /// Handle `StateMsg::SetEffects` — update endpoint effects and dispatch to PW filter.
    pub(crate) fn handle_set_effects(
        &mut self,
        ep_desc: EndpointDescriptor,
        effects: EffectsConfig,
    ) -> Vec<ToPipewireMessage> {
        let mut pw_messages = Vec::new();
        if let Some(ep) = self.endpoints.get_mut(&ep_desc) {
            ep.effects = effects.clone();
        }
        // Dispatch effects to PW filter — mix filters keyed as "mix.{ulid}"
        let filter_key = match ep_desc {
            EndpointDescriptor::Channel(id) => {
                let ch = self.channels.get(&id);
                if ch.is_some_and(|c| c.kind == ChannelKind::Sink) {
                    format!("mix.{}", id.inner())
                } else {
                    String::new()
                }
            }
            _ => String::new(),
        };
        if !filter_key.is_empty() {
            filter_lifecycle::update_effects(&mut pw_messages, &filter_key, effects);
        }
        pw_messages
    }

    /// Handle `StateMsg::SetCellEffects` — update link effects and dispatch to cell filter.
    pub(crate) fn handle_set_cell_effects(
        &mut self,
        source: EndpointDescriptor,
        sink: EndpointDescriptor,
        effects: EffectsConfig,
    ) -> Vec<ToPipewireMessage> {
        let mut pw_messages = Vec::new();
        if let Some(l) = self
            .links
            .iter_mut()
            .find(|l| l.start == source && l.end == sink)
        {
            l.cell_effects = effects.clone();
        }
        // Dispatch effects to cell's PW filter
        let filter_key = match (&source, &sink) {
            (EndpointDescriptor::Channel(ch), EndpointDescriptor::Channel(mx)) => {
                format!("{}-to-{}", ch.inner(), mx.inner())
            }
            _ => String::new(),
        };
        if !filter_key.is_empty() {
            filter_lifecycle::update_effects(&mut pw_messages, &filter_key, effects);
        }
        pw_messages
    }
}
