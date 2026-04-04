// CommandHandler trait — one handler per command category (SOLID: S + O + L).
//
// Adding a new command category means creating a new handler struct that
// implements this trait and registering it in the HandlerRegistry. No
// existing match arms or handler code need to change (Open/Closed).

use crate::graph::events::MixerEvent;
use crate::graph::{MixerSession, ReconcileSettings, RuntimeState};
use crate::pw::AudioGraph;
use crate::routing::messages::{StateMsg, StateOutputMsg};

/// One handler per command category. Implementations are interchangeable (Liskov).
pub trait CommandHandler: Send + Sync {
    /// Returns `true` if this handler is responsible for the given message.
    fn handles(&self, msg: &StateMsg) -> bool;

    /// Process the message, mutating `session` and `rt` as needed.
    /// Returns an optional output notification and a vec of domain events.
    #[allow(clippy::too_many_arguments)]
    fn handle(
        &self,
        session: &mut MixerSession,
        msg: StateMsg,
        graph: &AudioGraph,
        rt: &mut RuntimeState,
        settings: &ReconcileSettings,
    ) -> (Option<StateOutputMsg>, Vec<MixerEvent>);
}
