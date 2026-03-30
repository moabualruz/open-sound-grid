//! Application core: state, messages, update loop, view, and subscriptions.

pub mod handlers;
pub mod messages;
pub mod state;
pub mod persistence;
pub mod update;
pub mod view;
pub mod view_header;
pub mod subscriptions;

pub use messages::{ChannelPanelTab, Message};
pub use state::App;
