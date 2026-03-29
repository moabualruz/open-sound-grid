//! Global hotkey listener using KDE's kglobalacceld D-Bus service.
//!
//! Registers `Ctrl+Shift+M` as "open-sound-grid/mute_all" with
//! `org.kde.KGlobalAccel` on the session bus, then listens for the
//! `globalShortcutPressed` signal on the component object and forwards
//! activations through a [`tokio::sync::mpsc`] channel.
//!
//! The public entry point is [`spawn_hotkey_listener`].  If D-Bus is
//! unavailable or kglobalacceld is not running the function logs a warning
//! and returns a receiver that will never yield — the rest of the app is
//! unaffected.

use iced::futures::StreamExt as _;
use tokio::sync::mpsc;
use tracing::{debug, info, instrument, trace, warn};

// ── Public API ────────────────────────────────────────────────────────────────

/// Events that can be emitted by the hotkey listener.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyEvent {
    /// `Ctrl+Shift+M` — mute all channels.
    MuteAll,
}

/// Spawn the KDE global-shortcut listener in a background task.
///
/// Returns a receiver that yields [`HotkeyEvent`] values when a registered
/// hotkey is activated.  If D-Bus / kglobalacceld is unavailable the receiver
/// will never yield — no error is propagated to the caller.
#[instrument]
pub fn spawn_hotkey_listener() -> mpsc::UnboundedReceiver<HotkeyEvent> {
    info!("spawning KDE global hotkey listener");
    let (tx, rx) = mpsc::unbounded_channel::<HotkeyEvent>();
    tokio::spawn(async move {
        if let Err(e) = run_listener(tx).await {
            warn!(error = %e, "hotkey listener exited with error — hotkeys disabled");
        }
    });
    rx
}

// ── D-Bus constants ───────────────────────────────────────────────────────────

/// Component unique name used when registering with kglobalacceld.
const COMPONENT_UNIQUE: &str = "open-sound-grid";

/// Human-readable component name shown in KDE System Settings → Shortcuts.
const COMPONENT_FRIENDLY: &str = "Open Sound Grid";

/// Unique name for the mute-all action.
const ACTION_UNIQUE: &str = "mute_all";

/// Human-readable action name shown in KDE System Settings → Shortcuts.
const ACTION_FRIENDLY: &str = "Mute All";

/// Qt key integer for `Ctrl+Shift+M`.
///
/// `Qt::ControlModifier` (0x0400_0000) | `Qt::ShiftModifier` (0x0200_0000) |
/// `Qt::Key_M` (0x4d) = `0x0600_004d` = 100_663_373.
const QT_KEY_CTRL_SHIFT_M: i32 = 0x0600_004d;

/// `KGlobalAccel::NoAutoloading` — do not clobber a binding the user may have
/// customised in KDE System Settings.
const FLAG_NO_AUTOLOADING: u32 = 0x2;

// ── zbus proxy definitions ────────────────────────────────────────────────────

/// Proxy for `org.kde.KGlobalAccel` (the registration interface).
#[zbus::proxy(
    interface = "org.kde.KGlobalAccel",
    default_service = "org.kde.kglobalaccel",
    default_path = "/kglobalaccel"
)]
trait KGlobalAccel {
    /// Register an action with the daemon.
    ///
    /// `action_id` must be
    /// `[component_unique, action_unique, component_friendly, action_friendly]`.
    fn do_register(&self, action_id: &[&str]) -> zbus::Result<()>;

    /// Set the active key binding for an action.
    ///
    /// `keys` is a list of Qt key integers; `flags` is a `KGlobalAccel::RegisterFlag`
    /// bitmask.
    fn set_shortcut(&self, action_id: &[&str], keys: &[i32], flags: u32) -> zbus::Result<Vec<i32>>;

    /// Return the D-Bus object path for the component named `component_unique`.
    fn get_component(
        &self,
        component_unique: &str,
    ) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;
}

/// Proxy for `org.kde.kglobalaccel.Component` (the signal source).
#[zbus::proxy(
    interface = "org.kde.kglobalaccel.Component",
    default_service = "org.kde.kglobalaccel"
)]
trait KGlobalAccelComponent {
    /// Emitted when a registered shortcut key is pressed.
    #[zbus(signal)]
    fn global_shortcut_pressed(
        &self,
        component_unique: String,
        action_unique: String,
        timestamp: i64,
    ) -> zbus::Result<()>;
}

// ── Listener implementation ───────────────────────────────────────────────────

/// Connect to kglobalacceld, register the shortcut, then forward signal
/// activations through `tx`.
async fn run_listener(tx: mpsc::UnboundedSender<HotkeyEvent>) -> anyhow::Result<()> {
    debug!("connecting to session D-Bus");
    let conn = zbus::Connection::session().await?;
    trace!("session D-Bus connection established");

    // ── Registration ─────────────────────────────────────────────────────────

    let action_id: &[&str] = &[
        COMPONENT_UNIQUE,
        ACTION_UNIQUE,
        COMPONENT_FRIENDLY,
        ACTION_FRIENDLY,
    ];

    let accel = KGlobalAccelProxy::new(&conn).await?;
    debug!(action_id = ?action_id, "registering action with kglobalacceld");

    if let Err(e) = accel.do_register(action_id).await {
        warn!(error = %e, "do_register failed — kglobalacceld may be unavailable");
        return Err(e.into());
    }
    debug!("action registered");

    match accel
        .set_shortcut(action_id, &[QT_KEY_CTRL_SHIFT_M], FLAG_NO_AUTOLOADING)
        .await
    {
        Ok(assigned) => {
            debug!(assigned_keys = ?assigned, "shortcut keys set (Ctrl+Shift+M)");
        }
        Err(e) => {
            // Non-fatal: the user can still bind the action manually in
            // KDE System Settings → Shortcuts → Open Sound Grid.
            warn!(
                error = %e,
                "set_shortcut failed — bind manually in KDE System Settings if needed"
            );
        }
    }

    // ── Obtain the component object path ─────────────────────────────────────

    let component_path = match accel.get_component(COMPONENT_UNIQUE).await {
        Ok(p) => {
            debug!(path = %p, "obtained component object path");
            p
        }
        Err(e) => {
            warn!(error = %e, "get_component failed — cannot listen for shortcut signals");
            return Err(e.into());
        }
    };

    // ── Subscribe to globalShortcutPressed ────────────────────────────────────

    let component = KGlobalAccelComponentProxy::builder(&conn)
        .path(component_path)?
        .build()
        .await?;

    let mut pressed_stream = component.receive_global_shortcut_pressed().await?;
    info!(
        component = COMPONENT_UNIQUE,
        action = ACTION_UNIQUE,
        key = "Ctrl+Shift+M",
        "hotkey listener ready"
    );

    // ── Event loop ────────────────────────────────────────────────────────────

    loop {
        match pressed_stream.next().await {
            Some(signal) => {
                let args: GlobalShortcutPressedArgs<'_> = match signal.args() {
                    Ok(a) => a,
                    Err(e) => {
                        warn!(error = %e, "failed to parse globalShortcutPressed args");
                        continue;
                    }
                };

                trace!(
                    component = %args.component_unique,
                    action = %args.action_unique,
                    timestamp = args.timestamp,
                    "globalShortcutPressed signal received"
                );

                // Filter to our own action only.
                if args.component_unique == COMPONENT_UNIQUE && args.action_unique == ACTION_UNIQUE
                {
                    info!("Ctrl+Shift+M activated — sending MuteAll event");
                    if tx.send(HotkeyEvent::MuteAll).is_err() {
                        debug!("hotkey event receiver dropped — shutting down listener");
                        return Ok(());
                    }
                }
            }
            None => {
                debug!("globalShortcutPressed signal stream ended");
                return Ok(());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotkey_event_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<HotkeyEvent>();
    }
}
