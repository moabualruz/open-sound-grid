// Filter lifecycle helpers — shared message constructors for CreateFilter,
// RemoveFilter, UpdateFilterEq, and UpdateFilterEffects.
//
// Eliminates inline message construction that was repeated across
// eq_handler, endpoint_handler, and reconcile.

use crate::graph::events::MixerEvent;
use crate::graph::{EffectsConfig, EqConfig};

// ---------------------------------------------------------------------------
// Domain-event helpers (used by handlers and reconcile)
// ---------------------------------------------------------------------------

/// Push a `CreateFilter` domain event onto `events`.
#[expect(
    dead_code,
    reason = "reserved for handler use when filter creation is wired up"
)]
pub(crate) fn create_filter_event(events: &mut Vec<MixerEvent>, key: &str, name: &str) {
    events.push(MixerEvent::CreateFilter {
        filter_key: key.to_owned(),
        name: name.to_owned(),
    });
}

/// Push a `RemoveFilter` domain event onto `events`.
#[expect(
    dead_code,
    reason = "reserved for handler use when filter removal is wired up"
)]
pub(crate) fn remove_filter_event(events: &mut Vec<MixerEvent>, key: &str) {
    events.push(MixerEvent::RemoveFilter {
        filter_key: key.to_owned(),
    });
}

/// Push an `UpdateFilterEq` domain event onto `events`.
pub(crate) fn update_eq_event(events: &mut Vec<MixerEvent>, key: &str, eq: EqConfig) {
    events.push(MixerEvent::UpdateFilterEq {
        filter_key: key.to_owned(),
        eq,
    });
}

/// Push an `UpdateFilterEffects` domain event onto `events`.
pub(crate) fn update_effects_event(
    events: &mut Vec<MixerEvent>,
    key: &str,
    effects: EffectsConfig,
) {
    events.push(MixerEvent::UpdateFilterEffects {
        filter_key: key.to_owned(),
        effects,
    });
}
