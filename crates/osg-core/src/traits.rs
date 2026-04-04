//! Forward-design placeholder — SOLID interface traits for handlers and transport adapters.
//!
//! These traits are not yet implemented or imported anywhere. They describe the
//! intended SOLID boundary (see CLAUDE.md Standards › SOLID):
//!   - `EventHandler<E>` — one handler per command category (S + O)
//!   - `SinkManager`, `Router`, `VolumeControl`, `GraphObserver` — focused core traits (I)
//!   - `VolumeService`, `GraphService`, `RoutingService` — transport-agnostic service layer (D)
//!
//! Wire these traits in as each category is extracted from the monolithic mainloop.
//! The event types (`VolumeEvent`, `GraphEvent`) will move here from the broadcast
//! handlers once the typed-channel refactor lands.
