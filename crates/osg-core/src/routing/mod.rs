// Adapted from Sonusmix (MPL-2.0) — https://codeberg.org/sonusmix/sonusmix
//
// The routing module implements the desired-state reconciliation loop:
//   1. UI/API sends `StateMsg` to the reducer
//   2. The reducer applies mutations and runs a diff against the PipeWire graph
//   3. Corrective `ToPipewireMessage` commands are sent to PipeWire
//   4. PipeWire graph updates are debounced (16ms) and fed back into the diff

mod apps;
pub mod messages;
pub mod reconcile;
pub mod reducer;
mod update;

use thiserror::Error;

pub use messages::{StateMsg, StateOutputMsg};
pub use reducer::{ReducerHandle, debounced_graph_sender, run_reducer};

/// Errors originating from the routing/reducer layer.
#[derive(Error, Debug)]
pub enum RoutingError {
    #[error("failed to send message to reducer")]
    ReducerSendFailed,
}
