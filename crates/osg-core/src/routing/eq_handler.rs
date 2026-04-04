// EQ and effects command handlers extracted from eq_handlers.rs.
//
// Handles: SetEq, SetCellEq, SetEffects, SetCellEffects

use crate::graph::events::MixerEvent;
use crate::graph::{ChannelKind, EffectsConfig, EndpointDescriptor, EqConfig, MixerSession};
use crate::routing::filter_lifecycle;
use crate::routing::handler::CommandHandler;
use crate::routing::messages::{StateMsg, StateOutputMsg};

/// Command handler for EQ and effects messages.
pub struct EqCommandHandler;

impl CommandHandler for EqCommandHandler {
    fn handles(&self, msg: &StateMsg) -> bool {
        matches!(
            msg,
            StateMsg::SetEq(..)
                | StateMsg::SetCellEq(..)
                | StateMsg::SetEffects(..)
                | StateMsg::SetCellEffects(..)
        )
    }

    fn handle(
        &self,
        session: &mut MixerSession,
        msg: StateMsg,
        _graph: &crate::pw::AudioGraph,
        _rt: &mut crate::graph::RuntimeState,
        _settings: &crate::graph::ReconcileSettings,
    ) -> (Option<StateOutputMsg>, Vec<MixerEvent>) {
        let events = match msg {
            StateMsg::SetEq(ep_desc, eq) => session.handle_set_eq(ep_desc, eq),
            StateMsg::SetCellEq(source, sink, eq) => session.handle_set_cell_eq(source, sink, eq),
            StateMsg::SetEffects(ep_desc, effects) => session.handle_set_effects(ep_desc, effects),
            StateMsg::SetCellEffects(source, sink, effects) => {
                session.handle_set_cell_effects(source, sink, effects)
            }
            _ => unreachable!(),
        };
        (None, events)
    }
}

impl MixerSession {
    /// Handle `StateMsg::SetEq` — update endpoint EQ and dispatch to PW filter.
    pub(crate) fn handle_set_eq(
        &mut self,
        ep_desc: EndpointDescriptor,
        eq: EqConfig,
    ) -> Vec<MixerEvent> {
        let mut events = Vec::new();
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
            filter_lifecycle::update_eq_event(&mut events, &filter_key, eq);
        }
        events
    }

    /// Handle `StateMsg::SetCellEq` — update link EQ and dispatch to cell filter.
    pub(crate) fn handle_set_cell_eq(
        &mut self,
        source: EndpointDescriptor,
        sink: EndpointDescriptor,
        eq: EqConfig,
    ) -> Vec<MixerEvent> {
        let mut events = Vec::new();
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
            filter_lifecycle::update_eq_event(&mut events, &filter_key, eq);
        }
        events
    }

    /// Handle `StateMsg::SetEffects` — update endpoint effects and dispatch to PW filter.
    pub(crate) fn handle_set_effects(
        &mut self,
        ep_desc: EndpointDescriptor,
        effects: EffectsConfig,
    ) -> Vec<MixerEvent> {
        let mut events = Vec::new();
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
            filter_lifecycle::update_effects_event(&mut events, &filter_key, effects);
        }
        events
    }

    /// Handle `StateMsg::SetCellEffects` — update link effects and dispatch to cell filter.
    pub(crate) fn handle_set_cell_effects(
        &mut self,
        source: EndpointDescriptor,
        sink: EndpointDescriptor,
        effects: EffectsConfig,
    ) -> Vec<MixerEvent> {
        let mut events = Vec::new();
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
            filter_lifecycle::update_effects_event(&mut events, &filter_key, effects);
        }
        events
    }
}
