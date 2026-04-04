//! Focused service traits — SOLID interface boundaries (see CLAUDE.md Standards › SOLID).
//!
//! - `VolumeService` — volume and mute mutations (I: consumers depend only on what they use)
//! - `GraphObserver` — read access to the PipeWire audio graph (D: depends on trait, not OsgCore)
//! - `RoutingService` — routing commands and session state (D: depends on trait, not ReducerHandle)
//!
//! `OsgCore` implements all three traits. Mock impls in tests prove Liskov substitutability.

pub mod graph_observer;
pub mod routing_service;
pub mod volume_service;

pub use graph_observer::GraphObserver;
pub use routing_service::RoutingService;
pub use volume_service::VolumeService;
