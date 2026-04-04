// Re-export domain events from graph::events.
//
// This module previously held forward-design placeholders. The canonical
// definitions now live in graph::events; this re-export keeps any existing
// `use crate::events::MixerEvent` paths working.

pub use crate::graph::events::*;
