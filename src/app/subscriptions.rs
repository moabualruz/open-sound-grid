//! Async event subscriptions: plugin events, tray commands, hotkeys.

use iced::Subscription;

use crate::plugin::api::PluginEvent;
use crate::tray;

use super::messages::Message;
use super::state::{App, EVENT_RX};

impl App {
    /// Async subscriptions: plugin events + tray commands + window events + keyboard.
    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            Subscription::run(plugin_event_stream),
            Subscription::run(tray_event_stream),
            Subscription::run(hotkey_event_stream),
            iced::event::listen_with(|event, _status, _id| match event {
                iced::Event::Window(iced::window::Event::Resized(size)) => Some(
                    Message::WindowResized(size.width as u32, size.height as u32),
                ),
                _ => None,
            }),
            iced::keyboard::listen().filter_map(|event| match event {
                iced::keyboard::Event::KeyPressed { key, modifiers, .. } => {
                    Some(Message::KeyPressed(key, modifiers))
                }
                _ => None,
            }),
        ])
    }
}

// --- Async plugin event stream (zero latency, no polling) ---

/// Produces a stream of Messages from the plugin event channel.
/// Called by `Subscription::run` — must be a `fn()` pointer.
fn plugin_event_stream() -> impl iced::futures::Stream<Item = Message> {
    iced::stream::channel(64, async move |mut sender| {
        // Take the receiver from the global slot (consumed once)
        let mut rx = EVENT_RX
            .get()
            .and_then(|m| m.lock().ok())
            .and_then(|mut guard| guard.take());

        if rx.is_none() {
            tracing::warn!("no plugin event receiver — subscription idle");
            std::future::pending::<()>().await;
            return;
        }

        let rx = rx.as_mut().unwrap();
        tracing::info!("plugin event subscription started");

        loop {
            match rx.recv().await {
                Some(event) => {
                    let msg = match event {
                        PluginEvent::StateRefreshed(snapshot) => {
                            tracing::info!(
                                hardware_inputs = snapshot.hardware_inputs.len(),
                                hardware_outputs = snapshot.hardware_outputs.len(),
                                channels = snapshot.channels.len(),
                                "subscription received StateRefreshed"
                            );
                            Message::PluginStateRefreshed(snapshot)
                        }
                        PluginEvent::DevicesChanged => Message::PluginDevicesChanged,
                        PluginEvent::ApplicationsChanged(apps) => Message::PluginAppsChanged(apps),
                        PluginEvent::PeakLevels(levels) => Message::PluginPeakLevels(levels),
                        PluginEvent::Error(err) => Message::PluginError(err),
                        PluginEvent::ConnectionLost => Message::PluginConnectionLost,
                        PluginEvent::ConnectionRestored => Message::PluginConnectionRestored,
                        PluginEvent::SpectrumData { channel, bins } => {
                            Message::PluginSpectrumData { channel, bins }
                        }
                    };
                    if sender.try_send(msg).is_err() {
                        tracing::warn!("subscription sender full, dropping event");
                    }
                }
                None => {
                    tracing::warn!("plugin event channel closed");
                    let _ = sender.try_send(Message::PluginError("Plugin disconnected".into()));
                    std::future::pending::<()>().await;
                }
            }
        }
    })
}

// --- Tray command stream ---

/// Produces a stream of Messages from the tray command channel.
fn tray_event_stream() -> impl iced::futures::Stream<Item = Message> {
    iced::stream::channel(16, async move |mut sender| {
        let mut rx = tray::TRAY_RX
            .get()
            .and_then(|m| m.lock().ok())
            .and_then(|mut guard| guard.take());

        if rx.is_none() {
            tracing::debug!("no tray command receiver — tray subscription idle");
            std::future::pending::<()>().await;
            return;
        }

        let rx = rx.as_mut().unwrap();
        tracing::info!("tray command subscription started");

        loop {
            match rx.recv().await {
                Some(cmd) => {
                    tracing::debug!(cmd = ?cmd, "tray command received");
                    let msg = match cmd {
                        tray::TrayCommand::Show => Some(Message::TrayShow),
                        tray::TrayCommand::Hide => {
                            tracing::debug!("tray hide — no-op (iced has no hide API)");
                            None
                        }
                        tray::TrayCommand::Quit => Some(Message::TrayQuit),
                        tray::TrayCommand::MuteAll => Some(Message::TrayMuteAll),
                    };
                    if let Some(msg) = msg {
                        if sender.try_send(msg).is_err() {
                            tracing::warn!("tray subscription sender full, dropping command");
                        }
                    }
                }
                None => {
                    tracing::debug!("tray command channel closed");
                    std::future::pending::<()>().await;
                }
            }
        }
    })
}

// --- Hotkey event stream ---

/// Produces a stream of Messages from the global hotkey listener.
///
/// Spawns `hotkeys::spawn_hotkey_listener()` inside the async closure.
/// If D-Bus / kglobalacceld is unavailable, the stream silently idles.
fn hotkey_event_stream() -> impl iced::futures::Stream<Item = Message> {
    iced::stream::channel(16, async move |mut sender| {
        tracing::info!("hotkey subscription stream starting");

        let mut rx = crate::hotkeys::spawn_hotkey_listener();
        tracing::info!("hotkey listener spawned, waiting for events");

        loop {
            match rx.recv().await {
                Some(event) => {
                    tracing::debug!(?event, "hotkey event received");
                    let msg = match event {
                        crate::hotkeys::HotkeyEvent::MuteAll => Message::HotkeyMuteAll,
                    };
                    if sender.try_send(msg).is_err() {
                        tracing::warn!("hotkey subscription sender full, dropping event");
                    }
                }
                None => {
                    tracing::debug!("hotkey event channel closed");
                    std::future::pending::<()>().await;
                }
            }
        }
    })
}
