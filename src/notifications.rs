//! Desktop notifications for audio device events.

use tracing::{debug, info, instrument};

/// Show a notification when an audio device connects or disconnects.
#[instrument]
pub fn notify_device_change(summary: &str, body: &str) {
    info!(summary, body, "sending device change notification");
    if let Err(e) = notify_rust::Notification::new()
        .appname("Open Sound Grid")
        .summary(summary)
        .body(body)
        .icon("audio-card")
        .timeout(notify_rust::Timeout::Milliseconds(3000))
        .show()
    {
        debug!(error = %e, "notification send failed (non-fatal)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notify_device_change_does_not_panic_without_dbus() {
        notify_device_change("Test", "Testing notification");
    }
}
