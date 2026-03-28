/// System tray integration via ksni (StatusNotifierItem).
///
/// Runs in its own thread, communicates with the main iced app
/// via mpsc channels for show/hide/quit commands.
///
/// TODO: Implement ksni::Tray trait
/// - Icon: app icon
/// - Tooltip: "OpenSoundGrid - Audio Mixer"
/// - Menu: Show/Hide, Mute All, Quit
/// - Activation: toggle window visibility
pub fn spawn_tray() {
    tracing::info!("System tray: stub (not yet implemented)");
    // TODO: std::thread::spawn with ksni service
}
