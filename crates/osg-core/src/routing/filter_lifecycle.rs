// Filter lifecycle helpers — shared message constructors for CreateFilter,
// RemoveFilter, UpdateFilterEq, and UpdateFilterEffects.
//
// Eliminates inline message construction that was repeated across
// eq_handler, endpoint_handler, and reconcile.

use crate::graph::{EffectsConfig, EqConfig};
use crate::pw::ToPipewireMessage;

/// Push a `CreateFilter` message onto `messages`.
#[expect(dead_code, reason = "reserved for future use in reconcile.rs")]
pub(crate) fn create_filter(messages: &mut Vec<ToPipewireMessage>, key: &str, name: &str) {
    messages.push(ToPipewireMessage::CreateFilter {
        filter_key: key.to_owned(),
        name: name.to_owned(),
    });
}

/// Push a `RemoveFilter` message onto `messages`.
#[expect(dead_code, reason = "reserved for future use in reconcile.rs")]
pub(crate) fn remove_filter(messages: &mut Vec<ToPipewireMessage>, key: &str) {
    messages.push(ToPipewireMessage::RemoveFilter {
        filter_key: key.to_owned(),
    });
}

/// Push an `UpdateFilterEq` message onto `messages`.
pub(crate) fn update_eq(messages: &mut Vec<ToPipewireMessage>, key: &str, eq: EqConfig) {
    messages.push(ToPipewireMessage::UpdateFilterEq {
        filter_key: key.to_owned(),
        eq,
    });
}

/// Push an `UpdateFilterEffects` message onto `messages`.
pub(crate) fn update_effects(
    messages: &mut Vec<ToPipewireMessage>,
    key: &str,
    effects: EffectsConfig,
) {
    messages.push(ToPipewireMessage::UpdateFilterEffects {
        filter_key: key.to_owned(),
        effects,
    });
}
