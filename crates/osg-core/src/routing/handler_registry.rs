// HandlerRegistry — dispatches StateMsg to the correct CommandHandler.
//
// The registry lives in the reducer (not MixerSession) to avoid borrow
// checker issues: the reducer owns both `session` and `registry` separately.

use crate::graph::events::MixerEvent;
use crate::graph::{MixerSession, ReconcileSettings, RuntimeState};
use crate::pw::AudioGraph;
use crate::routing::app_handler::AppCommandHandler;
use crate::routing::endpoint_handler::EndpointCommandHandler;
use crate::routing::eq_handler::EqCommandHandler;
use crate::routing::handler::CommandHandler;
use crate::routing::link_handler::LinkCommandHandler;
use crate::routing::messages::{StateMsg, StateOutputMsg};
use crate::routing::order_handler::OrderCommandHandler;
use crate::routing::output_handler::OutputCommandHandler;
use crate::routing::volume_handler::VolumeCommandHandler;

/// Registry of command handlers. Iterates handlers in registration order
/// and dispatches to the first one that claims the message.
pub struct HandlerRegistry {
    handlers: Vec<Box<dyn CommandHandler>>,
}

impl std::fmt::Debug for HandlerRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HandlerRegistry")
            .field("handler_count", &self.handlers.len())
            .finish()
    }
}

impl HandlerRegistry {
    /// Create a registry pre-loaded with all built-in handlers.
    pub fn new() -> Self {
        let mut r = Self {
            handlers: Vec::new(),
        };
        r.register(Box::new(VolumeCommandHandler));
        r.register(Box::new(LinkCommandHandler));
        r.register(Box::new(EndpointCommandHandler));
        r.register(Box::new(AppCommandHandler));
        r.register(Box::new(EqCommandHandler));
        r.register(Box::new(OutputCommandHandler));
        r.register(Box::new(OrderCommandHandler));
        r
    }

    /// Register an additional handler (useful for testing with mocks).
    pub fn register(&mut self, handler: Box<dyn CommandHandler>) {
        self.handlers.push(handler);
    }

    /// Returns `true` if any registered handler claims the given message.
    pub fn handles(&self, msg: &StateMsg) -> bool {
        self.handlers.iter().any(|h| h.handles(msg))
    }

    /// Dispatch a message to the appropriate handler.
    ///
    /// # Panics
    /// Panics if no handler claims the message — every `StateMsg` variant
    /// must be covered by exactly one registered handler.
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch(
        &self,
        session: &mut MixerSession,
        msg: StateMsg,
        graph: &AudioGraph,
        rt: &mut RuntimeState,
        settings: &ReconcileSettings,
    ) -> (Option<StateOutputMsg>, Vec<MixerEvent>) {
        for handler in &self.handlers {
            if handler.handles(&msg) {
                return handler.handle(session, msg, graph, rt, settings);
            }
        }
        unreachable!("No handler registered for {:?}", msg);
    }
}

impl Default for HandlerRegistry {
    fn default() -> Self {
        Self::new()
    }
}
