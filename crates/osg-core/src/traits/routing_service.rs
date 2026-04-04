//! RoutingService — focused trait for routing commands and session state access.

use std::sync::Arc;

use tokio::sync::watch;

use crate::graph::MixerSession;
use crate::routing::messages::StateMsg;

/// Focused trait for sending routing commands and observing session state
/// (write model).
///
/// Consumers depend on this trait, not on OsgCore or ReducerHandle directly.
pub trait RoutingService {
    /// Send a state-mutation command to the reducer.
    fn command(&self, msg: StateMsg);

    /// Get a snapshot of the current desired state.
    fn state(&self) -> Arc<MixerSession>;

    /// Subscribe to session state changes (watch channel).
    fn subscribe_state(&self) -> watch::Receiver<Arc<MixerSession>>;
}
