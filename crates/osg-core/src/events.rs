//! Forward-design placeholder — typed domain events per command category.
//!
//! These types are not yet wired into the command pipeline. They describe the
//! intended future shape: each category gets its own typed channel with
//! independent backpressure and debounce (see ADR-007 and CLAUDE.md architecture
//! section). When MixerSession::update() is refactored to return Vec<MixerEvent>
//! instead of Vec<ToPipewireMessage>, the enums below become the wire types.
//!
//! Nothing imports from this module yet — add imports as each category is wired.
